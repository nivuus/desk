//! Le registre global du sommeil : un `Vivier` partagé, un canal d'ordres par
//! fenêtre, et le tour de roue qui débloque l'hystérésis.
//!
//! **Il ne décide rien.** Toute la logique est dans `vivier.rs`, qui est pur et
//! testé ; ce module ne fait que la brancher sur des canaux.
//!
//! **Pourquoi un état global de processus plutôt qu'un objet passé de main en
//! main.** Chaque fenêtre du capteur vit sur son propre fil, créé par
//! `serveur::ouvrir_les_commandes`, et l'arbitrage est par nature transverse :
//! le signal d'une fenêtre peut endormir sa voisine. Le registre des attentes
//! de connexion média (`serveur.rs`) emploie déjà exactement ce patron, pour
//! la même raison. C'est aussi ce qui permet à ce sous-bloc de **ne pas
//! toucher `serveur.rs`**, dont la marge de taille est de 10 lignes.

// `parts` porte le calcul et la distribution des parts de débit. Extrait pour
// la même raison que `fenetre::transitions` : ce fichier a franchi le
// plafond de 500 lignes du projet en y ajoutant le remède au canal rompu
// détecté par cette voie-là (voir `parts::distribuer_les_parts`). Il ne
// s'appelle pas `repartiteur` : ce nom est déjà pris par le module qui porte
// la RÈGLE pure ; celui-ci ne porte que sa BRANCHE sur ce registre.
mod parts;
mod porteurs;

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use crate::capteur::vivier::{Ordre, Raison, Vivier, HYSTERESIS, PLAFOND_EVEIL};

/// Période du tour de roue. Ni une cadence de rendu ni une horloge : c'est le
/// seul moyen pour une fenêtre bloquée sous hystérésis d'être réexaminée, et
/// 250 ms est très en deçà des 2 s d'hystérésis tout en restant négligeable.
const PERIODE_REARBITRAGE: Duration = Duration::from_millis(250);

/// Répit avant qu'une fenêtre dont la capture audio est morte ne redevienne
/// éligible au portage.
///
/// **Il finance le cas MAJORITAIRE** — une application, une fenêtre, donc
/// aucune voisine à promouvoir. Sans lui, le remède ne couvrirait que les
/// applications multi-fenêtres et le groupe resterait muet sans retour,
/// exactement comme avant D9. Réélire la même session construit une activation
/// *process loopback* NEUVE, ce qui est une chance réelle : les causes connues
/// d'un refus de lecture WASAPI — changement de périphérique, redémarrage du
/// service audio, changement de format — sont transitoires.
///
/// ⚠️ **NON CALIBRÉE.** Aucune mesure ne la fonde : elle rejoint `BPP_MIN`,
/// `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC` et
/// `TAILLE_MAX_SORTIE`.
pub const REPIT_REARMEMENT_AUDIO: Duration = Duration::from_secs(5);

/// Nombre de réarmements consécutifs avant abandon définitif.
///
/// Sans borne, un périphérique audio définitivement mort ferait tourner le
/// cycle « inapte → répit → réélue → morte » sans fin, et chaque tour coûte
/// une activation COM.
///
/// ⚠️ **NON CALIBRÉE**, comme la précédente. L'abandon définitif est
/// **journalisé**, jamais muet — c'est la contrainte que F3 de D7 a posée.
pub const REARMEMENTS_MAX: u32 = 5;

