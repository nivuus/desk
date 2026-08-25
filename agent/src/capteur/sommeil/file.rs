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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capteur::sommeil::{Message, Ordre};
    use std::collections::VecDeque;

    /// 🔴 LE CŒUR DE LA DÉCISION : deux `Part` sans lecture n'en laissent
    /// qu'UNE, et c'est la DERNIÈRE valeur qui survit.
    #[test]
    fn deux_parts_se_coalescent_en_une_seule() {
        let mut f = VecDeque::new();
        assert!(matches!(deposer(&mut f, Message::Part { bps: 1 }), Depot::Empilee));
        assert!(matches!(deposer(&mut f, Message::Part { bps: 2 }), Depot::Coalescee));
        assert_eq!(f.len(), 1);
        assert!(matches!(f[0], Message::Part { bps: 2 }));
    }

    /// 🔴 LE CRITÈRE QUI DISTINGUE LA COALESCENCE EN PLACE DE CELLE EN QUEUE,
    /// et c'est l'invariant que `sommeil.rs` écrit : au sein d'une session, le
    /// canal garantit l'ORDRE DE LIVRAISON entre les variantes. Coalescer en
    /// queue ferait franchir à la part un ordre de dormir déposé entre-temps.
    #[test]
    fn la_coalescence_conserve_la_position() {
        let mut f = VecDeque::new();
        deposer(&mut f, Message::Part { bps: 1 });
        deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
        deposer(&mut f, Message::Part { bps: 2 });
        assert_eq!(f.len(), 2);
        assert!(matches!(f[0], Message::Part { bps: 2 }), "la part garde sa PLACE");
        assert!(matches!(f[1], Message::Sommeil(Ordre::Reveiller)));
    }

    /// Un ordre perdu laisse une fenêtre endormie ou éveillée à tort.
    #[test]
    fn un_ordre_de_sommeil_n_est_jamais_coalesce() {
        let mut f = VecDeque::new();
        deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
        assert!(matches!(deposer(&mut f, Message::Sommeil(Ordre::Reveiller)), Depot::Empilee));
        assert_eq!(f.len(), 2);
    }

    /// Un presse-papier perdu, c'est la donnée de l'utilisateur.
    #[test]
    fn un_presse_papier_n_est_jamais_coalesce() {
        let mut f = VecDeque::new();
        deposer(&mut f, Message::PressePapier { texte: Some("a".into()), octets: 1 });
        let d = deposer(&mut f, Message::PressePapier { texte: Some("b".into()), octets: 1 });
        assert!(matches!(d, Depot::Empilee));
        assert_eq!(f.len(), 2);
    }

    /// La borne dure REFUSE, elle ne tronque pas en silence.
    #[test]
    fn au_dela_de_la_borne_le_depot_est_refuse() {
        let mut f = VecDeque::new();
        for _ in 0..PROFONDEUR_MAX {
            deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
        }
        assert!(matches!(deposer(&mut f, Message::Sommeil(Ordre::Reveiller)), Depot::Refusee));
        assert_eq!(f.len(), PROFONDEUR_MAX, "la file n'a pas grossi");
    }

    /// 🔴 LE TÉMOIN NÉGATIF, ET SA GARANTIE EXACTE : **une fois qu'une
    /// occurrence de la variante est DÉJÀ en file**, un dépôt de cette
    /// variante ne bute jamais sur la borne, quelle que soit la cadence — il
    /// coalesce, donc il ne teste même pas la borne. Sans lui, « refusée »
    /// au-dessus ne dirait pas que la coalescence borne réellement.
    ///
    /// ⚠️ **LA GARANTIE N'EST PAS PLUS LARGE QUE CELA, et le nom d'origine
    /// (`une_variante_coalescable_ne_bute_jamais_sur_la_borne`) SUR-AFFIRMAIT.**
    /// Le PREMIER dépôt d'une variante coalescable, lui, s'empile comme les
    /// autres et se heurte à la borne si la file est pleine d'incoalescables :
    /// c'est le cas que mesure `un_premier_depot_coalescable_bute_bien_sur_la_borne`
    /// juste en dessous. Ce n'est pas un défaut — le refus est explicite,
    /// jamais une troncature — mais une sur-affirmation est la classe de
    /// défaut que ce dépôt combat en premier.
    #[test]
    fn une_variante_deja_en_file_ne_bute_jamais_sur_la_borne() {
        let mut f = VecDeque::new();
        for i in 0..(PROFONDEUR_MAX * 10) {
            let d = deposer(&mut f, Message::Part { bps: i as u32 });
            assert!(!matches!(d, Depot::Refusee));
        }
        assert_eq!(f.len(), 1);
    }

    /// 🔴 CE QUE LA REVUE DE LA TÂCHE PRÉCÉDENTE A MESURÉ, et que le témoin
    /// ci-dessus ne dit pas : le PREMIER `Part` déposé sur une file pleine de
    /// variantes INCOALESCABLES n'a rien à remplacer, donc il s'empile — donc
    /// il est REFUSÉ. **Ce n'est pas un défaut** : le refus est explicite et
    /// compté, jamais une troncature silencieuse. C'est la borne de la
    /// garantie, et elle est désormais éprouvée plutôt que supposée.
    #[test]
    fn un_premier_depot_coalescable_bute_bien_sur_la_borne() {
        let mut f = VecDeque::new();
        for _ in 0..PROFONDEUR_MAX {
            deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
        }
        assert!(matches!(deposer(&mut f, Message::Part { bps: 1 }), Depot::Refusee));
        assert_eq!(f.len(), PROFONDEUR_MAX, "la file n'a pas grossi");
    }

    /// Le couple se comporte comme le canal qu'il remplace : ce qu'on dépose
    /// se reçoit, dans l'ordre.
    #[test]
    fn ce_qui_est_depose_se_recoit_dans_l_ordre() {
        let (e, r) = canal_de_session();
        e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
        e.envoyer(Message::PressePapier { texte: Some("a".into()), octets: 1 }).unwrap();
        assert!(matches!(r.essayer_recevoir(), Ok(Message::Sommeil(_))));
        assert!(matches!(r.essayer_recevoir(), Ok(Message::PressePapier { .. })));
        assert_eq!(r.essayer_recevoir(), Err(VideOuFerme::Vide));
    }

    /// 🔴 LE REFUS EST COMPTÉ. Un refus qui ne se compte pas est un refus
    /// qu'aucune exploitation ne verra jamais.
    #[test]
    fn les_refus_se_comptent() {
        let (e, _r) = canal_de_session();
        for _ in 0..PROFONDEUR_MAX {
            e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
        }
        assert_eq!(e.refuses(), 0, "aucun refus tant que la borne n'est pas atteinte");
        e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
        assert_eq!(e.refuses(), 1);
    }

    /// L'émetteur sait que plus personne ne lit.
    ///
    /// 🔴 C'EST LE TEST QUI TIENT LA PURGE DES SESSIONS MORTES du registre :
    /// `distribuer` retire une session sur `envoyer(...).is_err()`, et rien
    /// d'autre ne le fait sur ce chemin.
    #[test]
    fn un_receveur_tombe_ferme_l_emetteur() {
        let (e, r) = canal_de_session();
        drop(r);
        assert!(e.envoyer(Message::Sommeil(Ordre::Reveiller)).is_err());
    }

    /// Le receveur distingue « rien à lire » de « plus personne n'écrit ».
    #[test]
    fn un_emetteur_tombe_se_distingue_d_une_file_vide() {
        let (e, r) = canal_de_session();
        assert_eq!(r.essayer_recevoir(), Err(VideOuFerme::Vide));
        e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
        drop(e);
        // Ce qui reste en file se lit ENCORE : la fermeture ne jette rien.
        assert!(matches!(r.essayer_recevoir(), Ok(Message::Sommeil(_))));
        assert_eq!(r.essayer_recevoir(), Err(VideOuFerme::Ferme));
    }

    /// `vider` rend ce qui attend, dans l'ordre, et laisse la file vide.
    #[test]
    fn vider_rend_tout_ce_qui_attend_dans_l_ordre() {
        let (e, r) = canal_de_session();
        e.envoyer(Message::Part { bps: 7 }).unwrap();
        e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
        let recus = r.vider();
        assert_eq!(recus.len(), 2);
        assert!(matches!(recus[0], Message::Part { bps: 7 }));
        assert!(matches!(recus[1], Message::Sommeil(Ordre::Reveiller)));
        assert!(r.vider().is_empty());
    }
}
