//! La file des messages d'une session, BORNÉE PAR COALESCENCE.
//!
//! 🔴 POURQUOI CE MODULE EXISTE. Le canal du registre était un
//! `std::sync::mpsc::channel()` NON BORNÉ, et quatre sous-blocs y ont ajouté
//! chacun une variante. Trois d'entre elles sont poussées « au changement
//! seulement » et n'ont de valeur que dans leur DERNIÈRE occurrence : sous
//! coalescence, elles cessent de faire croître la file en régime permanent,
//! quelle que soit la cadence d'arrivée.
//!
//! 🔴 LA COALESCENCE CONSERVE LA POSITION, ET C'EST L'INVARIANT DU CANAL.
//! `sommeil.rs` écrit qu'au sein d'une session, un canal unique garantit
//! l'ORDRE DE LIVRAISON entre les variantes. Remplacer en place préserve cet
//! ordre ; déplacer en queue ferait franchir à une part de débit un ordre de
//! dormir déposé entre-temps, et livrerait la part APRÈS l'ordre qui aurait dû
//! la rendre caduque.
//!
//! ⚠️ SÉPARER LES VARIANTES EN CANAUX DISTINCTS DÉTRUIRAIT CET INVARIANT.
//! C'est la solution qui vient d'abord à l'esprit, et elle est fausse.
//!
//! ⚠️ ~~CE MODULE EST PUR : il ne connaît ni verrou, ni fil, ni Windows.~~
//! **DEVENU FAUX quand ce module a reçu le CANAL lui-même** (`Partage`,
//! `EmetteurSession`, `ReceveurSession`, plus bas) : il connaît désormais un
//! `Mutex` et un `Arc`. Barré plutôt qu'effacé, comme ce dépôt le fait
//! partout. **Ce qui reste vrai, et qui était l'intention** : il ne connaît
//! toujours ni Windows, ni aucun `#[cfg]` — `capteur/sommeil.rs` n'est pas
//! gaté, donc tout ce fichier se compile et s'éprouve sur l'hôte Linux par
//! `cargo test --workspace`. La RÈGLE (`deposer`, `coalescable`) est restée
//! pure, elle : elle prend une `VecDeque` et rien d'autre.
//!
//! 🔴 CE MODULE REMPLACE `std::sync::mpsc::channel()`, ET IL DOIT EN REPRODUIRE
//! UN COMPORTEMENT PRÉCIS : `envoyer` rend `Err` quand le receveur est tombé.
//! C'est là-dessus, et sur rien d'autre, que `registre.rs::distribuer` et
//! `parts::distribuer_les_parts` PURGENT une session morte. Un `envoyer` qui
//! rendrait toujours `Ok` ne casserait aucun test de ce fichier et laisserait
//! les sessions mortes s'accumuler au vivier, en silence, pour la vie du
//! processus capteur.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::Message;

/// La profondeur au-delà de laquelle un dépôt est REFUSÉ.
///
/// ⚠️ **NON CALIBRÉE.** Aucune constante de ce dépôt ne l'est. Elle est choisie
/// assez grande pour qu'un régime normal ne l'atteigne jamais — les variantes
/// coalescables n'y contribuent pas — et assez petite pour que la mémoire reste
/// bornée si un enfant cesse de lire.
pub(crate) const PROFONDEUR_MAX: usize = 64;

/// Ce qu'un dépôt a fait.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Depot {
    /// Ajouté en queue.
    Empilee,
    /// A remplacé, EN PLACE, un message de la même variante déjà en attente.
    Coalescee,
    /// La file était pleine. **Rien n'a été ajouté, rien n'a été retiré.**
    Refusee,
}

