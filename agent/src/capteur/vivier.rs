//! Le vivier d'encodeurs : qui dort, qui veille.
//!
//! **Pas de `#[cfg(windows)]`, aucun objet COM, aucun canal.** Ce module ne
//! fait que décider ; l'application des décisions vit dans `capteur/sommeil.rs`
//! et `capteur/fenetre.rs`. C'est le patron établi par D4 pour
//! `capteur/protocole.rs` et `capteur/distante.rs` : ce qui décide se teste sur
//! l'hôte, et c'est ici la pièce la plus coûteuse à se tromper.
//!
//! **Pourquoi un LRU et pas un « premier arrivé, premier servi ».** La fenêtre
//! au premier plan doit toujours gagner : c'est la main de l'utilisateur qui
//! arbitre, sans qu'il ait rien à régler.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Nombre d'encodeurs simultanément vivants que le capteur s'autorise.
///
/// **Relevé sur cette VM, pas une borne du système** : mesuré les 30 et
/// 31 juillet 2026 (la 9ᵉ création refusée au `SetOutputType` de la MFT NVIDIA,
/// `MF_E_UNSUPPORTED_D3D_TYPE`), inchangé que les encodeurs partagent un
/// périphérique D3D11 ou qu'ils en aient chacun un neuf. **La couche qui
/// l'impose n'est pas identifiée.**
pub const PLAFOND_EVEIL: usize = 8;

/// Temps minimal d'éveil avant qu'une fenêtre puisse être ÉVINCÉE.
///
/// ⚠️ **Valeur NON CALIBRÉE.** Elle borne le battement — dix fenêtres visibles
/// et un utilisateur qui passe de l'une à l'autre reconstruiraient sinon une
/// duplication DXGI et un encodeur par changement de focus. Le nombre
/// d'endormissements relevé à la recette est ce qui la jugera, pas une
/// intuition.
///
/// Elle ne protège PAS contre une mise en veille voulue : se masquer est un
/// geste explicite de l'utilisateur.
pub const HYSTERESIS: Duration = Duration::from_secs(2);

/// Temps d'attente après l'échec de reveil d'une fenêtre avant de la reproposer.
///
/// ⚠️ **Valeur NON CALIBRÉE.** Elle borne la fréquence de rejeu d'un réveil
/// refusé : sans elle, une fenêtre dont la construction d'encodeur échoue
/// serait relancée à chaque arbitrage dans la boucle serrée. Le nombre de
/// tentatives de reveil observé à la recette est ce qui la jugera.
pub const REPIT_APRES_ECHEC: Duration = Duration::from_millis(500);

/// Pourquoi une fenêtre s'endort. Les deux cas ne se valent pas pour
/// l'utilisateur, et le client les affiche différemment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raison {
    /// Il l'a voulu : la fenêtre est minimisée ou son onglet est caché.
    Masquee,
    /// Le vivier la lui a prise alors qu'il la regardait.
    Evincee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ordre {
    Dormir(Raison),
    Reveiller,
}

struct Entree {
    visible: bool,
    /// Instant du dernier focus ou de la dernière remise en visibilité.
    ///
    /// **La visibilité seule ne suffirait pas à ordonner un LRU** : dix
    /// fenêtres toutes visibles ont exactement la même visibilité, et
    /// l'éviction serait alors arbitraire.
    dernier_vu: Instant,
    eveillee: bool,
    eveillee_depuis: Instant,
    /// Instant du dernier échec de reveil, ou `None` si jamais échoué ou depuis
    /// longtemps. Exclut la fenêtre des candidates tant que le répit n'est pas
    /// écoulé.
    dernier_echec: Option<Instant>,
}

pub struct Vivier {
    plafond: usize,
    hysteresis: Duration,
    entrees: HashMap<String, Entree>,
}

impl Vivier {
    pub fn nouveau(plafond: usize, hysteresis: Duration) -> Vivier {
        Vivier { plafond, hysteresis, entrees: HashMap::new() }
    }

    pub fn inscrire(&mut self, session: &str, maintenant: Instant) -> Vec<(String, Ordre)> {
        // Une fenêtre naît ENDORMIE : le client annoncera sa visibilité, et
        // c'est elle qui la réveillera. Naître éveillée ferait dépasser le
        // plafond entre l'attache et le premier signal.
        self.entrees.insert(
            session.to_string(),
            Entree {
                visible: false,
                dernier_vu: maintenant,
                eveillee: false,
                eveillee_depuis: maintenant,
                dernier_echec: None,
            },
        );
        self.arbitrer(maintenant)
    }

