//! Le registre lui-même : l'état global (`Etat`), son point d'accès
//! (`etat`), et les quatre opérations qui le touchent (`distribuer`,
//! `oublier`, `inscrire`, `retirer`).
//!
//! ⚠️ **~~et le tour de roue~~ : il a quitté ce fichier au round de correction
//! 1** (25 août 2026), pour `registre/tour_de_roue.rs` — la phrase ci-dessus
//! le nommait, et elle est corrigée dans le même mouvement plutôt que laissée
//! à vieillir.
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
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use crate::capteur::vivier::{Ordre, Vivier, HYSTERESIS, PLAFOND_EVEIL};

use super::file::{canal_de_session, Envoi, ReceveurSession};
use super::{parts, porteurs, presse_papier, purger_les_inaptitudes, retirer_est_perime, Message, PERIODE_REARBITRAGE};

// Les TABLES du registre vivent chez le voisin : `Etat` est un agrégat de
// tables presque entièrement fait de documentation, ce fichier-ci porte ce qui
// AGIT sur elles. Extrait au round de correction 3, AVANT d'écrire ici — le
// fichier était à 470 pour un plafond de 500. Le module s'appelle `tables` et
// non `etat` parce qu'une fonction `etat()` vit juste en dessous.
mod tables;
pub(super) use tables::Etat;


// Le FIL du tour de roue vit chez le voisin : il porte une horloge et une E/S
// Win32, ce fichier-ci porte le registre. Extrait au round de correction 1,
// qui avait porté ce fichier à 508 lignes pour un plafond de 500 — extraire,
// jamais comprimer.
mod tour_de_roue;
use tour_de_roue::demarrer_le_tour_de_roue;

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
            dernier_presse_papier: None,
            notre_ecriture: None,
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
            let issue = match garde.canaux.get(&session) {
                Some(canal) => Some(canal.envoyer(Message::Sommeil(ordre))),
                // 🔴 `None` N'EST PAS UNE LIVRAISON, ET LE ROUND 2 A CORRIGÉ
                // CETTE RÉDACTION. Le comportement est celui d'avant (l'ancien
                // `None => false` : ne rien purger, la session n'est déjà plus
                // dans `canaux`), mais l'écrire `Envoi::Depose(...)` fabriquait
                // une livraison et rendait cette perte INDISCERNABLE d'un vrai
                // dépôt. Un `Option` la nomme pour ce qu'elle est : il n'y a
                // eu aucun envoi.
                None => None,
            };
            let rompu = match issue {
                // Aucun canal : rien n'est parti, et il n'y a rien à purger —
                // `oublier` a déjà retiré cette session. La tracer ferait une
                // ligne par ordre à chaque retrait, sur un chemin nominal.
                None => false,
                Some(Envoi::Depose(_)) => false,
                // 🔴 UN ORDRE DE SOMMEIL REFUSÉ EST PERDU, ET IL NE DOIT PAS
                // L'ÊTRE EN SILENCE (correctif du round 1 : il l'était).
                // `Sommeil` est la SEULE des quatre variantes que la fenêtre
                // APPLIQUE au lieu de la relayer, et la doc de cette fonction
                // dit ce que sa perte coûte : le vivier a déjà posé
                // `eveillee = true` en interne, aucun ré-arbitrage futur ne
                // réémettra cet ordre, et « la place resterait occupée dans le
                // vivier sans qu'aucun encodeur réel ne l'occupe, pour toute
                // la vie du processus ». C'est mot pour mot une fenêtre qui ne
                // s'endort plus.
                //
                // ⚠️ **PAS de purge** : la session est VIVANTE, seulement en
                // retard, et la purger tuerait l'arbitrage de la fenêtre la
                // plus en peine.
                //
                // 🔴 **CE QUI EST FAIT ICI EST « NE PAS MENTIR », PAS
                // « RETENTER »**, et la distinction porte toute la décision.
                // L'annulation ci-dessus rend au vivier l'état d'AVANT
                // l'ordre : rien n'est réémis à l'intérieur de cette boucle,
                // c'est le prochain arbitrage qui reprend, en voyant la
                // fenêtre dans son ancien état. **La terminaison de la boucle
                // n'est donc pas touchée du tout.**
                //
                // ⚠️ ~~Rendre sa place au vivier ne peut pas être appliqué ici
                // sans que la boucle DIVERGE.~~ **CETTE PHRASE ÉTAIT PLUS
                // FORTE QUE CE QUI ÉTAIT ÉTABLI, et le round de correction 2
                // l'a réfutée par la mesure** : la troisième voie — appeler
                // `echec_de_reveil` depuis la boucle — a été jouée, et la
                // suite termine en 0,02 s. `REPIT_APRES_ECHEC` exclut la
                // session des candidates, donc avec un `maintenant` capturé
                // une seule fois, l'ensemble en répit croît strictement.
                // **Ce qui est vrai, et rien de plus : la preuve de
                // terminaison ÉCRITE PLUS HAUT ne couvre pas ce cas** — elle
                // repose sur le fait qu'un nouveau lot n'est engendré que par
                // un canal ROMPU, et que chaque rompu QUITTE `canaux` avant le
                // tour suivant. C'est une preuve à refaire, pas une
                // divergence. Le remède retenu ne pose pas la question, et il
                // couvre en outre les DEUX sens là où `echec_de_reveil` ne
                // couvre que le réveil.
                //
                // La trace est un `error!` et non un `warn!` : c'est une perte
                // d'état non réparable. Elle n'est pas cadencée, à la
                // différence de celle du refus, parce que `distribuer` n'émet
                // un ordre que sur un CHANGEMENT d'arbitrage — jamais à chaque
                // tour de roue.
                Some(Envoi::Refuse) => {
                    // 🔴 ON REND AU VIVIER L'ÉTAT D'AVANT L'ORDRE. Sans cela,
                    // il aurait déjà écrit `eveillee` pour un ordre jamais
                    // parti — le SIXIÈME site de mémorisation, trouvé au round
                    // de correction 2. Voir `Vivier::annuler_ordre_non_livre`
                    // pour les deux sens et ce que chacun coûte.
                    garde.vivier.annuler_ordre_non_livre(&session, ordre);
                    tracing::error!(
                        session_cible = %session,
                        ?ordre,
                        "ordre de sommeil NON DEPOSE : file de la fenêtre pleine, \
                         etat du vivier rendu, l'ordre repartira au prochain arbitrage"
                    );
                    false
                }
                Some(Envoi::Rompu) => true,
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
/// par le superviseur — auquel cas `Table::prochaine_session`
/// (`superviseur/table.rs`, qui incrémente `compteur` avant de composer
/// `w-<n>`) donne un nom NEUF, donc aucune course —,
/// soit le MÊME enfant qui se rattache au capteur (`CanalTube::rattacher`)
/// après une rupture de tube, auquel cas il redit délibérément le même
/// `hwnd`/`sortie` dans une attache qui n'a jamais porté de génération. La
/// seule frontière où deux inscriptions distinctes du même nom peuvent se
/// chevaucher est donc une attache **côté capteur** : c'est là, et
/// seulement là, qu'une génération neuve doit naître.
///
/// L'appelant (`Fenetre::servir`) retient la valeur rendue le temps de son
/// service et la redonne telle quelle à `retirer`.
pub fn inscrire(session: &str, pid: u32) -> (ReceveurSession, u64) {
    let (emetteur, receveur) = canal_de_session(session);
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
    presse_papier::emettre_l_etat_courant(&mut garde, session);
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
