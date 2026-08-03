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

    /// Les sessions actuellement éveillées, dans un ordre non spécifié.
    ///
    /// Lu par `sommeil.rs` pour alimenter le répartiteur de débit (D6) : la
    /// part d'une fenêtre dépend de son éveil, et le vivier est la seule
    /// source de vérité sur ce point.
    pub fn eveillees(&self) -> Vec<String> {
        self.entrees
            .iter()
            .filter(|(_, entree)| entree.eveillee)
            .map(|(session, _)| session.clone())
            .collect()
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

/// Les tests vivent dans un fichier voisin : ce fichier-ci approche le plafond
/// de 500 lignes du projet, que cette extraction prévient de franchir. Le module
/// de tests demeure entièrement compilé et exécuté.
#[cfg(test)]
mod tests;