    pub fn retirer(&mut self, session: &str, maintenant: Instant) -> Vec<(String, Ordre)> {
        self.entrees.remove(session);
        self.arbitrer(maintenant)
    }

    pub fn signaler(
        &mut self,
        session: &str,
        visible: bool,
        focalisee: bool,
        maintenant: Instant,
    ) -> Vec<(String, Ordre)> {
        let Some(entree) = self.entrees.get_mut(session) else {
            // Un signal peut arriver d'un enfant dont la fenêtre vient d'être
            // retirée. Ignorer, jamais paniquer.
            return Vec::new();
        };
        // La récence se rafraîchit au focus ET au retour de visibilité : ce
        // sont les deux façons dont l'utilisateur dit « je regarde celle-ci ».
        if focalisee || (visible && !entree.visible) {
            entree.dernier_vu = maintenant;
        }
        entree.visible = visible;
        self.arbitrer(maintenant)
    }

    /// Enregistrer l'échec du reveil d'une fenêtre et mettre à jour l'état.
    ///
    /// Appelée par le capteur quand la construction de l'encodeur échoue.
    /// Repasse l'entrée à `eveillee = false` et pose un répit, puis
    /// ré-arbitre pour tenter de remplir la place ainsi libérée.
    pub fn echec_de_reveil(&mut self, session: &str, maintenant: Instant) -> Vec<(String, Ordre)> {
        let Some(entree) = self.entrees.get_mut(session) else {
            // Un signal peut arriver d'un enfant dont la fenêtre vient d'être
            // retirée. Ignorer, jamais paniquer.
            return Vec::new();
        };
        entree.eveillee = false;
        entree.dernier_echec = Some(maintenant);
        self.arbitrer(maintenant)
    }

    /// Ré-arbitrage périodique, appelé par le fil de `sommeil.rs`.
    ///
    /// **Indispensable, et pas un luxe** : sous hystérésis, une fenêtre qui
    /// demande à veiller peut être refusée. Elle est alors déjà visible et
    /// déjà focalisée — aucun signal ne viendra plus la débloquer, et elle
    /// dormirait pour toujours sans ce tour de roue.
    pub fn rearbitrer(&mut self, maintenant: Instant) -> Vec<(String, Ordre)> {
        self.arbitrer(maintenant)
    }

    pub fn eveillee(&self, session: &str) -> Option<bool> {
        self.entrees.get(session).map(|e| e.eveillee)
    }

