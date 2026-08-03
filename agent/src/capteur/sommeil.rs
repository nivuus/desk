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

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use crate::capteur::repartiteur::{self, Fenetre};
use crate::capteur::vivier::{Ordre, Raison, Vivier, HYSTERESIS, PLAFOND_EVEIL};

/// Période du tour de roue. Ni une cadence de rendu ni une horloge : c'est le
/// seul moyen pour une fenêtre bloquée sous hystérésis d'être réexaminée, et
/// 250 ms est très en deçà des 2 s d'hystérésis tout en restant négligeable.
const PERIODE_REARBITRAGE: Duration = Duration::from_millis(250);

/// Budget de débit de la session entière, en bits par seconde.
///
/// **De session, pas par fenêtre** — c'est tout le sujet du sous-bloc D6.
/// Lu une seule fois : le changer en cours de vie n'aurait aucun sens tant
/// que le lien ne change pas.
///
/// **12 Mb/s est un CHOIX, pas une dérivation.** La tâche 1 a montré que le
/// lien porte ≥ 1,45 Gb/s : la capacité du chemin ne borne rien ici, et le
/// budget ne s'en dérive pas. Ce qui borne est ce que le CLIENT décode. La
/// tâche 1bis relève, à huit fenêtres : 18,03 % d'images jetées au barreau
/// plein, 7,99 % à 1024×576, 1,47 % à 640×360 — et surtout que réduire les
/// bits **sans** franchir de seuil de barreau ne sauve rien (23,08 % à
/// surface constante). **Le levier est la résolution, le débit n'en est que
/// la commande.**
///
/// 12 Mb/s conserve au cas mono-fenêtre exactement ce qu'il a aujourd'hui, et
/// donne 1,33 Mb/s par fenêtre à huit — soit le barreau 852×480, **entre les
/// deux points mesurés, donc non mesuré**. ⚠️ **C'est la recette de la
/// tâche 10 qui le calibre** : si elle déçoit, descendre à 8 Mb/s, seul point
/// relevé propre, au prix du cas mono-fenêtre.
fn budget_bps() -> u32 {
    static BUDGET: OnceLock<u32> = OnceLock::new();
    *BUDGET.get_or_init(|| {
        let budget = std::env::var("BUDGET_BPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(12_000_000);
        tracing::info!(budget_bps = budget, "budget de debit de la session");
        budget
    })
}

/// Ce qu'une fenêtre reçoit du registre global.
///
/// **Un seul canal pour les deux**, et non deux canaux parallèles : l'ordre
/// entre un endormissement et la part qui en découle est ainsi garanti par
/// construction. Une fenêtre qui recevrait sa part d'endormie avant l'ordre de
/// dormir serait momentanément décrite comme endormie alors qu'elle encode
/// encore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Sommeil(Ordre),
    Part { bps: u32 },
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
        distribuer_les_parts(&mut garde);
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
                garde.canaux.remove(&session);
                let maintenant = Instant::now();
                suite.extend(garde.vivier.retirer(&session, maintenant));
            }
        }
        a_traiter = suite;
    }
}

/// Recalcule les parts et n'envoie que celles qui ont changé.
///
/// **Appelée APRÈS `distribuer`**, jamais avant : les ordres de sommeil
/// changent l'éveil, et une part calculée avant eux décrirait l'état
/// précédent.
///
/// Une session dont le canal est rompu est retirée, comme dans `distribuer` —
/// mais sans boucle de reprise : contrairement à un ordre de sommeil, une part
/// perdue n'engendre aucun ordre supplémentaire, et la prochaine passe la
/// rattrapera.
fn distribuer_les_parts(garde: &mut MutexGuard<'static, Etat>) {
    let eveillees = garde.vivier.eveillees();
    let focalisee = garde.focalisee.clone();
    let fenetres: Vec<Fenetre> = garde
        .canaux
        .keys()
        .map(|session| Fenetre {
            session: session.clone(),
            eveillee: eveillees.iter().any(|e| e == session),
            focalisee: focalisee.as_deref() == Some(session.as_str()),
        })
        .collect();

    let parts = repartiteur::repartir(budget_bps(), &fenetres);

    // Les sessions disparues ne doivent pas laisser leur part en mémoire.
    let vivantes: std::collections::HashSet<&String> =
        parts.iter().map(|(session, _)| session).collect();
    garde.dernieres_parts.retain(|session, _| vivantes.contains(session));

    let mut rompus = Vec::new();
    for (session, bps) in parts {
        if garde.dernieres_parts.get(&session) == Some(&bps) {
            continue;
        }
        let envoye = match garde.canaux.get(&session) {
            Some(canal) => canal.send(Message::Part { bps }).is_ok(),
            None => false,
        };
        if envoye {
            garde.dernieres_parts.insert(session, bps);
        } else {
            rompus.push(session);
        }
    }
    for session in rompus {
        garde.canaux.remove(&session);
        garde.dernieres_parts.remove(&session);
    }
}

pub fn inscrire(session: &str) -> Receiver<Message> {
    let (emetteur, receveur) = channel::<Message>();
    let mut garde = etat();
    if garde.canaux.insert(session.to_string(), emetteur).is_some() {
        tracing::warn!(%session, "canal d'ordres remplacé pour cette session");
    }
    let ordres = garde.vivier.inscrire(session, Instant::now());
    distribuer(&mut garde, ordres);
    distribuer_les_parts(&mut garde);
    receveur
}

