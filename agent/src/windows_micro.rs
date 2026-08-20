//! L'assemblage du microphone côté Windows : le câble, la garde de boucle, le
//! fil de rendu, et le puits que la boucle de transport alimente.
//!
//! C'est le seul fichier du bloc E2 qui **assemble** ; tout ce qui peut se
//! tromper en a été sorti et est éprouvé sous Linux — la désignation du câble
//! (`wasapi/peripherique.rs`), le contrôle de format (`wasapi/format.rs`), la
//! politique d'exclusivité (`micro/exclusivite.rs`), la garde de boucle
//! (`micro/boucle_locale.rs`).
//!
//! ## L'ordre est celui du plan E2, et il n'est pas commutatif
//!
//! 1. rejoindre la MTA, puis créer l'énumérateur **sur le fil de rendu** ;
//! 2. `rendu::identifiant_capte` — **si et seulement si** le loopback capte
//!    réellement un point de terminaison ;
//! 3. `rendu::resoudre_cable` ;
//! 4. `boucle_locale::evaluer` → `Risque` ⇒ **on s'arrête là**, et le `warn!`
//!    nomme le remède ;
//! 5. `RenduWasapi::ouvrir` (c'est là que le format est refusé) ;
//! 6. la boucle : `attendre_place` → `remplir` → `ecrire`.
//!
//! La garde vient **avant** l'ouverture, et pas après : ouvrir le câble puis
//! constater la boucle aurait déjà mis un flux de rendu sur le point de
//! terminaison que l'agent capte.
//!
//! ## Aucun objet COM ne traverse de frontière de fil
//!
//! `RenduWasapi` n'est pas `Send` (voir son en-tête) : tout ce qui est COM naît
//! et meurt sur le fil de rendu. La conséquence est que [`ouvrir`] ne peut pas
//! connaître le verdict en revenant d'un `spawn` — il l'apprend par un canal,
//! avec une borne. C'est le prix, écrit, du parti « ouvrir dans le fil » que la
//! tâche 7 a retenu pour n'avoir aucune promesse `unsafe` à tenir.
//!
//! ## Le `Mutex` du lecteur est GARDÉ, et voici pourquoi
//!
//! `PuitsMicro::deposer` est appelé **depuis la boucle de transport** et ne
//! doit jamais la faire attendre (`transport/piste_micro.rs`). E1 écrit
//! lui-même que « le bloc E2 aura un vrai fil WASAPI à échéance dure et devra
//! trancher autrement — une file sans verrou, ou un double tampon »
//! (`demarrage/micro/mesure.rs`). **On garde le `Mutex`.** Le fil de rendu ne
//! le tient que le temps de `remplir` — le décodage d'au plus une poignée de
//! trames Opus, de l'ordre de la dizaine de microsecondes — quand l'échéance
//! WASAPI est de l'ordre de 10 ms : trois ordres de grandeur au-dessus. Une
//! file sans verrou serait du travail écrit avant d'avoir constaté le besoin.
//!
//! 🔴 **Et le besoin est rendu OBSERVABLE plutôt que conjectural** : le fil
//! compte ses **retards d'échéance** (`retards` de la trace périodique). Si ce
//! compteur reste à zéro, la question est tranchée ; s'il monte, elle se pose
//! avec un chiffre. Sans lui, on l'aurait tranchée par opinion.

#![cfg(windows)]

use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

use crate::micro::boucle_locale::{evaluer, Boucle};
use crate::micro::exclusivite::{Exclusivite, Issue};
use crate::micro::{CompteursMicro, LecteurMicro, PuitsMicro, TrameMicro};
use crate::wasapi::ecriture::{rejoindre_mta, RenduWasapi, Reveil};
use crate::wasapi::rendu;
use crate::wasapi_format::trames_de_silence;
use crate::wasapi_peripherique::inventaire;
use crate::Config;

/// Le mutex nommé qui garantit qu'un seul processus écrit sur le câble.
mod verrou;

/// Combien de temps [`ouvrir`] attend le verdict du fil de rendu.
///
/// ⚠️ **Bornée, et pas généreusement.** Ce délai est payé par le DÉMARRAGE de
/// la session : `demarrage::micro::brancher` court avant `run()`. Une seconde
/// suffit très largement à trois appels COM sur des objets locaux ; au-delà,
/// c'est que quelque chose ne répond pas, et une session vidéo saine vaut mieux
/// qu'un micro qu'on attend.
const DELAI_VERDICT: Duration = Duration::from_secs(1);