/// Ce qu'une fenêtre reçoit du registre global.
///
/// **Un seul canal pour les deux**, et non deux canaux parallèles : ce qu'un
/// canal unique garantit est l'ordre de LIVRAISON — deux canaux parallèles
/// laisseraient une part d'endormie doubler l'ordre de dormir qui la motive,
/// et la fenêtre serait momentanément décrite comme endormie alors qu'elle
/// encode encore.
///
/// ⚠️ **Il ne garantit PAS l'ordre de CALCUL, et la distinction n'est pas
/// théorique** (I2, revue finale de branche du sous-bloc D6). Les appelants
/// respectent bien « `distribuer` puis `distribuer_les_parts` », sauf un : le
/// chemin `rompus` de `distribuer_les_parts` envoie les parts d'abord, puis
/// retire du vivier les sessions dont le canal est rompu, puis seulement
/// relaie les ordres que ce retrait engendre. Une session réveillée par la
/// place ainsi libérée reçoit son `Reveiller` APRÈS une part d'endormie déjà
/// périmée, et ne reçoit sa part d'éveillée qu'au tour de roue suivant.
/// **Borne : `PERIODE_REARBITRAGE`, 250 ms au plancher `PART_DORMANTE_BPS`.**
/// Conséquence assumée : la recalculer sur place demanderait une seconde passe
/// de parts sous le même verrou, pour 250 ms de plancher sur un chemin qui ne
/// s'emprunte qu'à la mort inopinée d'un fil de fenêtre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Sommeil(Ordre),
    Part { bps: u32 },
    /// Ordre de porter le son, ou de se taire. Poussé **au changement
    /// seulement**, comme `Part`.
    ///
    /// Sur le même canal que les deux autres, et pour la même raison : au
    /// sein du canal d'UNE session, un canal unique garantit l'ordre de
    /// LIVRAISON entre `Sommeil`, `Part` et `Audio`. **Cela ne s'étend pas
    /// entre deux sessions** : l'ancienne porteuse et la nouvelle ont chacune
    /// leur propre canal, lu par son propre fil de fenêtre. `porteurs::
    /// distribuer_l_audio` envoie l'ordre de se taire avant celui de porter,
    /// ce qui RÉDUIT la fenêtre où les deux fenêtres d'un même processus
    /// seraient audibles ensemble — sans la fermer : la borne réelle est
    /// l'ordonnancement des deux fils, pas ce canal.
    Audio { actif: bool },
}

struct Etat {
    vivier: Vivier,
    canaux: HashMap<String, Sender<Message>>,
    /// La session que le client déclare focalisée, si elle existe encore.
    ///
    /// Tenue ici et non dans `Vivier` : le vivier arbitre des places
    /// d'encodeur, le répartiteur des parts de débit. Le client émet `blur`
    /// aussi bien que `focus` (`client/src/visibilite.ts`), donc ce champ se
    /// vide bien quand la fenêtre perd le focus.
    focalisee: Option<String>,
    /// Dernière part envoyée à chaque session. **Le seul rempart contre une
    /// inondation** : le tour de roue ré-arbitre toutes les 250 ms, et sans
    /// cette mémoire huit fenêtres recevraient 32 messages par seconde à vie.
    dernieres_parts: HashMap<String, u32>,
    /// PID du processus propriétaire de chaque fenêtre. **Ici et pas dans un
    /// second registre** : le capteur n'a qu'une vérité à tenir, et deux
    /// tables à synchroniser en feraient deux.
    pids: HashMap<String, u32>,
    /// Rang d'arrivée de chaque session, et rang du dernier focus reçu. Deux
    /// compteurs tirés du même `horloge`, strictement croissante.
    arrivees: HashMap<String, u64>,
    derniers_focus: HashMap<String, u64>,
    /// Compteur monotone qui sert de rang aux deux tables ci-dessus. Un
    /// `Instant` ne conviendrait pas : il faut un ordre total, stable et
    /// comparable, pas une durée.
    horloge: u64,
    /// Dernier ordre audio envoyé à chaque session. **Le rempart contre
    /// l'inondation**, exactement comme `dernieres_parts` : le tour de roue
    /// ré-arbitre toutes les 250 ms.
    derniers_audio: HashMap<String, bool>,
    /// Instant après lequel une session dont la capture audio est morte
    /// redevient éligible au portage. Absente = apte.
    ///
    /// **Ici et pas dans `capteur::audio`** : ce module a l'horloge, l'autre
    /// est pur et le reste.
    inaptes: HashMap<String, Instant>,
    /// Nombre de réarmements consécutifs déjà accordés à chaque session.
    /// Remis à zéro dès qu'elle porte le son sans mourir.
    rearmements: HashMap<String, u32>,
}

static ETAT: OnceLock<Mutex<Etat>> = OnceLock::new();