/// Cette variante peut-elle remplacer une occurrence en attente d'elle-même ?
///
/// 🔴 LA RÈGLE EST « SA PERTE COÛTE-T-ELLE QUELQUE CHOSE ? », PAS « EST-ELLE
/// FRÉQUENTE ? ». `Part` et `Audio` sont poussées au changement seulement et
/// n'ont de valeur que dans leur dernière occurrence. `Sommeil` porte un ORDRE,
/// `PressePapier` porte la DONNÉE DE L'UTILISATEUR : ni l'un ni l'autre ne se
/// remplace.
///
/// ⚠️ TOUTE VARIANTE NEUVE DOIT PASSER ICI, et le `match` est EXHAUSTIF pour
/// que le compilateur l'exige — jamais un `_ => false`, qui la classerait
/// « à conserver » en silence et laisserait la file recroître.
pub(crate) fn coalescable(m: &Message) -> bool {
    match m {
        Message::Part { .. } | Message::Audio { .. } => true,
        Message::Sommeil(_) | Message::PressePapier { .. } => false,
    }
}

/// Deux messages sont-ils de la même variante ?
fn meme_variante(a: &Message, b: &Message) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Dépose un message, en appliquant la politique de sa variante.
pub(crate) fn deposer(file: &mut VecDeque<Message>, message: Message) -> Depot {
    if coalescable(&message) {
        if let Some(place) = file.iter().position(|en_attente| meme_variante(en_attente, &message)) {
            file[place] = message;
            return Depot::Coalescee;
        }
    }
    if file.len() >= PROFONDEUR_MAX {
        return Depot::Refusee;
    }
    file.push_back(message);
    Depot::Empilee
}

/// L'état partagé d'une session : sa file, et le compte de ses refus.
///
/// **Exactement deux détenteurs, jamais plus** : l'émetteur et le receveur.
/// Ni `EmetteurSession` ni `ReceveurSession` n'est `Clone`, et **cela n'est
/// pas un oubli** — c'est ce qui donne son sens au `Arc::strong_count`
/// d'`envoyer` (voir sa doc). Rendre l'un des deux clonable romprait la
/// détection du receveur tombé **sans qu'aucun test ne bronche**.
struct Partage {
    file: Mutex<VecDeque<Message>>,
    /// Compte CUMULÉ des dépôts refusés de cette session. **Par session, et
    /// c'est le point** : c'est ce qui permet de dire *laquelle* déborde.
    refuses: AtomicU64,
}

/// Le bout par lequel le registre écrit à une fenêtre.
pub(crate) struct EmetteurSession {
    partage: Arc<Partage>,
}

/// Le bout par lequel le fil de fenêtre lit. **Rendu par `inscrire`.**
pub(crate) struct ReceveurSession {
    partage: Arc<Partage>,
}

/// Pourquoi une réception n'a rien rendu.
///
/// La distinction reprend celle de `std::sync::mpsc::TryRecvError`
/// (`Empty` / `Disconnected`) que ce couple remplace : `transitions.rs`
/// traitait les deux cas différemment dans son commentaire, et les confondre
/// effacerait cette distinction.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum VideOuFerme {
    /// La file est VIDE — l'émetteur vit encore, il n'a rien déposé.
    Vide,
    /// Plus personne n'écrit : l'émetteur est tombé, et la file est épuisée.
    Ferme,
}

/// Le couple d'une session. **Un émetteur, un receveur, et jamais davantage.**
pub(crate) fn canal_de_session() -> (EmetteurSession, ReceveurSession) {
    let partage = Arc::new(Partage {
        file: Mutex::new(VecDeque::new()),
        refuses: AtomicU64::new(0),
    });
    (
        EmetteurSession { partage: Arc::clone(&partage) },
        ReceveurSession { partage },
    )
}

