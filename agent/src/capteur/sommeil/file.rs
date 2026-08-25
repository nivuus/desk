//! La file des messages d'une session, BORNÉE PAR COALESCENCE.
//!
//! 🔴 POURQUOI CE MODULE EXISTE. Le canal du registre était un
//! `std::sync::mpsc::channel()` NON BORNÉ, et quatre sous-blocs y ont ajouté
//! chacun une variante. Trois d'entre elles sont poussées « au changement
//! seulement » et n'ont de valeur que dans leur DERNIÈRE occurrence : sous
//! coalescence, elles cessent de faire croître la file en régime permanent,
//! quelle que soit la cadence d'arrivée.
//!
//! 🔴 LA COALESCENCE CONSERVE LA POSITION DU CRÉNEAU — ELLE NE CONSERVE PAS
//! L'ORDRE D'ARRIVÉE DES VALEURS, ET LA DISTINCTION EST TOUT LE SUJET.
//!
//! ❌ ~~Remplacer en place préserve l'ordre de livraison ; déplacer en queue
//! ferait franchir à une part de débit un ordre de dormir déposé entre-temps,
//! et livrerait la part APRÈS l'ordre qui aurait dû la rendre caduque.~~
//! **CET ARGUMENT ÉTAIT FAUX, et le round de correction 1 l'a réfuté** :
//! coalescer en QUEUE placerait toujours la valeur la plus récente en queue,
//! donc `Part(dormante), Sommeil(Reveiller), Part(éveillée)` y rendrait
//! `[Reveiller, Part(éveillée)]` — chronologiquement juste ET portant la
//! bonne valeur. Le mode de défaillance décrit n'existe pas. Barré plutôt
//! qu'effacé.
//!
//! **CE QUI EST VRAI, ET QUI TIENT LA DÉCISION.** Remplacer en place conserve
//! la POSITION du créneau : la file garde exactement autant d'entrées, aux
//! mêmes places. Elle ne conserve PAS l'ordre d'arrivée des valeurs — la
//! valeur d'éveillée est livrée à la place qu'occupait celle de dormante,
//! donc AVANT le `Sommeil` arrivé entre les deux. **C'est acceptable parce que
//! le capteur RELAIE ces deux variantes sans les appliquer** :
//! `fenetre/transitions.rs` écrit « rien à faire localement » pour `Part` et
//! pour `Audio` — seul `Sommeil` a un effet local. Les deux politiques
//! convergent donc vers le même état final, et c'est cet argument-là, pas le
//! précédent, qui justifie le choix.
//!
//! ⚠️ SÉPARER LES VARIANTES EN CANAUX DISTINCTS RESTE FAUX, pour une raison
//! qui, elle, n'a pas bougé : `Sommeil` a un effet local, et deux canaux
//! parallèles laisseraient un ordre de dormir doubler une part déjà livrée ou
//! l'inverse, sans qu'aucun ordre total n'existe entre eux. C'est la solution
//! qui vient d'abord à l'esprit, et elle est fausse.
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
//! 🔴 CE MODULE REMPLACE `std::sync::mpsc::channel()`, MAIS SON `envoyer` A
//! TROIS ISSUES LÀ OÙ `send` EN AVAIT DEUX — ET C'EST LE PIÈGE QUE LE ROUND
//! DE CORRECTION 1 A PAYÉ.
//!
//! Sous `mpsc`, `send(...).is_ok()` valait **« livré »**. Ici il ne vaudrait
//! plus que « pas déconnecté » : un refus de file pleine est un ÉCHEC DE
//! LIVRAISON sur une session parfaitement VIVANTE. Les cinq points d'appel de
//! production avaient gardé l'ancienne lecture, et deux d'entre eux
//! MÉMORISAIENT le refus comme un envoi (`dernieres_parts`,
//! `derniers_audio`), ce qui supprimait toute réémission future de cette
//! valeur — une fenêtre bloquée au débit précédent, ou muette, **sans
//! borne**.
//!
//! 🔴 C'EST POURQUOI `envoyer` NE REND PAS UN `Result` MAIS UN `Envoi` À
//! TROIS VARIANTES, `#[must_use]`, QUE CHAQUE APPELANT TRAITE PAR UN `match`
//! EXHAUSTIF. Le remède est MÉCANIQUE : le compilateur refuse un site qui
//! oublie un cas, et refusera de même toute variante future.
//! ⚠️ **`#[must_use]` sur `Depot` seul ne suffisait PAS, et cela a été
//! mesuré** : `Result` est lui-même `#[must_use]`, et `.is_ok()`, `.is_err()`
//! ou `let _ =` le consomment — ce qui éteint le `must_use` du `Depot` qu'il
//! contient. `cargo check` ne signalait AUCUN des cinq sites.

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
///
/// ⚠️ **`#[must_use]` est posé ici par principe — il ne garde PAS les appels
/// à `envoyer`.** Mesuré : enveloppé dans un `Result` (lui-même `must_use`),
/// il est éteint dès qu'on écrit `.is_ok()` ou `let _ =`. C'est `Envoi`,
/// plus bas, qui porte la garde réelle. Ce `must_use`-ci ne couvre que les
/// appels DIRECTS à `deposer`.
#[must_use]
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
    /// Compte CUMULÉ des dépôts refusés de cette session.
    refuses: AtomicU64,
    /// Le nom de la session, porté ICI et pour une seule raison : **rendre la
    /// trace du refus ATTRIBUABLE**.
    ///
    /// 🔴 ~~Le nom de la session n'est pas ici, le span de l'appelant
    /// l'attribue.~~ **FAUX, et le round de correction 1 l'a établi : IL
    /// N'EXISTE AUCUN SPAN.** Le seul `info_span!` du capteur est posé sur le
    /// fil de FENÊTRE (`capteur/fenetre.rs`) ; les cinq appels à `envoyer`
    /// courent soit sur le fil du tour de roue — aucun span —, soit, via
    /// `signaler`, **sous le span d'une AUTRE session**, `distribuer_les_parts`
    /// poussant à *toutes*. Le champ aurait alors été FAUX, ce qui est pire
    /// qu'absent. Un exploitant lisant `refuses=8` sans savoir de quelle
    /// session n'apprend rien.
    ///
    /// ⚠️ **La trace le publie sous le nom `session_cible`, PAS `session`**
    /// (round 2) : le span d'une autre session peut bel et bien envelopper
    /// cette ligne, et deux `session=` de valeurs différentes côte à côte
    /// rejoueraient la confusion d'un cran plus loin. Le nom distinct dit
    /// lequel des deux désigne la fenêtre qui déborde.
    session: String,
}