fn etat() -> MutexGuard<'static, Etat> {
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
fn distribuer(garde: &mut MutexGuard<'static, Etat>, ordres: Vec<(String, Ordre)>) {
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
fn oublier(garde: &mut MutexGuard<'static, Etat>, session: &str) -> Vec<(String, Ordre)> {
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

pub fn inscrire(session: &str, pid: u32) -> Receiver<Message> {
    let (emetteur, receveur) = channel::<Message>();
    let mut garde = etat();
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
    receveur
}

pub fn retirer(session: &str) {
    let mut garde = etat();
    let ordres = oublier(&mut garde, session);
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
}

pub fn signaler(session: &str, visible: bool, focalisee: bool) {
    let mut garde = etat();
    if focalisee {
        garde.focalisee = Some(session.to_string());
        // Le rang du focus, et non un booléen : c'est lui qui fait tenir la
        // règle 3 de l'arbitrage — un groupe qui perd tout focus garde son son
        // sur la DERNIÈRE à l'avoir eu.
        garde.horloge += 1;
        let rang = garde.horloge;
        garde.derniers_focus.insert(session.to_string(), rang);
    } else if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    let ordres = garde.vivier.signaler(session, visible, focalisee, Instant::now());
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
}

/// Signale l'échec de la reconstruction du `WindowsSource` lors d'un réveil.
///
/// Appelée par la tâche 6 quand la reconstruction du `WindowsSource` échoue
/// après un `Ordre::Reveiller` : sans ce chemin de retour, le vivier croirait
/// la fenêtre éveillée pour toujours et ne la reproposerait jamais au tour de
/// roue.
pub fn echec_de_reveil(session: &str) {
    let mut garde = etat();
    let ordres = garde.vivier.echec_de_reveil(session, Instant::now());
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
}

/// Une session signale que sa capture audio est morte.
///
/// **Ne répond rien, et c'est voulu** : l'arbitrage est global et la décision
/// peut concerner une AUTRE fenêtre. L'effet revient par
/// `DepuisCapteur::Audio`, poussé sur la connexion média de chaque fenêtre
/// concernée — exactement le patron de `signaler`.
pub fn audio_mort(session: &str) {
    let mut garde = etat();
    let tours = garde.rearmements.entry(session.to_string()).or_insert(0);
    *tours += 1;
    if *tours > REARMEMENTS_MAX {
        // Abandon définitif, JOURNALISÉ. Un silence muet est précisément le
        // défaut que F3 de D7 a corrigé ; ne pas le réintroduire ici.
        tracing::warn!(
            %session,
            rearmements = *tours - 1,
            "capture audio morte et abandon définitif : le groupe de PID restera muet"
        );
        garde.inaptes.insert(session.to_string(), Instant::now() + Duration::from_secs(86_400));
    } else {
        tracing::info!(
            %session,
            rearmement = *tours,
            repit = ?REPIT_REARMEMENT_AUDIO,
            "capture audio morte, réarmement programmé"
        );
        garde.inaptes.insert(session.to_string(), Instant::now() + REPIT_REARMEMENT_AUDIO);
    }
    porteurs::distribuer_l_audio(&mut garde);
}

/// Retire du registre les inaptitudes dont le répit a expiré.
///
/// **Nommée et séparée pour être ÉPROUVABLE** : le tour de roue (250 ms) est
/// ce qui rend le répit effectif, et sans cette purge une inapte le resterait
/// jusqu'au prochain événement, qui peut ne jamais venir. Un `retain` en ligne
/// dans le tour de roue ne serait couvert par aucun test.
pub(super) fn purger_les_inaptitudes(inaptes: &mut HashMap<String, Instant>, maintenant: Instant) {
    inaptes.retain(|_, echeance| *echeance > maintenant);
}

/// Le texte que le client recevra. **Stable** : il traverse deux protocoles et
/// s'affiche à l'utilisateur.
pub fn raison_en_texte(raison: Raison) -> &'static str {
    match raison {
        Raison::Masquee => "masquee",
        Raison::Evincee => "evincee",
    }
}

// Extrait dans un fichier voisin : cette suite portait `sommeil.rs` à 500
// lignes pour un plafond de projet à 500, marge nulle dès sa naissance. Voir
// l'en-tête de `sommeil/tests.rs`.
#[cfg(test)]
mod tests;
