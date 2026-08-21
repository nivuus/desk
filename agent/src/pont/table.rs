//! Les commandes en vol : corrélation, expiration, annulation, sessions
//! d'énumération. **PUR** : aucun `cfg`, aucune horloge lue en interne — le
//! temps est un paramètre, ce qui rend l'expiration testable sans dormir.
//!
//! **Le défaut de l'ancien pont que ce module existe pour ne pas rejouer**
//! (spec §4.2) : un écouteur `message` était posé **par requête**
//! (`src/file.js:155`) et jamais retiré sur le chemin d'erreur (`:126-129`).
//! Une opération en échec laissait donc son écouteur à vie, et tous les
//! survivants ré-analysaient chaque message suivant — le coût croissait avec
//! le nombre d'échecs passés, indéfiniment. Ici il n'y a **qu'une** entrée par
//! commande, retirée par la première des trois issues : réponse, annulation,
//! expiration.
//!
//! **Une réponse tardive est JETÉE, jamais appliquée.** C'est l'invariant
//! central : [`resoudre`] rend `None` pour une corrélation annulée, expirée ou
//! inconnue. Appliquer une réponse dont la commande ProjFS a déjà été
//! complétée écrirait dans un tampon que le système a repris.
//!
//! [`resoudre`]: Table::resoudre

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// ⚠️ **NON CALIBRÉES.** Posées, pas mesurées.
///
/// ✅ **F4 A DONNÉ DES DISTRIBUTIONS, ET DEUX DES CINQ BUDGETS MORDENT
/// RÉELLEMENT.** `DELAI_LISTER` (20 s) solde tout listage au-delà de ~3 150
/// entrées, et `DELAI_LIRE` (5 s) solde toute lecture de quatre morceaux ou
/// plus, `MORCEAUX_EN_VOL = 4` faisant que ces quatre morceaux se partagent
/// ~33 Kio/s. `DELAI_ATTRIBUTS` (2 s) a une marge de deux ordres de grandeur
/// (traversées de 13 à 31 ms), et `DELAI_MUTATION` (15 s) aussi (125 à 193 ms).
/// ⚠️ **DONNER DE QUOI CALIBRER N'EST PAS CALIBRER** : F4 publie des
/// distributions, jamais des valeurs proposées — choisir un nombre demande un
/// jugement d'usage qu'aucun chantier de ce dépôt n'a jamais porté. Elles
/// rejoignent `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
/// `REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX` dans la liste des constantes
/// de ce dépôt qu'aucune mesure n'a jugées.
///
/// **Pourquoi TROIS budgets et non un** : l'ancien pont en avait **un seul**,
/// 10 s, pour tout (`src/file.js:89`). D'où deux défauts symétriques — des
/// lectures de gros blocs qui expiraient avant d'aboutir, et des `getattr` qui
/// figeaient l'Explorateur dix secondes sur un chemin inexistant. Un budget
/// unique ne peut pas être juste pour une opération qui doit répondre en
/// millisecondes et pour une qui transfère des mégaoctets.
///
/// ✅ **Le quatrième budget est arrivé : c'est [`DELAI_ECRIRE`], et F2 le pose.**
/// *(Cette ligne annonçait « il appartient à F2 » ; elle est corrigée ici
/// plutôt que laissée au futur, par la branche même qui la réalise.)*
pub const DELAI_ATTRIBUTS: Duration = Duration::from_secs(2);
pub const DELAI_LIRE: Duration = Duration::from_secs(5);
pub const DELAI_LISTER: Duration = Duration::from_secs(20);