/// Le bout par lequel le registre écrit à une fenêtre.
pub(crate) struct EmetteurSession {
    partage: Arc<Partage>,
}

/// Le bout par lequel le fil de fenêtre lit. **Rendu par `inscrire`.**
pub(crate) struct ReceveurSession {
    partage: Arc<Partage>,
}

/// Ce qu'il est advenu d'un `envoyer`. **TROIS issues, jamais deux.**
///
/// 🔴 UN ENUM ET NON UN `Result<Depot, ()>`, ET C'EST LE REMÈDE MÉCANIQUE AU
/// CRITIQUE DU ROUND 1. Un `Result` invite à `.is_ok()` / `.is_err()`, qui
/// écrasent `Depose` et `Refuse` sur une seule valeur — c'est exactement la
/// confusion qui a coûté deux mémorisations fautives. Ici le compilateur
/// **exige** que chaque site nomme les trois cas, et le fera encore pour
/// toute variante ajoutée plus tard. Précédent du dépôt : le `match`
/// exhaustif de `transport/controle.rs`, qui « se signale au compilateur »,
/// par opposition au catch-all de `capteur/pont_media.rs` qui a tué un fil en
/// silence six fois.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Envoi {
    /// Le message est dans la file : il ATTEINDRA la fenêtre. Porte ce que le
    /// dépôt a fait (empilé, ou coalescé sur une occurrence en attente).
    Depose(Depot),
    /// La file était pleine : **rien n'a été déposé**, et le message est
    /// perdu. ⚠️ **La session est VIVANTE** — la purger serait tuer
    /// l'arbitrage de la fenêtre la plus en peine. Et **il ne faut pas non
    /// plus le compter comme livré** : tout appelant qui MÉMORISE ce qu'il a
    /// envoyé doit s'abstenir ici, sans quoi son garde d'écrasement supprime
    /// la réémission de cette valeur pour toujours.
    Refuse,
    /// Le receveur est tombé : la session est MORTE, il faut la PURGER. C'est
    /// le seul cas qui remplace l'ancien `send(...).is_err()`.
    Rompu,
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
///
/// `session` n'est retenu que pour rendre la trace du refus attribuable —
/// voir le champ `Partage::session`, qui dit pourquoi aucun span ne peut le
/// faire à sa place.
pub(crate) fn canal_de_session(session: &str) -> (EmetteurSession, ReceveurSession) {
    let partage = Arc::new(Partage {
        file: Mutex::new(VecDeque::new()),
        refuses: AtomicU64::new(0),
        session: session.to_string(),
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
    /// 🔴 `Envoi::Rompu` SIGNIFIE « LE RECEVEUR EST TOMBÉ », et **c'est le
    /// contrat que `std::sync::mpsc::Sender::send(...).is_err()` donnait avant
    /// ce module**. `registre.rs::distribuer`,
    /// `parts::distribuer_les_parts`, `porteurs::distribuer_l_audio` et
    /// `presse_papier::distribuer` purgent une session morte sur cette valeur,
    /// et **rien d'autre ne la purge sur ces chemins** : ne jamais la rendre
    /// laisserait les sessions mortes occuper une place au vivier pour la vie
    /// du processus.
    ///
    /// ⚠️ **COMMENT ON LE SAIT : `Arc::strong_count(&self.partage) == 1`.**
    /// C'est le SEUL signal disponible — il n'y a plus de `mpsc` pour le
    /// donner. Il ne vaut que parce que `canal_de_session` crée exactement
    /// deux détenteurs et qu'aucun des deux bouts n'est `Clone` : `1` veut
    /// alors dire « je suis seul », donc « le receveur a été laissé choir ».
    ///
    /// 🔴 **`Envoi::Refuse` N'EST NI UNE LIVRAISON NI UNE RUPTURE**, et c'est
    /// la troisième issue que `mpsc` n'avait pas. La confondre avec la
    /// première fait mémoriser un message jamais parti ; avec la seconde, elle
    /// purge une session bien vivante. Le `match` exhaustif qu'`Envoi` impose
    /// est ce qui empêche les deux.
    pub(crate) fn envoyer(&self, message: Message) -> Envoi {
        if Arc::strong_count(&self.partage) == 1 {
            return Envoi::Rompu;
        }
        match sous_verrou(&self.partage.file, |file| deposer(file, message)) {
            Depot::Refusee => {
                let refuses = self.partage.refuses.fetch_add(1, Ordering::Relaxed) + 1;
                self.journaliser_le_refus(refuses);
                Envoi::Refuse
            }
            depose => Envoi::Depose(depose),
        }
    }

    /// Le compte CUMULÉ des dépôts refusés de cette session.
    ///
    /// ⚠️ ~~**`#[cfg(test)]`**~~ **FAUX AU PRÉSENT — le gate est tombé au
    /// round 3, voir juste en dessous ; ce qui suit décrit l'état d'ALORS,
    /// au round 1.** C'était une correction du round 1 : elle n'avait AUCUN
    /// appelant de production (`method 'refuses' is never used` sur la
    /// cible Windows), alors que sa doc annonçait le bénéfice « dire LAQUELLE
    /// déborde ». Ce bénéfice était réalisé par la TRACE, qui porte désormais le
    /// nom de session ; cet accesseur n'existait que pour que le test puisse
    /// éprouver le compteur. Le gater était ce qui empêchait de réaffirmer un
    /// bénéfice d'exploitation qui n'existait pas.
    ///
    /// ✅ **LE GATE `#[cfg(test)]` EST TOMBÉ AU ROUND 3** : la méthode a
    /// désormais un appelant de PRODUCTION — `registre::distribuer` s'en sert
    /// pour cadencer sa propre trace sur le MÊME palier que
    /// `journaliser_le_refus`, de sorte que les deux lignes sortent ensemble.
    /// Le raisonnement qui l'avait gatée reste juste : on ne dégate pas pour
    /// faire joli, on dégate parce qu'un appelant est apparu.
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
    /// 🔴 ~~Le nom de la session n'est PAS ici ; le span de l'appelant
    /// l'attribue.~~ **CORRIGÉ AU ROUND 1 : IL N'EXISTE AUCUN SPAN**, et la
    /// trace n'était donc attribuable à personne. L'émetteur connaît sa
    /// session : il la porte, et la trace la nomme — sous `session_cible`, pour
    /// ne pas entrer en collision avec un span englobant. Voir
    /// `Partage::session`.
    fn journaliser_le_refus(&self, refuses: u64) {
        if refuses.is_power_of_two() {
            tracing::warn!(
                // `session_cible` et non `session` : ce champ peut se poser
                // SOUS le span `fenetre{session=…}` d'une AUTRE session — le
                // fil de fenêtre qui appelle `signaler` fait pousser à
                // TOUTES. Deux `session=` de valeurs différentes sur la même
                // ligne rejoueraient, d'un cran plus loin, la confusion que ce
                // champ vient supprimer.
                session_cible = %self.partage.session,
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
    ///
    /// ⚠️ **`#[cfg(test)]`, et c'est une correction du round 1** : ses six
    /// appelants sont TOUS des helpers de test (`method 'vider' is never used`
    /// sur la cible Windows). La production, elle, ne lit ce canal que par
    /// `essayer_recevoir`, en boucle.
    #[cfg(test)]
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