pub fn retirer(session: &str) {
    let mut garde = etat();
    garde.canaux.remove(session);
    if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    let ordres = garde.vivier.retirer(session, Instant::now());
    distribuer(&mut garde, ordres);
    distribuer_les_parts(&mut garde);
}

pub fn signaler(session: &str, visible: bool, focalisee: bool) {
    let mut garde = etat();
    if focalisee {
        garde.focalisee = Some(session.to_string());
    } else if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    let ordres = garde.vivier.signaler(session, visible, focalisee, Instant::now());
    distribuer(&mut garde, ordres);
    distribuer_les_parts(&mut garde);
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
    distribuer_les_parts(&mut garde);
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

    fn verrouiller_pour_le_test() -> MutexGuard<'static, ()> {
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
    fn premier_ordre(canal: &Receiver<Message>) -> Option<Ordre> {
        loop {
            match canal.try_recv() {
                Ok(Message::Sommeil(ordre)) => return Some(ordre),
                Ok(Message::Part { .. }) => continue,
                Err(_) => return None,
            }
        }
    }

    #[test]
    fn une_session_inscrite_recoit_l_ordre_de_se_reveiller_quand_elle_devient_visible() {
        let _verrou = verrouiller_pour_le_test();
        // Noms uniques : le registre est un état GLOBAL de processus, et les
        // tests Rust tournent en parallèle dans le même processus.
        let ordres = inscrire("t5-a");
        signaler("t5-a", true, true);
        assert_eq!(premier_ordre(&ordres), Some(Ordre::Reveiller));
        retirer("t5-a");
    }

    #[test]
    fn une_session_retiree_ne_recoit_plus_rien() {
        let _verrou = verrouiller_pour_le_test();
        let ordres = inscrire("t5-b");
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
        let ordres = inscrire("t5-c");
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

    #[test]
    fn une_session_qui_s_eveille_recoit_une_part_apres_son_ordre_de_reveil() {
        let _verrou = verrouiller_pour_le_test();
        let messages = inscrire("t6-a");
        signaler("t6-a", true, true);

        let recus: Vec<Message> = messages.try_iter().collect();
        let position_reveil = recus
            .iter()
            .position(|m| matches!(m, Message::Sommeil(Ordre::Reveiller)))
            .expect("l'ordre de réveil doit être présent");
        // La recherche part de `position_reveil`, pas du début : l'inscription
        // elle-même envoie déjà une PREMIÈRE part (le plancher endormi, avant
        // tout ordre — voir la doc de `inscrire`), et c'est légitime. La
        // recherche depuis le début confondrait cette part-là, envoyée AVANT
        // le réveil, avec celle que ce test veut vérifier : celle qui décrit
        // la fenêtre ÉVEILLÉE, et qui doit suivre son ordre.
        let position_part = recus[position_reveil..]
            .iter()
            .position(|m| matches!(m, Message::Part { .. }))
            .map(|i| i + position_reveil)
            .expect("une part doit suivre le réveil");
        assert!(
            position_reveil < position_part,
            "la part suit l'ordre, jamais l'inverse : une fenêtre encore endormie \
             recevrait sinon une part d'éveillée"
        );
        retirer("t6-a");
    }

    #[test]
    fn une_part_inchangee_n_est_pas_reemise() {
        let _verrou = verrouiller_pour_le_test();
        let messages = inscrire("t6-b");
        signaler("t6-b", true, true);
        let _ = messages.try_iter().count();

        // Même signal, donc même état, donc même part : rien ne doit partir.
        signaler("t6-b", true, true);
        let parts: Vec<Message> = messages
            .try_iter()
            .filter(|m| matches!(m, Message::Part { .. }))
            .collect();
        assert!(parts.is_empty(), "une part inchangée ne se réémet pas : {parts:?}");
        retirer("t6-b");
    }

    #[test]
    fn l_arrivee_d_une_seconde_fenetre_reduit_la_part_de_la_premiere() {
        let _verrou = verrouiller_pour_le_test();
        let a = inscrire("t6-c");
        signaler("t6-c", true, true);
        let premiere = derniere_part(&a).expect("la première doit avoir une part");

        let b = inscrire("t6-d");
        signaler("t6-d", true, false);
        let apres = derniere_part(&a).expect("la première doit être ré-servie");
        assert!(
            apres < premiere,
            "part de la première : {premiere} puis {apres} — elle doit baisser"
        );
        assert!(derniere_part(&b).is_some(), "la seconde doit recevoir une part");

        retirer("t6-c");
        retirer("t6-d");
    }

    /// Dernière part reçue sur un canal, en vidant ce qui s'y trouve.
    fn derniere_part(canal: &Receiver<Message>) -> Option<u32> {
        canal
            .try_iter()
            .filter_map(|m| match m {
                Message::Part { bps } => Some(bps),
                _ => None,
            })
            .last()
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
            let ordres = inscrire(&nom);
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
        let ordres_attend = inscrire("t5-attend");
        signaler("t5-attend", true, true);
        assert_eq!(premier_ordre(&ordres_attend), None, "t5-attend devrait rester endormie");

        // "t5-tardif" arrive ensuite : plus recente que "t5-attend", donc elle
        // la devancerait si une place se liberait. Son recepteur est jete
        // immediatement : son canal est rompu des avant toute tentative
        // d'envoi.
        drop(inscrire("t5-tardif"));
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
