//! Le registre lui-même : l'état global (`Etat`), son point d'accès
//! (`etat`), le tour de roue, et les quatre opérations qui le touchent
//! (`distribuer`, `oublier`, `inscrire`, `retirer`).
//!
//! **Extrait de `sommeil.rs` pour rester sous le plafond de 500 lignes du
//! projet** (revue de la tâche 10, D9) — même motif et même montage que
//! `parts.rs` et `porteurs.rs`, ses deux voisins déjà extraits pour cette
//! raison. `sommeil.rs` garde *ce par quoi on parle au registre* (`Message`,
//! la façade publique `signaler`/`audio_mort`/`echec_de_reveil`) ; ce
//! fichier-ci est *le registre lui-même*.
//!
//! **Transposition, pas réécriture** : ce fichier est le déplacement à
//! l'identique du bloc `Etat`..`retirer` de `sommeil.rs` — aucune valeur,
//! aucun ordre d'opération, aucune signature n'a changé, seule la visibilité
//! de `Etat`/`etat`/`distribuer`/`oublier` est passée à `pub(super)` pour
//! rester atteignable depuis `sommeil.rs` et ses autres descendants
//! (`parts.rs`, `porteurs.rs`, `tests.rs`), exactement comme avant
//! l'extraction.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use crate::capteur::vivier::{Ordre, Vivier, HYSTERESIS, PLAFOND_EVEIL};

use super::{parts, porteurs, purger_les_inaptitudes, retirer_est_perime, Message, PERIODE_REARBITRAGE};

pub(super) struct Etat {
    pub(super) vivier: Vivier,
    pub(super) canaux: HashMap<String, Sender<Message>>,
    /// La session que le client déclare focalisée, si elle existe encore.
    ///
    /// Tenue ici et non dans `Vivier` : le vivier arbitre des places
    /// d'encodeur, le répartiteur des parts de débit. Le client émet `blur`
    /// aussi bien que `focus` (`client/src/visibilite.ts`), donc ce champ se
    /// vide bien quand la fenêtre perd le focus.
    pub(super) focalisee: Option<String>,
    /// Dernière part envoyée à chaque session. **Le seul rempart contre une
    /// inondation** : le tour de roue ré-arbitre toutes les 250 ms, et sans
    /// cette mémoire huit fenêtres recevraient 32 messages par seconde à vie.
    pub(super) dernieres_parts: HashMap<String, u32>,
    /// PID du processus propriétaire de chaque fenêtre. **Ici et pas dans un
    /// second registre** : le capteur n'a qu'une vérité à tenir, et deux
    /// tables à synchroniser en feraient deux.
    pub(super) pids: HashMap<String, u32>,
    /// Rang d'arrivée de chaque session, et rang du dernier focus reçu. Deux
    /// compteurs tirés du même `horloge`, strictement croissante.
    pub(super) arrivees: HashMap<String, u64>,
    pub(super) derniers_focus: HashMap<String, u64>,
    /// Compteur monotone qui sert de rang aux deux tables ci-dessus. Un
    /// `Instant` ne conviendrait pas : il faut un ordre total, stable et
    /// comparable, pas une durée.
    pub(super) horloge: u64,
    /// Dernier ordre audio envoyé à chaque session. **Le rempart contre
    /// l'inondation**, exactement comme `dernieres_parts` : le tour de roue
    /// ré-arbitre toutes les 250 ms.
    pub(super) derniers_audio: HashMap<String, bool>,
    /// Instant après lequel une session dont la capture audio est morte
    /// redevient éligible au portage. Absente = apte.
    ///
    /// **Ici et pas dans `capteur::audio`** : ce module a l'horloge, l'autre
    /// est pur et le reste.
    pub(super) inaptes: HashMap<String, Instant>,
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
    pub(super) rearmements: HashMap<String, u32>,
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
    pub(super) generations: HashMap<String, u64>,
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
    pub(super) prochaine_generation: u64,
}

static ETAT: OnceLock<Mutex<Etat>> = OnceLock::new();

