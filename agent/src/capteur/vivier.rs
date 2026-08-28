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

    /// Annule la mutation d'état qu'un ordre a produite, quand cet ordre
    /// **n'a pas pu être déposé** dans la file de sa fenêtre.
    ///
    /// 🔴 POURQUOI CETTE MÉTHODE EXISTE — LE SIXIÈME SITE DE MÉMORISATION,
    /// TROUVÉ AU ROUND DE CORRECTION 2 (25 août 2026). `arbitrer` écrit
    /// `eveillee` **AVANT que l'ordre parte** (étapes 1 et 5). C'est
    /// exactement le patron de `dernieres_parts` et `derniers_audio` que le
    /// round 1 a corrigé dans `sommeil/` — mais sur la seule variante que la
    /// fenêtre APPLIQUE au lieu de la relayer, et avec un coût dans les DEUX
    /// sens :
    ///
    /// - **`Reveiller` refusé** : `eveillee` reste `true`, la place du vivier
    ///   est occupée sans qu'aucun encodeur réel ne l'occupe, et `arbitrer`
    ///   étant idempotent, **aucun ré-arbitrage futur ne réémet l'ordre** —
    ///   mesuré : dix `rearbitrer` de suite ne rendent rien. C'est une fenêtre
    ///   qui ne se réveille plus, pour la vie du processus.
    /// - **`Dormir` refusé** : `eveillee` passe à `false` **définitivement**
    ///   alors que la fenêtre tient toujours son encodeur — et **plus aucun
    ///   arbitrage ne la réordonnera**, puisque le vivier la croit déjà
    ///   endormie. ⚠️ **C'est la moitié la plus coûteuse, et c'est celle qu'on
    ///   avait manquée** : il existe un `echec_de_reveil` pour le premier
    ///   sens, il n'existe **aucun** `echec_de_sommeil`.
    ///
    /// 🔴 **CE QUE CETTE MÉTHODE N'EMPÊCHE PAS, ET QUI A ÉTÉ SUR-AFFIRMÉ**
    /// (round de correction 3) : elle n'empêche **pas** la sur-souscription du
    /// plafond d'encodeurs. `arbitrer` libère le créneau à l'étape 1, élit la
    /// remplaçante à l'étape 4 et émet son `Reveiller` à l'étape 5 — **tout
    /// dans la même passe, avant que le dépôt ne soit seulement tenté** ;
    /// l'annulation ne court qu'après. Mesuré : `eveillees()` monte bien à 9
    /// pour un plafond de 8. **Ce qu'elle obtient est que cette
    /// sur-souscription soit TRANSITOIRE au lieu de permanente** — au
    /// ré-arbitrage suivant, le vivier voit 9 > 8 et rendort quelqu'un, là où
    /// sans elle il ne verrait jamais 9 et laisserait la dérive s'installer.
    /// Le test
    /// `sommeil::tests_refus::une_sur_souscription_par_un_dormir_non_depose_est_resorbee_au_tour_suivant`
    /// la mesure dans les deux temps.
    ///
    /// 🔴 LE REMÈDE EST « NE PAS MENTIR », PAS « RETENTER ». L'état
    /// redevient celui d'AVANT l'ordre, donc le vivier décrit à nouveau la
    /// réalité ; le prochain arbitrage voit la fenêtre dans son ancien état et
    /// **réémet l'ordre de lui-même**. Rien n'est retenté à l'intérieur de
    /// `distribuer`, donc **la terminaison de sa boucle n'est pas touchée** —
    /// c'est le tour de roue suivant qui reprend.
    ///
    /// ⚠️ **CE QUE L'ANNULATION NE RESTAURE PAS, et il faut le dire** : sur un
    /// `Reveiller`, `arbitrer` a posé `dernier_echec = None`, et la valeur
    /// d'avant n'est pas mémorisée. Elle n'est donc pas rendue. La
    /// conséquence est **voulue** : la session redevient candidate sans
    /// répit, ce qui est précisément ce qu'on cherche — que le prochain
    /// arbitrage la réélise et réémette son ordre. `eveillee_depuis`, lui,
    /// n'est lu que sur une entrée éveillée : le remettre serait sans effet.
    /// Sur un `Dormir`, l'annulation est exacte — `arbitrer` n'y touche
    /// qu'`eveillee`.
    ///
    /// **Sans effet si la session a disparu entre-temps**, jamais une panique :
    /// c'est le régime de `echec_de_reveil` juste au-dessus, et pour la même
    /// raison.
    ///
    /// **Ne ré-arbitre PAS et ne rend aucun ordre**, à la différence de
    /// `echec_de_reveil` : elle est appelée DEPUIS la boucle de
    /// `sommeil::registre::distribuer`, qui est en train de distribuer un lot.
    /// Y engendrer un lot de plus ferait dépendre sa terminaison d'un chemin
    /// que sa preuve ne couvre pas.
    pub fn annuler_ordre_non_livre(&mut self, session: &str, ordre: Ordre) {
        let Some(entree) = self.entrees.get_mut(session) else {
            return;
        };
        match ordre {
            Ordre::Reveiller => entree.eveillee = false,
            Ordre::Dormir(_) => entree.eveillee = true,
        }
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
    /// Lu par `sommeil/parts.rs` pour alimenter le répartiteur de débit (D6) :
    /// la part d'une fenêtre dépend de son éveil, et le vivier est la seule
    /// source de vérité sur ce point. Le présent est bien le temps juste — ce
    /// lecteur existe depuis la tâche 4 du sous-bloc.
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