/// Période de la trace périodique. Une seconde, comme celle du puits de mesure
/// de E1 — les deux se lisent côte à côte dans `agent.log`.
const PERIODE_TRACE: Duration = Duration::from_secs(1);

/// Borne de l'attente de place à chaque tour. Trois fois la période usuelle du
/// moteur audio partagé (10 ms) : assez pour ne jamais expirer en régime
/// normal, assez peu pour que le fil se réveille et compte son retard si le
/// périphérique cesse de signaler.
const DELAI_PLACE: Duration = Duration::from_millis(30);

/// Le puits que la boucle de transport alimente.
///
/// Il ne fait que deux choses : arbitrer l'exclusivité, et déposer. **Il
/// n'écrit rien sur le câble** — c'est le fil de rendu qui consomme, à son
/// rythme.
pub struct PuitsCable {
    lecteur: Arc<Mutex<LecteurMicro>>,
    exclusivite: Exclusivite<verrou::MutexNomme>,
    session: String,
}

impl PuitsMicro for PuitsCable {
    fn deposer(&mut self, trame: TrameMicro) -> bool {
        // ⚠️ La tentative est refaite à CHAQUE dépôt ; seul le JOURNAL est
        // unique (Décision 2 du plan E2). Un refus collant condamnerait la
        // fenêtre B à rester sans micro pour la vie de son processus après la
        // mort de la fenêtre A, sans qu'aucune ligne ne le dise.
        match self.exclusivite.arbitrer() {
            Issue::Accepte => {}
            Issue::AccepteApresRefus => tracing::info!(
                session = %self.session,
                "micro : cable acquis apres un refus — une autre fenetre l'a relache"
            ),
            Issue::RefusePremierement => {
                tracing::warn!(
                    session = %self.session,
                    "micro : une autre fenetre tient deja le cable, cette session restera muette \
                     tant qu'elle le tiendra. Le navigateur ne l'apprend pas (Decision 9 du plan \
                     E2, leguee a E3) : le bouton s'allume et rien ne sort"
                );
                return false;
            }
            Issue::RefuseDejaDit => return false,
        }

        match self.lecteur.lock() {
            Ok(mut lecteur) => {
                lecteur.deposer(trame);
                true
            }
            // Le verrou est empoisonné : le fil de rendu a paniqué. On refuse
            // plutôt que de propager la panique dans la boucle de transport —
            // un défaut du micro ne tue jamais une session vidéo (spec §10).
            Err(_) => false,
        }
    }
}

/// Ouvre le câble, arme la garde de boucle, lance le fil de rendu, et rend le
/// puits — ou dit **pourquoi** il ne le peut pas.
///
/// **N'échoue jamais la session** : l'appelant journalise et continue sans
/// micro. `micro_disponible()` reste faux, `ready` porte `mic: false`, et le
/// bouton du navigateur ne paraît pas.
pub fn ouvrir(config: &Config) -> Result<PuitsCable> {
    let lecteur = Arc::new(Mutex::new(
        LecteurMicro::new().context("creation du lecteur de micro")?,
    ));

    // ⚠️ La question « le loopback capte-t-il un point de terminaison ? » se
    // décide ICI, sur la configuration, et jamais dans le fil : en process
    // loopback (`fenetre_hwnd` posé) il n'y a AUCUN endpoint —
    // `ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK)` vise
    // un arbre de processus —, et `AUDIO=0` n'ouvre aucune capture. Dans les
    // deux cas il n'y a rien à comparer, et interroger COM pour rien coûterait
    // une résolution de périphérique au démarrage de chaque enfant.
    let loopback_de_session = config.audio && config.fenetre_hwnd.is_none();

    let (envoi, reception) = sync_channel::<Result<Verdict>>(1);
    let lecteur_fil = Arc::clone(&lecteur);
    let session = config.session_id.clone();
    let session_fil = session.clone();
    std::thread::spawn(move || {
        fil_de_rendu(lecteur_fil, session_fil, loopback_de_session, envoi);
    });

    let verdict = match reception.recv_timeout(DELAI_VERDICT) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return Err(e),
        Err(_) => bail!(
            "le fil de rendu du micro n'a rendu aucun verdict en {:?} : pas de micro",
            DELAI_VERDICT
        ),
    };

    let mutex = verrou::MutexNomme::creer()?;
    tracing::info!(
        session = %session,
        cable = %verdict.cable,
        format = %verdict.format,
        reveil = verdict.reveil.libelle(),
        espace_mutex = mutex.espace(),
        "micro : ecriture sur le cable ARMEE"
    );

    Ok(PuitsCable {
        lecteur,
        exclusivite: Exclusivite::new(mutex),
        session,
    })
}