pub(super) fn etat() -> MutexGuard<'static, Etat> {
    let mutex = ETAT.get_or_init(|| {
        demarrer_le_tour_de_roue();
        Mutex::new(Etat {
            vivier: Vivier::nouveau(PLAFOND_EVEIL, HYSTERESIS),
            canaux: HashMap::new(),
            focalisee: None,
            dernieres_parts: HashMap::new(),
            pids: HashMap::new(),
            arrivees: HashMap::new(),
            derniers_focus: HashMap::new(),
            horloge: 0,
            derniers_audio: HashMap::new(),
            inaptes: HashMap::new(),
            rearmements: HashMap::new(),
            generations: HashMap::new(),
            prochaine_generation: 0,
        })
    });
    // Un empoisonnement ne doit pas tuer le capteur : l'état du vivier reste
    // cohérent (un `Vec` d'ordres perdu au pire), et refuser de servir serait
    // pire que de continuer.
    mutex.lock().unwrap_or_else(|empoisonne| empoisonne.into_inner())
}

/// **Un seul fil pour tout le processus**, démarré à la première inscription.
///
/// **La sûreté ne tient pas au `sleep` ci-dessous.** Ce fil est lancé DEPUIS la
/// fermeture d'initialisation de `ETAT.get_or_init` ; c'est
/// `OnceLock::get_or_init` lui-même qui garantit qu'un second fil appelant
/// `etat()` pendant que cette fermeture tourne encore **bloque** jusqu'à ce
/// qu'elle se termine — la réentrance qui paniquerait serait celle du *même*
/// fil, qui n'a pas lieu ici. Le `sleep` n'est qu'une cadence, pas une garde.
fn demarrer_le_tour_de_roue() {
    std::thread::spawn(|| loop {
        std::thread::sleep(PERIODE_REARBITRAGE);
        let mut garde = etat();
        let maintenant = Instant::now();
        let ordres = garde.vivier.rearbitrer(maintenant);
        distribuer(&mut garde, ordres);
        parts::distribuer_les_parts(&mut garde);
        purger_les_inaptitudes(&mut garde.inaptes, Instant::now());
        porteurs::distribuer_l_audio(&mut garde);
    });
}

/// Envoie chaque ordre à la fenêtre concernée. Un canal rompu signale une
/// fenêtre déjà morte : on retire son entrée plutôt que de la journaliser à
/// chaque tour de roue.
///
/// **En boucle jusqu'à épuisement, et pas un seul passage.** Retirer une
/// session éveillée libère sa place, et `arbitrer` peut alors élire une AUTRE
/// session en réponse — en posant `eveillee = true` sur elle EN INTERNE, dans
/// le même mouvement qui produit l'ordre `Reveiller` correspondant. Si ce
/// nouveau lot d'ordres n'était pas distribué à son tour, cette élection ne
/// serait qu'un artefact du modèle : `arbitrer` est idempotent, il la croit
/// déjà servie, et plus aucun ré-arbitrage futur — pas même le tour de roue —
/// ne réémettrait cet ordre. La place resterait occupée dans le vivier sans
/// qu'aucun encodeur réel ne l'occupe, pour toute la vie du processus.
///
/// **Terminaison** : un tour n'engendre un nouveau lot que s'il a détecté au
/// moins un canal rompu, et chaque canal rompu détecté est retiré de
/// `canaux` avant que le tour suivant ne commence. `canaux` est fini et
/// décroît strictement à chaque retrait ; le nombre de tours est donc borné
/// par le nombre de sessions inscrites.
pub(super) fn distribuer(garde: &mut MutexGuard<'static, Etat>, ordres: Vec<(String, Ordre)>) {
    let mut a_traiter = ordres;
    while !a_traiter.is_empty() {
        let mut suite = Vec::new();
        for (session, ordre) in a_traiter {
            let rompu = match garde.canaux.get(&session) {
                Some(canal) => canal.send(Message::Sommeil(ordre)).is_err(),
                None => false,
            };
            if rompu {
                suite.extend(oublier(garde, &session));
            }
        }
        a_traiter = suite;
    }
}