/// Le budget d'un MORCEAU d'écriture — pas d'un fichier.
///
/// ⚠️ **NON CALIBRÉE**, comme les trois ci-dessus. Elle est plus large que
/// [`DELAI_LIRE`] pour une raison de forme, pas de mesure : le navigateur doit
/// **écrire** sur le disque du poste local, et le morceau `dernier` déclenche
/// en plus le `close()` de `createWritable()`, c'est-à-dire la committaison —
/// une copie du fichier d'échange vers sa destination, dont le coût croît avec
/// la taille du fichier et qu'aucune mesure de ce dépôt ne borne.
///
/// 🔴 **CE BUDGET NE PROTÈGE PERSONNE, et c'est ce qui le distingue des trois
/// autres.** Les leurs bornent l'attente d'une APPLICATION bloquée dans un
/// rappel ProjFS ; celui-ci borne l'attente du **fil d'écriture**, qui ne fait
/// attendre personne. Son dépassement ne rend aucun `HRESULT` : il laisse
/// l'entrée AU JOURNAL et la nomme.
pub const DELAI_ECRIRE: Duration = Duration::from_secs(30);

/// Le budget d'une MUTATION — un renommage ou une suppression.
///
/// ⚠️ **UN QUATRIÈME BUDGET, là où la spec §5.3 en pose trois, et c'est une
/// divergence DÉCLARÉE.** *(Le commentaire ci-dessus disait déjà « le quatrième
/// budget est arrivé : c'est `DELAI_ECRIRE` » — celui-ci est donc le
/// CINQUIÈME, et le compte de la spec a vieilli de deux sous-blocs.)*
///
/// ⚠️ **NON CALIBRÉE**, comme les quatre autres.
///
/// **Pourquoi il n'est ni celui d'une lecture ni celui d'une écriture** : une
/// mutation ne transporte **aucun octet** — c'est un seul aller-retour —, mais
/// son repli de copie, lui, est en O(taille) ET en O(nombre d'entrées) côté
/// navigateur, sur un répertoire qu'il faut recréer feuille à feuille. Le
/// budget d'une lecture (5 s) tuerait le renommage d'un répertoire profond ;
/// celui d'une écriture (30 s) figerait l'Explorateur une demi-minute sur un
/// simple `ren` refusé.
///
/// 🔴 **CELUI-CI PROTÈGE QUELQU'UN, à la différence de [`DELAI_ECRIRE`].** Une
/// mutation naît d'une notification POST — l'application a déjà rendu la
/// main —, **mais le `PRE_` qui la précède est SYNCHRONE** : l'Explorateur y
/// attend. Le budget borne donc bien l'attente d'une application, comme les
/// trois de F1 et à l'inverse de celui de l'écriture.
pub const DELAI_MUTATION: Duration = Duration::from_secs(15);

/// Ce qu'une commande en vol attend, et de quoi la réponse devra être
/// interprétée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attendue {
    Attributs {
        chemin: String,
    },
    Lire {
        chemin: String,
        position: u64,
        longueur: u32,
    },
    /// Un morceau d'écriture poussé vers le navigateur.
    ///
    /// ⚠️ **`dernier` est retenu ici parce que c'est lui qui décide de ce que
    /// le `Fait` signifie** : sur le dernier morceau, il vaut « le fichier est
    /// commis, l'entrée peut sortir du journal » ; sur les autres, seulement
    /// « demande le suivant ». Le relire de l'en-tête émis serait le relire
    /// d'une source que le pair aurait pu déformer.
    Ecrire {
        chemin: String,
        dernier: bool,
    },
    /// Une création d'entrée poussée vers le navigateur.
    Creer {
        chemin: String,
    },
    /// Une **mutation** poussée vers le navigateur (F3).
    ///
    /// ⚠️ **`chemin` est la SOURCE**, celle sur laquelle des écritures peuvent
    /// être dues. La destination d'un renommage vit dans l'en-tête émis, pas
    /// ici : la table n'a pas à la connaître pour apparier une réponse.
    Muter {
        chemin: String,
        /// `true` pour un renommage, `false` pour une suppression. **Ce qui en
        /// dépend est le JOURNAL**, jamais l'appariement — mais un journal qui
        /// ne dirait pas lequel des deux verbes a échoué renverrait le lecteur
        /// au code source.
        renommage: bool,
    },
    Lister {
        chemin: String,
        /// ⚠️ **Le GUID d'énumération du rappel, PAS le chemin** (spec §7.2).
        /// Deux applications qui listent le même répertoire en même temps
        /// ouvrent deux sessions distinctes sur le même chemin : indexer par
        /// chemin ferait que la seconde écraserait la première, et l'une des
        /// deux recevrait un répertoire vide.
        enumeration: [u8; 16],
    },
}