/// Ce que le fil rend à [`ouvrir`] quand tout s'est bien passé. **Des chaînes
/// et un `enum`, aucun objet COM** : c'est ce qui traverse la frontière de fil,
/// et rien d'autre ne le peut.
struct Verdict {
    cable: String,
    format: String,
    reveil: Reveil,
}

/// Le fil de rendu : il ouvre, il rend son verdict, puis il écrit jusqu'à la
/// fin du processus.
fn fil_de_rendu(
    lecteur: Arc<Mutex<LecteurMicro>>,
    session: String,
    loopback_de_session: bool,
    envoi: SyncSender<Result<Verdict>>,
) {
    let (mut rendu_wasapi, canaux) = match preparer(loopback_de_session) {
        Ok((r, canaux, verdict)) => {
            // ⚠️ Si l'envoi échoue, `ouvrir` a déjà renoncé (délai dépassé) :
            // on s'arrête plutôt que d'écrire sur un câble que personne
            // n'alimentera — le puits n'existe pas.
            if envoi.send(Ok(verdict)).is_err() {
                return;
            }
            (r, canaux)
        }
        Err(e) => {
            let _ = envoi.send(Err(e));
            return;
        }
    };

    let mut tampon = vec![0.0f32; 0];
    let mut precedents = CompteursMicro::default();
    let mut ecrites: u64 = 0;
    let mut silence: u64 = 0;
    let mut retards: u64 = 0;
    let mut prochaine_trace = Instant::now() + PERIODE_TRACE;

    loop {
        let trames = match rendu_wasapi.attendre_place(DELAI_PLACE) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    session = %session,
                    erreur = %e,
                    "micro : attente de place sur le cable echouee, fil de rendu arrete"
                );
                return;
            }
        };
        if trames == 0 {
            // Le tampon était encore plein : c'est un retard d'échéance, pas
            // une erreur. C'est CE compteur qui rendra décidable la question du
            // `Mutex` (voir l'en-tête de module).
            retards += 1;
        } else {
            let besoin = trames * canaux;
            if tampon.len() < besoin {
                tampon.resize(besoin, 0.0);
            }
            let cible = &mut tampon[..besoin];

            {
                let Ok(mut lecteur) = lecteur.lock() else {
                    tracing::warn!(
                        session = %session,
                        "micro : verrou du lecteur empoisonne, fil de rendu arrete"
                    );
                    return;
                };
                // ⚠️ `remplir` ne bloque JAMAIS et complète au silence
                // (spec §8) : le câble doit être alimenté en continu. Une
                // application qui écoute un tampon vide ne perçoit pas du
                // silence, elle voit un flux qui s'interrompt — ce n'est pas la
                // même chose, et cela s'entend.
                //
                // ⚠️ Le verrou est relâché ICI, à la fin de ce bloc, et donc
                // AVANT l'écriture : `deposer` ne doit jamais attendre la fin
                // d'un appel WASAPI.
                lecteur.remplir(cible);
            }
            if ecrire(&mut rendu_wasapi, cible, trames, &session).is_err() {
                return;
            }
            ecrites += trames as u64;
            silence += trames_de_silence(cible, canaux) as u64;
        }

        if Instant::now() >= prochaine_trace {
            let compteurs = match lecteur.lock() {
                Ok(l) => l.compteurs(),
                Err(_) => return,
            };
            tracer(
                &session,
                ecrites,
                silence,
                retards,
                rendu_wasapi.reveil(),
                &compteurs,
                &precedents,
            );
            precedents = compteurs;
            ecrites = 0;
            silence = 0;
            retards = 0;
            prochaine_trace = Instant::now() + PERIODE_TRACE;
        }
    }
}

/// L'écriture, isolée pour que la boucle reste lisible. `Err(())` signifie
/// « le fil s'arrête », et la cause est déjà journalisée.
fn ecrire(
    rendu_wasapi: &mut RenduWasapi,
    pcm: &[f32],
    trames: usize,
    session: &str,
) -> std::result::Result<(), ()> {
    match rendu_wasapi.ecrire(pcm, trames) {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::warn!(
                session = %session,
                erreur = %e,
                "micro : ecriture sur le cable echouee, fil de rendu arrete"
            );
            Err(())
        }
    }
}