/// Oublie TOUT ce que le registre retient d'une session, et rend les ordres
/// que son retrait du vivier engendre.
///
/// **Le point de passage unique**, et c'est tout son intérêt : le registre
/// retient NEUF choses d'une session (son canal, sa dernière part, son PID,
/// son rang d'arrivée, son dernier focus reçu, son dernier ordre audio
/// envoyé, le focus courant si elle le porte, son inaptitude audio et son
/// compteur de réarmements — ces deux dernières depuis D9), et rend
/// séparément son entrée au vivier. Il en existe trois chemins de retrait —
/// la fermeture normale (`retirer`), la détection d'un canal rompu pendant la
/// distribution des ORDRES (`distribuer`), et la même détection pendant
/// celle des PARTS (`parts::distribuer_les_parts`).
///
/// ⚠️ **`focalisee` était le champ oublié par les deux derniers** (M1, revue
/// finale de branche du sous-bloc D6). Seule la fermeture normale le vidait.
/// Une session focalisée qui meurt par canal rompu laissait donc son nom dans
/// `focalisee` ; comme plus aucune fenêtre vivante ne porte ce nom, la
/// majoration `FACTEUR_FOCUS` cessait de s'appliquer à quiconque — sans
/// différence observable, puisqu'elle ne s'appliquait déjà à personne
/// d'autre. **La conséquence qui MORD est ailleurs, et elle est atteignable** :
/// un rattachement réinscrit la MÊME session (voir `inscrire` et le chemin de
/// reprise de D4), qui héritait alors du focus sans que le client l'ait jamais
/// réémis — deux parts au lieu d'une, prises sur ses voisines.
pub(super) fn oublier(garde: &mut MutexGuard<'static, Etat>, session: &str) -> Vec<(String, Ordre)> {
    garde.canaux.remove(session);
    garde.dernieres_parts.remove(session);
    // Les quatre tables de D7 s'oublient ICI et nulle part ailleurs. Le
    // registre a trois chemins de retrait (fermeture normale, canal rompu
    // détecté par les ordres, canal rompu détecté par les parts) : un champ
    // oublié par deux d'entre eux est exactement le défaut M1 de la revue
    // finale de branche du sous-bloc D6.
    //
    // `derniers_audio` en particulier : un rattachement réinscrit la MÊME
    // session (voir `inscrire`), et un `false` resté en mémoire ferait juger
    // l'ordre déjà livré — sur un canal disparu avec la rupture. La fenêtre
    // resterait muette sans terme.
    garde.pids.remove(session);
    garde.arrivees.remove(session);
    garde.derniers_focus.remove(session);
    garde.derniers_audio.remove(session);
    // `inaptes` et `rearmements` (D9) : même motif que `derniers_audio`
    // ci-dessus, et c'est le défaut M1 de la revue finale de branche du
    // sous-bloc D6 rejoué une deuxième fois. Un rattachement réinscrit la
    // MÊME session (voir `inscrire` et le chemin de reprise de D4) : sans
    // cette purge, une session dont la capture audio venait de mourir
    // hériterait de son inaptitude ou de son compteur de réarmements
    // périmés à travers une reconnexion pourtant saine — jusqu'à
    // `REPIT_REARMEMENT_AUDIO` (5 s) d'exclusion injustifiée dans le cas
    // ordinaire, jusqu'à 24 h de silence garanti après un abandon définitif.
    garde.inaptes.remove(session);
    garde.rearmements.remove(session);
    if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    garde.vivier.retirer(session, Instant::now())
}

