//! Les tables du registre : le struct `Etat` et rien d'autre.
//!
//! **Extrait de `registre.rs` dans le round de correction 3 (25 août 2026),
//! AVANT d'y écrire** : le fichier était à 470 lignes pour un plafond de
//! projet à 500, et ce round y porte la cadence de la trace et deux
//! corrections de rédaction. Extraire, jamais comprimer — et dans une tâche
//! DÉDIÉE, avant celle qui ajoute. C'est la troisième extraction de cette
//! série (`file/tests.rs` au round 1, `parts/tests.rs` au round 2), et la
//! seconde de `registre.rs` après `registre/tour_de_roue.rs`.
//!
//! **Ce qui a guidé la coupe** : `Etat` est un AGRÉGAT DE TABLES, presque
//! entièrement fait de documentation — chacun de ses champs porte la raison
//! d'être d'une table et les défauts qu'elle a coûtés. `registre.rs` garde ce
//! qui AGIT sur elles : `etat()`, `distribuer`, `oublier`, `inscrire`,
//! `retirer`.
//!
//! ⚠️ **Le module s'appelle `tables` et non `etat`, à dessein** :
//! `registre.rs` porte déjà une fonction `etat()`, et si Rust distingue sans
//! peine un module d'une fonction, un lecteur humain trébuche. Le nom dit ce
//! que le fichier contient.
//!
//! ⚠️ **Transposition, pas réécriture** : le bloc est déplacé à l'identique,
//! aucun champ, aucun type, aucun commentaire n'a changé. Seule la VISIBILITÉ
//! est réécrite — `pub(super)` valait « visible dans `sommeil` » depuis
//! `registre` ; depuis `registre::tables` il faudrait deux crans, et la portée
//! est donc écrite en toutes lettres (`pub(in crate::capteur::sommeil)`)
//! plutôt qu'élargie à `pub(crate)`, qui serait PLUS LARGE que l'original.

use std::collections::HashMap;
use std::time::Instant;

use crate::capteur::vivier::Vivier;
use crate::capteur::sommeil::file::EmetteurSession;