    /// Le cœur : calcule l'ensemble cible des éveillées, et en déduit les
    /// transitions. **Idempotent** — appelé deux fois de suite sans changement
    /// d'état ni de temps, il ne rend rien la seconde fois.
    ///
    /// **Ordre du vecteur rendu** : tous les `Dormir` précèdent tout
    /// `Reveiller`, chaque groupe étant trié par nom de session. Un réveil
    /// appliqué avant le sommeil qu'il finance demanderait transitoirement un
    /// encodeur de plus que le plafond.
    fn arbitrer(&mut self, maintenant: Instant) -> Vec<(String, Ordre)> {
        let mut ordres_dormir = Vec::new();
        let mut ordres_reveiller = Vec::new();

        // 1. Toute éveillée devenue invisible s'endort. Sans hystérésis : le
        //    masquage est explicite.
        let masquees: Vec<String> = self
            .entrees
            .iter()
            .filter(|(_, e)| e.eveillee && !e.visible)
            .map(|(nom, _)| nom.clone())
            .collect();
        for nom in masquees {
            if let Some(e) = self.entrees.get_mut(&nom) {
                e.eveillee = false;
            }
            ordres_dormir.push((nom, Ordre::Dormir(Raison::Masquee)));
        }

        // 2. Les épinglées : éveillées, encore visibles, et réveillées depuis
        //    moins que l'hystérésis. Elles occupent leur place quoi qu'il
        //    arrive.
        let epinglees: Vec<String> = self
            .entrees
            .iter()
            .filter(|(_, e)| {
                e.eveillee
                    && e.visible
                    && maintenant.saturating_duration_since(e.eveillee_depuis) < self.hysteresis
            })
            .map(|(nom, _)| nom.clone())
            .collect();

        // 3. Les candidates : toutes les visibles, sauf celles en répit après
        //    échec, de la plus récemment vue à la plus ancienne. Un ordre total
        //    est nécessaire pour que le résultat ne dépende pas du parcours
        //    d'une table de hachage : à récence égale, le nom départage.
        let mut candidates: Vec<(String, Instant)> = self
            .entrees
            .iter()
            .filter(|(_, e)| {
                e.visible
                    && e.dernier_echec.map_or(true, |t| {
                        maintenant.saturating_duration_since(t) >= REPIT_APRES_ECHEC
                    })
            })
            .map(|(nom, e)| (nom.clone(), e.dernier_vu))
            .collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        // 4. L'ensemble cible : les épinglées d'abord, puis les candidates les
        //    plus récentes jusqu'à remplir le plafond.
        let mut cible: Vec<String> = epinglees.clone();
        for (nom, _) in candidates {
            if cible.len() >= self.plafond {
                break;
            }
            if !cible.contains(&nom) {
                cible.push(nom);
            }
        }

        // 5. Les transitions.
        let noms: Vec<String> = self.entrees.keys().cloned().collect();
        for nom in noms {
            let doit_veiller = cible.contains(&nom);
            let Some(e) = self.entrees.get_mut(&nom) else { continue };
            if doit_veiller && !e.eveillee {
                e.eveillee = true;
                e.eveillee_depuis = maintenant;
                e.dernier_echec = None;
                ordres_reveiller.push((nom, Ordre::Reveiller));
            } else if !doit_veiller && e.eveillee {
                e.eveillee = false;
                ordres_dormir.push((nom, Ordre::Dormir(Raison::Evincee)));
            }
        }

        // Trier chaque groupe pour déterminisme total.
        ordres_dormir.sort_by(|a, b| a.0.cmp(&b.0));
        ordres_reveiller.sort_by(|a, b| a.0.cmp(&b.0));

        // Rendus dans l'ordre : tous les dormir avant tous les reveiller.
        ordres_dormir.extend(ordres_reveiller);
        ordres_dormir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    /// Un vivier de 2 places, sans hystérésis, pour que les tests d'éviction
    /// n'aient pas à faire vieillir le temps.
    fn petit() -> Vivier {
        Vivier::nouveau(2, Duration::ZERO)
    }

    #[test]
    fn une_fenetre_inscrite_est_endormie_tant_qu_elle_n_est_pas_visible() {
        let mut v = petit();
        let ordres = v.inscrire("a", t0());
        assert!(ordres.is_empty(), "une inscription seule n'ordonne rien : {ordres:?}");
        assert_eq!(v.eveillee("a"), Some(false));
    }

    #[test]
    fn une_fenetre_visible_est_reveillee_quand_il_reste_de_la_place() {
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        let ordres = v.signaler("a", true, true, t);
        assert_eq!(ordres, vec![("a".to_string(), Ordre::Reveiller)]);
        assert_eq!(v.eveillee("a"), Some(true));
    }

    #[test]
    fn une_fenetre_masquee_s_endort_avec_la_raison_masquee() {
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        let ordres = v.signaler("a", false, false, t + Duration::from_secs(1));
        assert_eq!(ordres, vec![("a".to_string(), Ordre::Dormir(Raison::Masquee))]);
        assert_eq!(v.eveillee("a"), Some(false));
    }

    #[test]
    fn au_dela_du_plafond_la_moins_recemment_vue_est_evincee() {
        let mut v = petit();
        let t = t0();
        for (i, nom) in ["a", "b", "c"].iter().enumerate() {
            let quand = t + Duration::from_millis(i as u64 * 100);
            v.inscrire(nom, quand);
            v.signaler(nom, true, true, quand);
        }
        // « a » a été vue en premier, donc la plus anciennement vue des trois.
        assert_eq!(v.eveillee("a"), Some(false), "a devait être évincée");
        assert_eq!(v.eveillee("b"), Some(true));
        assert_eq!(v.eveillee("c"), Some(true));
    }

    #[test]
    fn l_eviction_porte_la_raison_evincee_et_non_masquee() {
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        v.inscrire("b", t + Duration::from_millis(100));
        v.signaler("b", true, true, t + Duration::from_millis(100));
        v.inscrire("c", t + Duration::from_millis(200));
        let ordres = v.signaler("c", true, true, t + Duration::from_millis(200));
        assert!(
            ordres.contains(&("a".to_string(), Ordre::Dormir(Raison::Evincee))),
            "l'éviction doit être motivée, pas confondue avec une mise en veille voulue : {ordres:?}"
        );
        assert!(ordres.contains(&("c".to_string(), Ordre::Reveiller)));
    }

    #[test]
    fn un_focus_rafraichit_la_recence_et_protege_de_l_eviction() {
        let mut v = petit();
        let t = t0();
        for (i, nom) in ["a", "b"].iter().enumerate() {
            let quand = t + Duration::from_millis(i as u64 * 100);
            v.inscrire(nom, quand);
            v.signaler(nom, true, true, quand);
        }
        // « a » reprend le focus : elle redevient la plus récemment vue.
        v.signaler("a", true, true, t + Duration::from_millis(500));
        v.inscrire("c", t + Duration::from_millis(600));
        v.signaler("c", true, true, t + Duration::from_millis(600));
        assert_eq!(v.eveillee("a"), Some(true), "a venait d'être focalisée");
        assert_eq!(v.eveillee("b"), Some(false), "b est la plus anciennement vue");
    }

    #[test]
    fn l_hysteresis_empeche_d_evincer_une_fenetre_tout_juste_reveillee() {
        let mut v = Vivier::nouveau(1, Duration::from_secs(2));
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        assert_eq!(v.eveillee("a"), Some(true));

        // « b » demande à veiller 500 ms plus tard : « a » est protégée.
        v.inscrire("b", t + Duration::from_millis(500));
        let ordres = v.signaler("b", true, true, t + Duration::from_millis(500));
        assert!(ordres.is_empty(), "rien ne doit bouger sous l'hystérésis : {ordres:?}");
        assert_eq!(v.eveillee("a"), Some(true));
        assert_eq!(v.eveillee("b"), Some(false));
    }

    #[test]
    fn passee_l_hysteresis_la_rearbitration_periodique_debloque_le_reveil() {
        let mut v = Vivier::nouveau(1, Duration::from_secs(2));
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        v.inscrire("b", t + Duration::from_millis(500));
        v.signaler("b", true, true, t + Duration::from_millis(500));

        // Sans ce ré-arbitrage, « b » attendrait un signal qui ne viendra
        // jamais : elle est déjà visible et déjà focalisée.
        let ordres = v.rearbitrer(t + Duration::from_secs(3));
        assert!(ordres.contains(&("a".to_string(), Ordre::Dormir(Raison::Evincee))));
        assert!(ordres.contains(&("b".to_string(), Ordre::Reveiller)));
    }

    #[test]
    fn une_fenetre_masquee_s_endort_meme_sous_l_hysteresis() {
        // Se masquer est un geste EXPLICITE de l'utilisateur : l'hystérésis
        // protège contre le battement d'éviction, jamais contre une volonté.
        let mut v = Vivier::nouveau(2, Duration::from_secs(10));
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        let ordres = v.signaler("a", false, false, t + Duration::from_millis(10));
        assert_eq!(ordres, vec![("a".to_string(), Ordre::Dormir(Raison::Masquee))]);
    }

    #[test]
    fn retirer_une_fenetre_eveillee_rend_sa_place_a_une_endormie() {
        let mut v = petit();
        let t = t0();
        for (i, nom) in ["a", "b", "c"].iter().enumerate() {
            let quand = t + Duration::from_millis(i as u64 * 100);
            v.inscrire(nom, quand);
            v.signaler(nom, true, true, quand);
        }
        assert_eq!(v.eveillee("a"), Some(false));
        let ordres = v.retirer("c", t + Duration::from_secs(1));
        assert_eq!(ordres, vec![("a".to_string(), Ordre::Reveiller)]);
        assert_eq!(v.eveillee("c"), None, "une session retirée n'a plus d'état");
    }

    #[test]
    fn un_signal_pour_une_session_inconnue_est_ignore_sans_paniquer() {
        let mut v = petit();
        let ordres = v.signaler("fantome", true, true, t0());
        assert!(ordres.is_empty());
        assert_eq!(v.eveillee("fantome"), None);
    }

    #[test]
    fn un_ordre_n_est_jamais_emis_deux_fois_pour_le_meme_etat() {
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        // Second signal identique : la fenêtre est déjà éveillée.
        let ordres = v.signaler("a", true, true, t + Duration::from_millis(50));
        assert!(ordres.is_empty(), "un état inchangé n'ordonne rien : {ordres:?}");
    }

    #[test]
    fn un_reveil_qui_echoue_rend_l_entree_endormie_et_ne_la_réélit_pas_au_tour_suivant() {
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        assert_eq!(v.eveillee("a"), Some(true), "a était éveillée");

        // L'encodeur échoue à se construire : enregistrer l'échec.
        let ordres = v.echec_de_reveil("a", t + Duration::from_millis(100));
        assert_eq!(v.eveillee("a"), Some(false), "a doit s'endormir après l'échec");
        // Aucun nouvel ordre ne doit être émis : a était déjà le seul éveillé.
        assert!(ordres.is_empty(), "pas de réélection au tour même de l'échec : {ordres:?}");
    }

    #[test]
    fn passé_le_répit_un_rearbitrer_repropose_bien_la_fenetre_en_echec() {
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        v.echec_de_reveil("a", t + Duration::from_millis(100));
        assert_eq!(v.eveillee("a"), Some(false), "a est endormie après l'échec");

        // Avant le répit : pas de reproposition.
        let ordres = v.rearbitrer(t + Duration::from_millis(200));
        assert!(
            ordres.is_empty(),
            "avant le répit, a n'est pas reproposée : {ordres:?}"
        );
        assert_eq!(v.eveillee("a"), Some(false));

        // Après le répit : reproposition.
        let ordres = v.rearbitrer(t + Duration::from_millis(700));
        assert_eq!(
            ordres,
            vec![("a".to_string(), Ordre::Reveiller)],
            "après le répit, a doit être réveillée : {ordres:?}"
        );
        assert_eq!(v.eveillee("a"), Some(true));
    }

    #[test]
    fn les_ordres_rendus_placent_tous_les_dormir_avant_tout_reveiller() {
        // Cas où on éteint une fenêtre et on en allume une autre
        // simultanément : vérifier que Dormir précède Reveiller.
        let mut v = petit();
        let t = t0();
        for (i, nom) in ["a", "b", "c"].iter().enumerate() {
            let quand = t + Duration::from_millis(i as u64 * 100);
            v.inscrire(nom, quand);
            v.signaler(nom, true, true, quand);
        }
        // État : a endormi, b et c éveillés.
        assert_eq!(v.eveillee("a"), Some(false));
        assert_eq!(v.eveillee("b"), Some(true));
        assert_eq!(v.eveillee("c"), Some(true));

        // Faire arriver une quatrième fenêtre : d > c > b > a en récence.
        v.inscrire("d", t + Duration::from_millis(300));
        let ordres = v.signaler("d", true, true, t + Duration::from_millis(300));

        // Ordres attendus : b s'endort (Dormir), d s'éveille (Reveiller).
        // Ou plus largement, les Dormir avant les Reveiller.
        let premiers_dormir = ordres.iter().position(|(_, o)| matches!(o, Ordre::Dormir(_)));
        let premier_reveiller =
            ordres.iter().position(|(_, o)| matches!(o, Ordre::Reveiller));
        if let (Some(d_idx), Some(r_idx)) = (premiers_dormir, premier_reveiller) {
            assert!(
                d_idx < r_idx,
                "tous les Dormir doivent précéder tout Reveiller : {ordres:?}"
            );
        }
    }

    #[test]
    fn une_fenetre_en_repit_reste_exclue_des_candidates_jusqu_au_repit_ecoulé() {
        // Une fenêtre qui échoue à s'éveiller reste en répit et n'est pas
        // reproposée même si une place se libère, jusqu'à ce que le répit
        // s'écoule.
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        v.inscrire("b", t + Duration::from_millis(100));
        v.signaler("b", true, true, t + Duration::from_millis(100));

        // Échec du reveil de « a » à t + 200.
        v.echec_de_reveil("a", t + Duration::from_millis(200));
        // « b » se voit masquer à t + 300 (100 ms après l'échec), libérant une place.
        // Mais « a » est encore en répit (le répit dure 500 ms).
        let ordres = v.signaler("b", false, false, t + Duration::from_millis(300));
        assert!(
            !ordres.iter().any(|(nom, _)| nom == "a"),
            "a ne doit pas être réveillée car elle est en répit depuis seulement 100 ms : {ordres:?}"
        );
        assert_eq!(v.eveillee("a"), Some(false));
    }
}