/// Prend le verrou d'une file, **sans jamais paniquer**.
///
/// 🔴 CE QU'IL ADVIENT D'UN VERROU EMPOISONNÉ : **on reprend l'état tel quel
/// et on continue**, jamais un `unwrap()`. Trois raisons, dans cet ordre :
///
/// ① **Le capteur tient TOUTES les fenêtres.** Une panique ici, sur le fil du
/// tour de roue ou sur un fil de fenêtre, emporterait le canal de chacune des
/// N sessions, pas seulement celui de la session fautive.
///
/// ② **L'état reste cohérent par construction.** Rien de ce qui court sous ce
/// verrou ne peut paniquer en laissant la `VecDeque` à moitié écrite :
/// `deposer` n'y fait qu'un `position`, une écriture indexée et un
/// `push_back`, et `essayer_recevoir` un `pop_front`. Un empoisonnement ne
/// pourrait venir que d'une panique d'un AUTRE fil pendant qu'il tient ce
/// verrou — au pire un message de plus ou de moins en file.
///
/// ③ **Le dépôt a déjà ce précédent, et il est nommé** :
/// `registre.rs::etat()` fait le même `unwrap_or_else(|e| e.into_inner())`,
/// pour la même raison, écrite au même endroit.
fn sous_verrou<T>(
    verrou: &Mutex<VecDeque<Message>>,
    action: impl FnOnce(&mut VecDeque<Message>) -> T,
) -> T {
    let mut file = verrou.lock().unwrap_or_else(|empoisonne| empoisonne.into_inner());
    action(&mut file)
}

impl EmetteurSession {
    /// Dépose un message pour la fenêtre, et dit ce qu'il en est advenu.
    ///
    /// 🔴 `Err(())` SIGNIFIE « LE RECEVEUR EST TOMBÉ », et **c'est le contrat
    /// que `std::sync::mpsc::Sender::send` donnait avant ce module**.
    /// `registre.rs::distribuer` et `parts::distribuer_les_parts` purgent une
    /// session morte sur cette valeur, et **rien d'autre ne la purge sur ce
    /// chemin** : la faire rendre `Ok` inconditionnellement laisserait les
    /// sessions mortes occuper une place au vivier pour la vie du processus.
    ///
    /// ⚠️ **COMMENT ON LE SAIT : `Arc::strong_count(&self.partage) == 1`.**
    /// C'est le SEUL signal disponible — il n'y a plus de `mpsc` pour le
    /// donner. Il ne vaut que parce que `canal_de_session` crée exactement
    /// deux détenteurs et qu'aucun des deux bouts n'est `Clone` : `1` veut
    /// alors dire « je suis seul », donc « le receveur a été laissé choir ».
    ///
    /// ⚠️ **Un refus n'est PAS une erreur** : il rend `Ok(Depot::Refusee)`.
    /// Les appelants du registre ne jugent que « rompu ou non », et confondre
    /// les deux ferait purger une session bien vivante dont la file déborde —
    /// c'est-à-dire tuer l'arbitrage de la fenêtre la plus en peine.
    #[allow(clippy::result_unit_err)]
    pub(crate) fn envoyer(&self, message: Message) -> Result<Depot, ()> {
        if Arc::strong_count(&self.partage) == 1 {
            return Err(());
        }
        let depot = sous_verrou(&self.partage.file, |file| deposer(file, message));
        if depot == Depot::Refusee {
            let refuses = self.partage.refuses.fetch_add(1, Ordering::Relaxed) + 1;
            self.journaliser_le_refus(refuses);
        }
        Ok(depot)
    }

    /// Le compte CUMULÉ des dépôts refusés de cette session.
    pub(crate) fn refuses(&self) -> u64 {
        self.partage.refuses.load(Ordering::Relaxed)
    }