pub(in crate::capteur::sommeil) struct Etat {
    pub(in crate::capteur::sommeil) vivier: Vivier,
    /// Le bout ÉMETTEUR du canal de chaque session.
    ///
    /// 🔴 **`EmetteurSession` et non `Sender<Message>` depuis le 25 août
    /// 2026** : le canal `mpsc` était NON BORNÉ, et une fenêtre qui cesse de
    /// lire faisait croître sa file sans terme. Le contrat qui compte ici est
    /// INCHANGÉ — `envoyer` rend `Err` quand le receveur est tombé, et c'est
    /// sur cette valeur que `distribuer` et `parts::distribuer_les_parts`
    /// purgent une session morte. Voir `file.rs` pour la borne, la
    /// coalescence, le compte des refus et sa trace.
    pub(in crate::capteur::sommeil) canaux: HashMap<String, EmetteurSession>,
    /// La session que le client déclare focalisée, si elle existe encore.
    ///
    /// Tenue ici et non dans `Vivier` : le vivier arbitre des places
    /// d'encodeur, le répartiteur des parts de débit. Le client émet `blur`
    /// aussi bien que `focus` (`client/src/visibilite.ts`), donc ce champ se
    /// vide bien quand la fenêtre perd le focus.
    pub(in crate::capteur::sommeil) focalisee: Option<String>,
    /// Dernière part envoyée à chaque session. **Le seul rempart contre une
    /// inondation** : le tour de roue ré-arbitre toutes les 250 ms, et sans
    /// cette mémoire huit fenêtres recevraient 32 messages par seconde à vie.
    pub(in crate::capteur::sommeil) dernieres_parts: HashMap<String, u32>,
    /// PID du processus propriétaire de chaque fenêtre. **Ici et pas dans un
    /// second registre** : le capteur n'a qu'une vérité à tenir, et deux
    /// tables à synchroniser en feraient deux.
    pub(in crate::capteur::sommeil) pids: HashMap<String, u32>,
    /// Rang d'arrivée de chaque session, et rang du dernier focus reçu. Deux
    /// compteurs tirés du même `horloge`, strictement croissante.
    pub(in crate::capteur::sommeil) arrivees: HashMap<String, u64>,
    pub(in crate::capteur::sommeil) derniers_focus: HashMap<String, u64>,
    /// Compteur monotone qui sert de rang aux deux tables ci-dessus. Un
    /// `Instant` ne conviendrait pas : il faut un ordre total, stable et
    /// comparable, pas une durée.
    pub(in crate::capteur::sommeil) horloge: u64,
    /// Dernier ordre audio envoyé à chaque session. **Le rempart contre
    /// l'inondation**, exactement comme `dernieres_parts` : le tour de roue
    /// ré-arbitre toutes les 250 ms.
    pub(in crate::capteur::sommeil) derniers_audio: HashMap<String, bool>,
    /// Instant après lequel une session dont la capture audio est morte
    /// redevient éligible au portage. Absente = apte.
    ///
    /// **Ici et pas dans `capteur::audio`** : ce module a l'horloge, l'autre
    /// est pur et le reste.
    pub(in crate::capteur::sommeil) inaptes: HashMap<String, Instant>,
    /// La DERNIÈRE annonce de presse-papier distribuée, quelle qu'elle soit.
    ///
    /// 🔴 **C'est la moitié AGENT du legs n°3 de P1** — « une fenêtre attachée
    /// après une copie ne reçoit jamais ce contenu ». Sans cette mémoire, une
    /// fenêtre qui s'attache attend la copie SUIVANTE, et le `Sondeur` le dit
    /// de lui-même : son premier tour prend l'état courant pour référence et
    /// n'annonce rien.
    ///
    /// ⚠️ **Elle mémorise AUSSI les `Annonce::Refus`, et il le faut** : une
    /// fenêtre qui s'attache après un refus doit voir le bandeau, sans quoi
    /// elle attendrait un contenu qui n'arrivera jamais.
    ///
    /// 🔴 **AUCUNE PURGE À LA RÉ-INSCRIPTION, et la symétrie avec
    /// `dernieres_parts` / `derniers_audio` est TROMPEUSE** (D-P3-3). Ces
    /// deux-là se purgent parce que `distribuer_les_parts` et
    /// `distribuer_l_audio` FILTRENT sur eux : sans purge, une part identique
    /// à celle envoyée sur l'ANCIEN canal serait jugée déjà livrée sur le
    /// canal NEUF, qui ne l'a jamais reçue. L'émission du presse-papier à
    /// l'inscription, elle, est INCONDITIONNELLE : il n'y a rien à filtrer,
    /// donc rien à purger — et purger ici retirerait la mémoire au moment
    /// précis où l'on veut s'en servir, le remède ne remédiant alors à rien.
    pub(in crate::capteur::sommeil) dernier_presse_papier: Option<crate::presse_papier::Annonce>,
    /// Le couple (numéro de séquence, texte) de NOTRE PROPRE écriture du
    /// presse-papier, en attente d'être consommé par le tour de roue pour
    /// armer les gardes n°1 et n°2 de D5 (sous-bloc P2).
    ///
    /// 🔴 **Il vit ICI, sous le verrou, et non à côté du `Sondeur`, parce que
    /// les deux ne courent pas sur le même fil.** Le `Sondeur` est local au fil
    /// du tour de roue ; l'écriture, elle, arrive du fil de FENÊTRE qui sert la
    /// commande `PressePapierEcrire`. Il n'existe aucun moyen d'armer le garde
    /// depuis là sans course — sinon ce registre, qui est déjà le point de
    /// rendez-vous verrouillé des deux.
    ///
    /// ⚠️ **Cela DÉPLACE la course, cela ne la supprime pas, et il faut le
    /// dire** : jusqu'à `PERIODE_REARBITRAGE` (250 ms) peut s'écouler entre
    /// notre `SetClipboardData` et la consommation ci-dessous. Si une AUTRE
    /// copie survient dans cet intervalle, poser `reference` sur *notre* `seq`
    /// ne la masque pas — le compteur aura encore bougé, et cette copie sera
    /// annoncée. **C'est le comportement voulu**, exact au sens de D5, et un
    /// test le vérifie plutôt que de le supposer.
    ///
    /// Écrasement du dernier : deux écritures en moins d'un tour de roue ne
    /// laissent que la seconde, qui est celle que le presse-papier porte
    /// réellement.
    pub(in crate::capteur::sommeil) notre_ecriture: Option<(u32, String)>,
    /// Nombre de réarmements consécutifs déjà accordés à chaque session.
    ///
    /// ❌ **« Remis à zéro dès qu'elle porte le son sans mourir » décrit la
    /// sémantique que le sous-bloc D10 a précisément RETIRÉE** (relevé par la
    /// revue transverse : ce fichier n'a pas été touché par la branche, d'où
    /// le résidu). La remise à zéro sur la **décision** d'arbitrage a quitté
    /// `sommeil/porteurs.rs` ; `signaler_audio_vivant` (`capteur/sommeil.rs`)
    /// en est désormais le seul point, et il ne court que sur une **PREUVE**
    /// — un paquet réel, remonté par `VersCapteur::AudioVivant`. C'est le
    /// leg 6 de D9, et c'était son objet : le compteur comptait des échecs
    /// non consécutifs.
    ///
    /// **Conséquence assumée, à connaître** : une session peut « porter le
    /// son sans mourir » et ne jamais voir son compteur retomber, si aucun
    /// paquet n'arrive jamais. C'est voulu — c'est exactement l'état que la
    /// recette ② de D10 a trouvé en production (une source reconstruite qui
    /// naissait muette) et que le compteur doit dénoncer, pas absoudre.
    pub(in crate::capteur::sommeil) rearmements: HashMap<String, u32>,
    /// Génération de la dernière inscription connue de chaque session (D9,
    /// F5 de D7 — course au `retirer` quand un nom se réinscrit). Posée par
    /// `inscrire`, lue et effacée par `retirer` via `retirer_est_perime`.
    ///
    /// **Volontairement absente d'`oublier`** : c'est `retirer` seul qui la
    /// purge, et seulement quand il n'est pas périmé. La purger depuis
    /// `oublier` la ferait disparaître aussi sur les chemins de canal rompu,
    /// qui n'ont aucune génération à comparer et ne doivent donc jamais
    /// l'effacer à la place d'un rattachement déjà inscrit. **Conséquence
    /// assumée** : sur ces chemins-là, l'entrée d'un nom survit à sa session
    /// pour le reste de la vie du processus capteur — sans effet
    /// fonctionnel (elle ne fait que dormir dans une `HashMap`), et les noms
    /// de session n'étant jamais réemployés (`Table::compteur`,
    /// `superviseur/table.rs`), cette table ne fait que croître avec le
    /// nombre de fenêtres jamais ouvertes sur la durée de vie du capteur.
    pub(in crate::capteur::sommeil) generations: HashMap<String, u64>,
    /// Compteur qui frappe la génération de chaque `inscrire` — **DISTINCT
    /// de `horloge` ci-dessus, délibérément**.
    ///
    /// `horloge` a une invariant que `generations` viole nécessairement :
    /// `arrivees`/`derniers_focus` ne sont posées qu'à la PREMIÈRE
    /// inscription d'un nom (`if !garde.arrivees.contains_key(session)`) et
    /// restent ensuite STABLES tant que le nom vit — c'est ce qui leur donne
    /// un sens de rang d'ARRIVÉE. `generations` a l'exigence inverse :
    /// **chaque** appel à `inscrire`, y compris un rattachement sous le même
    /// nom, doit recevoir une valeur NEUVE — c'est le seul moyen de
    /// distinguer l'instance vivante de la précédente. Faire porter cette
    /// exigence par `horloge` demanderait de le frapper inconditionnellement
    /// à `inscrire`, donc de désolidariser sa progression de la garde qui
    /// protège `arrivees`/`derniers_focus` — un couplage qui rendrait cette
    /// exigence dépendante d'une logique écrite pour un autre besoin, et
    /// silencieusement cassable par une évolution future de cette garde.
    /// Deux compteurs, deux invariants, aucun risque de confusion.
    pub(in crate::capteur::sommeil) prochaine_generation: u64,
}
