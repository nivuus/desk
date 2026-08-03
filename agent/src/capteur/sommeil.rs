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
    /// Sur le même canal que les deux autres, et pour la même raison : un
    /// canal unique garantit l'ordre de LIVRAISON. Un ordre de se taire doit
    /// atteindre l'ancienne porteuse **avant** que la nouvelle ne commence,
    /// sans quoi les deux fenêtres d'un même processus seraient audibles
    /// ensemble.
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
/// retient quatre choses d'une session (son canal, sa dernière part, le focus
/// si elle le porte, son entrée au vivier), et il en existe trois chemins de
/// retrait — la fermeture normale (`retirer`), la détection d'un canal rompu
/// pendant la distribution des ORDRES (`distribuer`), et la même détection
/// pendant celle des PARTS (`parts::distribuer_les_parts`).
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
    // `derniers_audio` en particulier : un rattachement réinscrit la MÊME
    // session (voir `inscrire`), et un `false` resté en mémoire ferait juger
    // l'ordre déjà livré — sur un canal disparu avec la rupture. La fenêtre
    // resterait muette sans terme.
    garde.pids.remove(session);
    garde.arrivees.remove(session);
    garde.derniers_focus.remove(session);
    garde.derniers_audio.remove(session);
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
    // Le rang d'arrivée n'est posé qu'à la PREMIÈRE inscription : un
    // rattachement ne doit pas faire perdre à la fenêtre son ancienneté au
    // sein de son groupe de PID.
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