#[derive(Debug)]
struct EnVol {
    /// La commande ProjFS à compléter, **s'il y en a une**.
    ///
    /// 🔴 **`None` POUR UNE ÉCRITURE, et ce n'est pas un cas dégénéré : c'est
    /// la nature du write-back.** Une écriture ne complète AUCUN rappel — elle
    /// naît d'une notification POST, qui a déjà rendu la main à l'application.
    /// Il n'y a donc rien à compléter, et appeler `PrjCompleteCommand(0)` sur
    /// une commande inexistante serait un appel au système sur un identifiant
    /// qui appartient à quelqu'un d'autre.
    command_id: Option<i32>,
    quoi: Attendue,
    echeance: Instant,
    /// L'instant d'inscription.
    ///
    /// 🔴 **C'est ce qui rend le legs n°4 de F1 DIAGNOSTICABLE**, et rien
    /// d'autre ne le rendrait : F1 a mesuré des lectures qui CALENT sans jamais
    /// expirer — `commande expirée` reste à 0 pendant 540 s — et déclare qu'on
    /// ne sait pas OÙ le blocage se produit, « faute d'une trace à
    /// l'inscription en table ». L'échéance seule ne suffit pas : elle dit
    /// quand la commande mourra, jamais depuis combien de temps elle attend.
    inscrite_a: Instant,
}

/// Les commandes en vol, indexées par corrélation.
#[derive(Debug, Default)]
pub struct Table {
    en_vol: HashMap<u32, EnVol>,
    /// Prochaine corrélation à distribuer. Croît strictement, et **enjambe**
    /// toute valeur encore en vol au moment du rebouclage.
    prochaine: u32,
}

impl Table {
    pub fn nouvelle() -> Self {
        Self::default()
    }

    /// Départ de compteur injectable, **pour les tests seuls**.
    ///
    /// ⚠️ Sans cette couture, le test du rebouclage de `u32` serait
    /// **vacueux** : l'atteindre honnêtement demanderait quatre milliards
    /// d'inscriptions, et un test qu'on ne peut pas exécuter est un test qui
    /// n'existe pas. C'est le patron que D10 a attrapé quatre fois.
    #[cfg(test)]
    pub fn nouvelle_depuis(prochaine: u32) -> Self {
        Self { prochaine, ..Self::default() }
    }

    /// Inscrit une commande ProjFS et rend sa corrélation.
    pub fn inscrire(&mut self, command_id: i32, quoi: Attendue, echeance: Instant) -> u32 {
        self.inscrire_interne(Some(command_id), quoi, echeance)
    }

    /// Inscrit une opération qui ne complète **aucun** rappel ProjFS — une
    /// écriture — et rend sa corrélation.
    ///
    /// 🔴 **POURQUOI LA MÊME TABLE, ET NON UNE SECONDE SOURCE DE CORRÉLATIONS.**
    /// Le canal est unique, et la corrélation est un `u32` monotone avec
    /// recherche d'un libre. Deux compteurs indépendants sur le même canal se
    /// collisionneraient, et **la collision serait SILENCIEUSE** : une réponse
    /// appliquée à la mauvaise commande. C'est exactement le défaut que
    /// [`Table::corrélation_libre`] documente déjà contre le rebouclage —
    /// obtenir la corrélation d'ailleurs le rejouerait par la porte de derrière.
    pub fn inscrire_sans_commande(&mut self, quoi: Attendue, echeance: Instant) -> u32 {
        self.inscrire_interne(None, quoi, echeance)
    }