    /// Journalise un refus **AU FRANCHISSEMENT D'UN PALIER, jamais à chaque
    /// refus** — et le palier retenu est la **puissance de deux** du compte
    /// cumulé (1, 2, 4, 8, 16…).
    ///
    /// 🔴 POURQUOI PAS UNE TRACE PAR REFUS. « Ne jamais tracer par paquet dans
    /// la boucle de transport » : 18 619 lignes en quelques secondes sur un
    /// partage CIFS ont déjà empêché une session de s'établir. Une fenêtre
    /// bloquée reçoit un message tous les `PERIODE_REARBITRAGE` (250 ms) au
    /// minimum, et bien davantage sur un presse-papier actif — la trace par
    /// refus croîtrait sans borne avec la durée du blocage.
    ///
    /// 🔴 POURQUOI LE PALIER PLUTÔT QU'UNE TRACE « À L'ENTRÉE EN SATURATION ».
    /// L'alternative — tracer la transition « ne refusait pas → refuse » —
    /// n'est PAS bornée : une file qui oscille autour de `PROFONDEUR_MAX` la
    /// franchit à chaque tour de roue, et l'on retombe sur une ligne toutes
    /// les 250 ms pour la durée du blocage. Le compte cumulé, lui, est
    /// monotone : **au plus 64 lignes pour toute la vie d'une session**, quoi
    /// qu'il arrive, et la ligne porte le compte, donc l'ampleur reste
    /// lisible sans qu'on ait à compter les lignes.
    ///
    /// ⚠️ Le nom de la session n'est PAS ici : ce module ne le connaît pas, et
    /// le lui donner ferait porter au canal une identité qui appartient au
    /// registre. Le span de l'appelant l'attribue.
    fn journaliser_le_refus(&self, refuses: u64) {
        if refuses.is_power_of_two() {
            tracing::warn!(
                refuses,
                profondeur_max = PROFONDEUR_MAX,
                "file d'une session pleine : message REFUSE (trace au palier, puissance de deux)"
            );
        }
    }
}

impl ReceveurSession {
    /// Retire le plus ancien message en attente, sans jamais bloquer.
    ///
    /// **Le seul appel de PRODUCTION** (`transitions.rs::appliquer_les_ordres`,
    /// en boucle jusqu'à `Err`). ⚠️ **Aucune variante bloquante n'est livrée,
    /// et c'est délibéré** : le relevé de surface n'a trouvé ni `recv()` ni
    /// `recv_timeout()` sur ce canal, ni en production ni dans les tests. Une
    /// méthode bloquante demanderait une `Condvar` que personne n'appellerait,
    /// donc un mécanisme que le produit n'exerce jamais.
    ///
    /// ⚠️ **`Ferme` ne se rend qu'une fois la file ÉPUISÉE** : ce qui a été
    /// déposé avant la chute de l'émetteur se lit encore, comme le faisait
    /// `mpsc`. Jeter ces messages perdrait un ordre de sommeil déjà décidé.
    pub(crate) fn essayer_recevoir(&self) -> Result<Message, VideOuFerme> {
        match sous_verrou(&self.partage.file, |file| file.pop_front()) {
            Some(message) => Ok(message),
            None if Arc::strong_count(&self.partage) == 1 => Err(VideOuFerme::Ferme),
            None => Err(VideOuFerme::Vide),
        }
    }

    /// Retire et rend TOUT ce qui attend, dans l'ordre.
    ///
    /// Le remplaçant de `Receiver::try_iter().collect()`, dont les suites de
    /// `parts`, `porteurs` et `presse_papier` se servent pour lire le DERNIER
    /// message d'une variante. **En une seule prise de verrou** plutôt qu'une
    /// par message.
    pub(crate) fn vider(&self) -> Vec<Message> {
        sous_verrou(&self.partage.file, |file| file.drain(..).collect())
    }
}

// Module de tests extrait dans un fichier voisin : ce fichier était à 445
// lignes pour un plafond de projet à 500, et le round de correction 1 y
// ajoute du code et de la doc. Extraire, jamais comprimer — et dans une
// tâche DÉDIÉE, avant celle qui ajoute. Même montage et même idiome que
// `superviseur/table.rs` ; voir la doc en tête du fichier extrait pour
// pourquoi ce `#[path]` ne relève PAS de la convention `<parent>_<enfant>`.
#[cfg(test)]
#[path = "file/tests.rs"]
mod tests;