/// Le texte que le client recevra. **Stable** : il traverse deux protocoles et
/// s'affiche à l'utilisateur.
pub fn raison_en_texte(raison: Raison) -> &'static str {
    match raison {
        Raison::Masquee => "masquee",
        Raison::Evincee => "evincee",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noms uniques (t5-a, t5-b…) ne suffisent pas à isoler ces tests entre
    /// eux : le vivier partagé n'a qu'UN plafond de `PLAFOND_EVEIL` places
    /// pour tout le processus, et un test qui le sature (pour éprouver une
    /// place qui se libère) prive de facto les autres tests, exécutés en
    /// parallèle par défaut, de toute place disponible — observé : la
    /// saturation à 8 fait échouer intermittemment un test voisin qui
    /// s'attend à s'éveiller aussitôt. Un `Mutex` dédié aux tests sérialise
    /// ce fichier sans toucher au code de production ni à `vivier.rs`.
    static VERROU_TESTS: Mutex<()> = Mutex::new(());

    /// `pub(super)` : repris par `parts::tests`, qui sature le même vivier
    /// partagé et doit s'y sérialiser exactement de la même façon (ce module
    /// et `parts::tests` sont deux descendants distincts de `sommeil`, pas
    /// l'un de l'autre — d'où la visibilité explicite).
    pub(super) fn verrouiller_pour_le_test() -> MutexGuard<'static, ()> {
        VERROU_TESTS.lock().unwrap_or_else(|empoisonne| empoisonne.into_inner())
    }

    /// Le premier ORDRE de sommeil reçu, en ignorant les parts qui peuvent le
    /// précéder ou s'y intercaler.
    ///
    /// **Nécessaire depuis que `inscrire` se termine par
    /// `distribuer_les_parts`** : la toute première part d'une session part à
    /// l'inscription même, avant tout ordre — une fenêtre encore endormie a
    /// bien une part (le plancher `PART_DORMANTE_BPS`), et c'est délibéré
    /// (voir la doc de `inscrire`). Les tests d'ORDRE, hérités de D5, portent
    /// sur `Ordre` et non sur `Message` : ce filtre restaure leur intention
    /// d'origine sans la changer.
    ///
    /// `pub(super)` : repris par `parts::tests`, pour la même raison que
    /// `verrouiller_pour_le_test`.
    pub(super) fn premier_ordre(canal: &Receiver<Message>) -> Option<Ordre> {
        loop {
            match canal.try_recv() {
                Ok(Message::Sommeil(ordre)) => return Some(ordre),
                Ok(Message::Part { .. }) => continue,
                Ok(Message::Audio { .. }) => continue,
                Err(_) => return None,
            }
        }
    }

    #[test]
    fn une_session_inscrite_recoit_l_ordre_de_se_reveiller_quand_elle_devient_visible() {
        let _verrou = verrouiller_pour_le_test();
        // Noms uniques : le registre est un état GLOBAL de processus, et les
        // tests Rust tournent en parallèle dans le même processus.
        let ordres = inscrire("t5-a", 5001);
        signaler("t5-a", true, true);
        assert_eq!(premier_ordre(&ordres), Some(Ordre::Reveiller));
        retirer("t5-a");
    }

    #[test]
    fn une_session_retiree_ne_recoit_plus_rien() {
        let _verrou = verrouiller_pour_le_test();
        let ordres = inscrire("t5-b", 5002);
        retirer("t5-b");
        signaler("t5-b", true, true);
        assert_eq!(premier_ordre(&ordres), None);
    }

    #[test]
    fn les_deux_raisons_ont_un_texte_stable_pour_le_client() {
        // Pas d'accès au vivier partagé ici : aucun verrou requis.
        assert_eq!(raison_en_texte(Raison::Masquee), "masquee");
        assert_eq!(raison_en_texte(Raison::Evincee), "evincee");
    }

    #[test]
    fn un_echec_de_reveil_rendort_la_session_et_ne_la_reelit_pas_immediatement() {
        let _verrou = verrouiller_pour_le_test();
        // "t5-c" devient visible et focalisee, donc eveillee par arbitrer().
        let ordres = inscrire("t5-c", 5003);
        signaler("t5-c", true, true);
        assert_eq!(premier_ordre(&ordres), Some(Ordre::Reveiller));

        // La reconstruction du WindowsSource echoue : le vivier doit repasser
        // la session a l'etat endormi. Aucun nouvel ORDRE n'est du dans les
        // 500 ms de repit qui suivent, meme si la session reste visible et
        // focalisee : la reproposer immediatement bouclerait a chaque
        // arbitrage sur une construction d'encodeur vouee a rechouer. Une
        // part (le retour au plancher `PART_DORMANTE_BPS`) est en revanche
        // legitime : la fenetre est reellement rendormie.
        echec_de_reveil("t5-c");
        assert_eq!(premier_ordre(&ordres), None);

        retirer("t5-c");
    }

    /// M1 de la revue finale de branche du sous-bloc D6 : `retirer` vidait
    /// `focalisee`, mais ni `distribuer` ni le chemin `rompus` de
    /// `distribuer_les_parts` ne le faisaient. Une session focalisée qui meurt
    /// par canal rompu — le fil de fenêtre qui panique avant son point de
    /// retrait unique — laissait donc son nom dans le registre.
    ///
    /// **L'état est lu directement, et c'est délibéré.** La conséquence
    /// visible par les parts n'est pas discriminante : un nom mort ne désigne
    /// aucune fenêtre vivante, donc la majoration ne s'applique à personne —
    /// ce qui est aussi le cas quand `focalisee` vaut `None`. Ce qui MORD est
    /// la réinscription du même nom (rattachement, chemin de reprise de D4),
    /// qui hériterait du focus sans que le client l'ait jamais réémis ; mais
    /// l'éprouver par les parts exigerait un `signaler` sur ce nom, qui vide
    /// `focalisee` de lui-même et effacerait le défaut avant de le mesurer.
    /// Le champ est privé à ce module, et ce test en est un descendant : le
    /// lire est l'observation la plus directe et la moins ambiguë.
    #[test]
    fn un_canal_rompu_libere_aussi_le_focus_de_la_session_morte() {
        let _verrou = verrouiller_pour_le_test();
        let canal = inscrire("m1-focus", 5004);
        signaler("m1-focus", true, true);
        assert_eq!(
            etat().focalisee.as_deref(),
            Some("m1-focus"),
            "précondition : le registre tient bien cette session pour la focalisée"
        );

        // Le fil de "m1-focus" meurt SANS passer par `retirer`, exactement ce
        // qui arrive quand il panique.
        drop(canal);

        // L'inscription d'une session tierce — endormie — fait varier le
        // budget partagé, donc la part de "m1-focus", donc tente un envoi sur
        // son canal rompu : c'est ce qui déclenche la détection.
        inscrire("m1-tiers", 5005);
        assert_eq!(
            etat().focalisee,
            None,
            "le focus d'une session morte doit être rendu avec le reste de ce que le registre \
             retenait d'elle"
        );

        retirer("m1-focus");
        retirer("m1-tiers");
    }

    #[test]
    fn un_retrait_qui_libere_une_place_reveille_bien_la_session_qui_l_attendait() {
        let _verrou = verrouiller_pour_le_test();
        // Sature les PLAFOND_EVEIL (8) places avec des sessions dediees, dont
        // on garde les Receiver vivants pour que leur canal ne soit jamais
        // rompu par accident pendant le test.
        let mut recepteurs_pleins = Vec::new();
        for i in 0..8 {
            let nom = format!("t5-plein-{i}");
            let ordres = inscrire(&nom, 5100 + i as u32);
            signaler(&nom, true, true);
            assert_eq!(
                premier_ordre(&ordres),
                Some(Ordre::Reveiller),
                "{nom} devrait s'eveiller"
            );
            recepteurs_pleins.push((nom, ordres));
        }

        // "t5-attend" arrive alors que le plafond est deja atteint : elle
        // reste endormie, faute de place.
        let ordres_attend = inscrire("t5-attend", 5200);
        signaler("t5-attend", true, true);
        assert_eq!(premier_ordre(&ordres_attend), None, "t5-attend devrait rester endormie");

        // "t5-tardif" arrive ensuite : plus recente que "t5-attend", donc elle
        // la devancerait si une place se liberait. Son recepteur est jete
        // immediatement : son canal est rompu des avant toute tentative
        // d'envoi.
        drop(inscrire("t5-tardif", 5201));
        signaler("t5-tardif", true, true);

        // Libere UNE place en retirant le premier "plein". Le vivier elit
        // alors "t5-tardif" (la plus recente des deux candidates bloquees),
        // et cette livraison echoue puisque son canal est rompu. Sans la
        // boucle de `distribuer`, le Reveiller que ce retrait engendre
        // ENSUITE pour "t5-attend" serait perdu pour toujours : le vivier
        // aurait deja pose `eveillee = true` sur "t5-attend" en interne, et
        // plus aucun rearbitrage ne le reproposerait.
        retirer("t5-plein-0");

        assert_eq!(
            premier_ordre(&ordres_attend),
            Some(Ordre::Reveiller),
            "le reveil libere par la mort de t5-tardif doit atteindre t5-attend"
        );

        // Nettoyage.
        retirer("t5-attend");
        retirer("t5-tardif");
        for (nom, _) in recepteurs_pleins.into_iter().skip(1) {
            retirer(&nom);
        }
    }
}