    fn inscrire_interne(
        &mut self,
        command_id: Option<i32>,
        quoi: Attendue,
        echeance: Instant,
    ) -> u32 {
        let correlation = self.corrélation_libre();
        // ⚠️ **`echeance` est déjà calculée par l'appelant depuis SON horloge**,
        // et l'instant d'inscription est pris ici : les deux viennent de la
        // même `Instant::now()` à quelques microsecondes près, et le module
        // reste pur — il ne lit pas l'heure pour DÉCIDER, seulement pour
        // HORODATER ce qu'il retient. Le faire passer en paramètre ferait un
        // troisième argument que tous les appelants poseraient à la même
        // valeur.
        let inscrite_a = Instant::now();
        self.en_vol.insert(correlation, EnVol { command_id, quoi, echeance, inscrite_a });
        correlation
    }

    /// La prochaine corrélation qui ne collide avec aucune commande en vol.
    ///
    /// Le compteur reboucle après `u32::MAX` : sans cette recherche, la
    /// corrélation rebouclée écraserait une commande encore en vol, et sa
    /// réponse serait appliquée à la mauvaise. La boucle se termine parce que
    /// la table est bornée par la mémoire, donc très en deçà de 2^32 entrées.
    fn corrélation_libre(&mut self) -> u32 {
        loop {
            let candidate = self.prochaine;
            self.prochaine = self.prochaine.wrapping_add(1);
            if !self.en_vol.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    /// Rend la commande d'une corrélation, ou `None` si elle a été annulée,
    /// expirée, ou n'a jamais existé — la réponse tardive est alors **jetée**.
    ///
    /// **Le troisième terme est l'ÂGE de la commande** : le temps écoulé entre
    /// son inscription et cet instant, c'est-à-dire **la traversée
    /// pont → navigateur → pont**. C'est ce que [`crate::pont::latence`]
    /// observe, et c'est tout ce que le pont sait mesurer — ni l'entrée dans le
    /// rappel, ni le balayage, ni `PrjCompleteCommand` n'y sont.
    ///
    /// ⚠️ **`maintenant` est un PARAMÈTRE**, comme partout dans ce module : le
    /// temps n'y est jamais lu, ce qui rend l'âge testable sans dormir. C'est
    /// la même discipline que [`Table::plus_ancienne`] et que
    /// [`Table::expirees`].
    ///
    /// ⚠️ **Une commande EXPIRÉE ne passe pas par ici** : `expirees` la retire
    /// elle-même. L'âge rendu est donc celui d'une traversée qui a **abouti**,
    /// et jamais celui d'un échec — les deux se lisent sur deux lignes de
    /// recensement distinctes, jamais l'une pour l'autre.
    pub fn resoudre(
        &mut self,
        correlation: u32,
        maintenant: Instant,
    ) -> Option<(Option<i32>, Attendue, Duration)> {
        self.en_vol
            .remove(&correlation)
            .map(|e| (e.command_id, e.quoi, maintenant.saturating_duration_since(e.inscrite_a)))
    }

    /// Annule la commande ProjFS `command_id`, et rend **TOUTES** ses
    /// corrélations.
    ///
    /// 🔴 **TOUTES, ET C'EST LA FENÊTRE DE LECTURE DE F3 QUI L'EXIGE.** Jusqu'à
    /// F2, une commande n'avait qu'UNE corrélation en vol — le morceau *n+1*
    /// n'étant demandé qu'à réception du *n* —, et cette fonction n'en rendait
    /// qu'une. Depuis F3, une lecture peut en avoir jusqu'à
    /// `pont::lecture::MORCEAUX_EN_VOL`.
    ///
    /// **Ce qu'une version qui n'en rendrait qu'une produirait :** les *N−1*
    /// autres resteraient en vol, expireraient au budget, et
    /// `service::balayer` appellerait alors `PrjCompleteCommand` sur une
    /// commande **DÉJÀ COMPLÉTÉE** — c'est-à-dire un appel au système sur un
    /// identifiant qui appartient désormais à quelqu'un d'autre. *Le pire des
    /// modes de défaillance : muet, différé, et hors de notre processus.*
    ///
    /// L'ordre des corrélations rendues est **déterministe** : un `HashMap` n'en
    /// a aucun, et un appelant qui les journaliserait produirait un ordre
    /// différent à chaque exécution.
    pub fn annuler(&mut self, command_id: i32) -> Vec<u32> {
        let mut correlations: Vec<u32> = self
            .en_vol
            .iter()
            .filter(|(_, e)| e.command_id == Some(command_id))
            .map(|(c, _)| *c)
            .collect();
        correlations.sort_unstable();
        for c in &correlations {
            self.en_vol.remove(c);
        }
        correlations
    }

    /// Retire et rend tout ce qui est échu à `maintenant`.
    ///
    /// L'échéance est **atteinte**, pas dépassée : une commande dont
    /// l'échéance vaut exactement `maintenant` est expirée. Le contraire ferait
    /// dépendre l'expiration de la granularité de l'horloge.
    pub fn expirees(&mut self, maintenant: Instant) -> Vec<(Option<i32>, u32)> {
        let echues: Vec<u32> = self
            .en_vol
            .iter()
            .filter(|(_, e)| e.echeance <= maintenant)
            .map(|(c, _)| *c)
            .collect();
        echues
            .into_iter()
            .map(|c| (self.en_vol.remove(&c).expect("relevée à l'instant").command_id, c))
            .collect()
    }

    /// Retire et rend TOUT. Appelée **avant** `PrjStopVirtualizing` : une
    /// commande laissée en vol y attendrait une réponse que plus rien ne peut
    /// délivrer, et ProjFS attendrait sa complétion indéfiniment.
    pub fn vider(&mut self) -> Vec<(Option<i32>, u32)> {
        let mut tout: Vec<(Option<i32>, u32)> =
            self.en_vol.drain().map(|(c, e)| (e.command_id, c)).collect();
        // Ordre déterministe : un `HashMap` n'en a aucun, et un appelant qui
        // journaliserait cette liste produirait un ordre différent à chaque
        // exécution.
        tout.sort_unstable_by_key(|(_, c)| *c);
        tout
    }

    pub fn en_vol(&self) -> usize {
        self.en_vol.len()
    }

    /// Depuis combien de temps la PLUS ANCIENNE commande en vol attend.
    ///
    /// 🔴 **C'EST CE QUI DÉPARTAGE LES QUATRE HYPOTHÈSES DU LEGS N°4 DE F1**,
    /// et aucune n'était départageable jusqu'ici :
    ///
    /// | Ce que le recensement montre | Ce que cela dit du blocage |
    /// | --- | --- |
    /// | `en vol=0` alors que l'application est figée | **rien n'a jamais été inscrit** : le blocage est dans le rappel, ou avant lui |
    /// | `en vol=N` et cette durée qui croît **au-delà du budget** | la table ne balaie plus : le fil du pont est sorti de sa boucle |
    /// | `en vol=N` et cette durée bornée par le budget | l'inscription et l'expiration marchent : le blocage est ailleurs |
    /// | plus aucune ligne de recensement | **le fil du pont est mort**, ce que rien ne disait |
    ///
    /// `None` quand rien n'est en vol — et c'est la première ligne du tableau.
    pub fn plus_ancienne(&self, maintenant: Instant) -> Option<Duration> {
        self.en_vol
            .values()
            .map(|e| maintenant.saturating_duration_since(e.inscrite_a))
            .max()
    }

    /// Combien de commandes en vol n'ont **aucun** rappel ProjFS à compléter —
    /// c'est-à-dire les écritures et les mutations.
    ///
    /// ⚠️ **Le distinguer du total n'est pas une coquetterie** : une
    /// application figée avec `en vol=3` et `sans_commande=3` n'attend RIEN du
    /// pont — les trois sont des poussées, et son blocage est ailleurs.
    pub fn sans_commande(&self) -> usize {
        self.en_vol.values().filter(|e| e.command_id.is_none()).count()
    }
}

#[cfg(test)]
mod tests;