/// Inscrit une session au registre et rend, avec son canal, la génération
/// qui vient de lui être attribuée.
///
/// **La génération est frappée ICI, par le capteur, et non transmise par le
/// protocole** (revue de la première version de cette tâche, D9) : la
/// frapper au LANCEMENT d'un processus ne couvre pas la course réelle. Les
/// deux seuls chemins qui réinscrivent un nom sont soit un enfant relancé
/// par le superviseur — auquel cas `Table::compteur`
/// (`superviseur/table.rs:399`) donne un nom NEUF, donc aucune course —,
/// soit le MÊME enfant qui se rattache au capteur (`CanalTube::rattacher`)
/// après une rupture de tube, auquel cas il redit délibérément le même
/// `hwnd`/`sortie` dans une attache qui n'a jamais porté de génération. La
/// seule frontière où deux inscriptions distinctes du même nom peuvent se
/// chevaucher est donc une attache **côté capteur** : c'est là, et
/// seulement là, qu'une génération neuve doit naître.
///
/// L'appelant (`Fenetre::servir`) retient la valeur rendue le temps de son
/// service et la redonne telle quelle à `retirer`.
pub fn inscrire(session: &str, pid: u32) -> (Receiver<Message>, u64) {
    let (emetteur, receveur) = channel::<Message>();
    let mut garde = etat();
    // Frappée INCONDITIONNELLEMENT, à CHAQUE appel — y compris un
    // rattachement sous le même nom : c'est la seule façon de distinguer
    // l'instance qui vient de s'inscrire de la précédente. Voir le champ
    // `prochaine_generation` pour pourquoi ce compteur est distinct de
    // `horloge`.
    garde.prochaine_generation += 1;
    let generation = garde.prochaine_generation;
    garde.generations.insert(session.to_string(), generation);
    if garde.canaux.insert(session.to_string(), emetteur).is_some() {
        tracing::warn!(%session, "canal d'ordres remplacé pour cette session");
        // Sans cette purge, une part identique à celle déjà envoyée sur
        // L'ANCIEN canal (disparu avec la rupture) serait jugée déjà livrée
        // par le filtre d'écrasement de `distribuer_les_parts`, et le canal
        // NEUF ne la recevrait jamais si la topologie n'a pas changé entre
        // les deux inscriptions — le plafond de débit resterait périmé sans
        // terme. Une première inscription n'a, elle, rien à purger.
        garde.dernieres_parts.remove(session);
        // Même motif que la ligne ci-dessus : l'ordre audio mémorisé l'a été
        // sur l'ANCIEN canal, disparu avec la rupture.
        garde.derniers_audio.remove(session);
    }
    garde.pids.insert(session.to_string(), pid);
    // Le rang d'arrivée n'est posé que si la session n'en a pas déjà un.
    //
    // ⚠️ **La portée réelle est plus étroite que l'intention** (F6, revue
    // finale de branche du sous-bloc D7). L'intention est qu'un rattachement
    // ne fasse pas perdre à la fenêtre son ancienneté au sein de son groupe de
    // PID — mais `oublier` retire `arrivees` ET `derniers_focus`, et sur un
    // rattachement ORDINAIRE le `retirer` du fil de fenêtre mort court AVANT
    // que l'enfant ne se reconnecte : les deux tables sont alors déjà vides,
    // et la garde ci-dessous ne retient rien. Elle ne mord que dans la fenêtre
    // de course où l'inscription neuve précède le retrait de l'ancienne.
    //
    // Retenir ces tables pendant un délai de grâce serait le remède, mais
    // c'est un changement de conception du registre — à cadrer, pas à
    // improviser : la même identité par NOM porte déjà une course connue,
    // consignée pour le sous-bloc suivant.
    if !garde.arrivees.contains_key(session) {
        garde.horloge += 1;
        let rang = garde.horloge;
        garde.arrivees.insert(session.to_string(), rang);
    }
    let ordres = garde.vivier.inscrire(session, Instant::now());
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
    (receveur, generation)
}

pub fn retirer(session: &str, generation: u64) {
    let mut garde = etat();
    // Sans effet si l'inscription enregistrée est PLUS RÉCENTE : ce `retirer`
    // est celui d'une instance déjà remplacée par un rattachement (F5, D9).
    // Ne PAS appeler `oublier` ici est délibéré : les neuf tables qu'elle
    // purge appartiennent toutes à l'instance VIVANTE, pas à celle,
    // périmée, qui appelle ce `retirer`.
    if retirer_est_perime(&garde.generations, session, generation) {
        tracing::info!(%session, generation, "retirer périmé ignoré");
        return;
    }
    let ordres = oublier(&mut garde, session);
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
    garde.generations.remove(session);
}