/// Tout le COM du démarrage, dans l'ordre du plan E2. Rend le flux ouvert, son
/// nombre de canaux, et le verdict à renvoyer.
fn preparer(loopback_de_session: bool) -> Result<(RenduWasapi, usize, Verdict)> {
    rejoindre_mta()?;
    // SAFETY : le fil courant vient de rejoindre la MTA (`rejoindre_mta`
    // ci-dessus refuse s'il appartenait déjà à une STA).
    let enumerateur: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .context("creation de l'enumerateur de peripheriques audio (micro)")?;

    let capte = if loopback_de_session {
        Some(rendu::identifiant_capte(&enumerateur)?)
    } else {
        None
    };

    let (peripherique, identifiant) = rendu::resoudre_cable(&enumerateur)?;

    // 🔴 La garde de boucle locale, AVANT toute ouverture. Mesurée nécessaire
    // le 20 août 2026 : le rendu par défaut de cette VM EST le câble, et
    // `LoopbackCapture::open` capte le défaut quand `AUDIO_PERIPHERIQUE` est
    // absente. L'utilisateur s'entendrait lui-même avec la latence du tour
    // complet ; sans casque, la boucle acoustique se refermerait par les
    // haut-parleurs.
    if evaluer(capte.as_deref(), &identifiant) == Boucle::Risque {
        let disponibles = rendu::enumerer(&enumerateur).unwrap_or_default();
        // ⚠️ **C'est le MICRO qui cède, et jamais le son** (Décision 3) : le son
        // est un chantier livré depuis le chantier A, le micro est ce qu'on
        // ajoute. Le remède est NOMMÉ, sur le patron du bras `Choix::Ambigu`
        // de `resoudre`, qui énumère déjà ses candidats.
        bail!(
            "micro DESACTIVE : le loopback audio de cette session capte le cable meme sur lequel \
             le micro ecrirait ({identifiant}) — l'utilisateur s'entendrait lui-meme. Remede : \
             posez AUDIO_PERIPHERIQUE sur un AUTRE rendu. Disponibles : {}",
            inventaire(&disponibles)
        );
    }

    let rendu_wasapi = RenduWasapi::ouvrir(&peripherique)?;
    let format = rendu_wasapi.description().to_string();
    let reveil = rendu_wasapi.reveil();
    // Le nombre de canaux n'est pas relu du flux : `RenduWasapi::ouvrir` a
    // REFUSÉ tout format qui ne soit pas stéréo (`wasapi/format.rs`), donc il
    // vaut `CANAUX` ou l'ouverture a échoué. Le relire ouvrirait la porte à ce
    // que les deux divergent.
    let canaux = crate::wasapi_format::CANAUX;
    Ok((
        rendu_wasapi,
        canaux,
        Verdict {
            cable: identifiant,
            format,
            reveil,
        },
    ))
}

/// La trace périodique.
///
/// ⚠️ **`session` est OBLIGATOIRE** : `agent.log` mêle le superviseur et tous
/// ses enfants depuis D4, et D6 a dû ré-imputer deux traces en pleine recette
/// faute de ce champ. Une trace sans lui est un nombre dans un multiensemble
/// anonyme.
#[allow(clippy::too_many_arguments)]
fn tracer(
    session: &str,
    ecrites: u64,
    silence: u64,
    retards: u64,
    reveil: Reveil,
    compteurs: &CompteursMicro,
    precedents: &CompteursMicro,
) {
    // Les compteurs sont des DELTAS de la seconde écoulée, pas des cumuls : un
    // cumul ferait traîner un unique incident pour le restant de la session.
    let d = |maintenant: u64, avant: u64| maintenant.saturating_sub(avant);
    tracing::info!(
        session = %session,
        ecrites,
        // ⚠️ `ecrites` et `silence` CÔTE À CÔTE : `remplir` complète au silence
        // sans jamais le dire, donc « 48 000 trames écrites » s'écrit
        // exactement pareil pour un micro qui parle et pour un micro qui se
        // tait. Sans le second, cette trace ne prouve rien.
        silence,
        retards,
        reveil = reveil.libelle(),
        deposees = d(compteurs.deposees, precedents.deposees),
        sauts = d(compteurs.sauts, precedents.sauts),
        insertions = d(compteurs.insertions, precedents.insertions),
        plc = d(compteurs.plc, precedents.plc),
        // ⚠️ **`plc` et `plc_plafonnees` CÔTE À CÔTE**, et c'est le fond de
        // cette paire : les deux naissent d'une trame manquante, et sans le
        // second on ne distingue pas « la dissimulation travaille » de « le
        // plafond a mordu et le puits se tait ». Voir `micro/dissimulation.rs`.
        plc_plafonnees = d(compteurs.plc_plafonnees, precedents.plc_plafonnees),
        "micro ecrit sur le cable"
    );
}
