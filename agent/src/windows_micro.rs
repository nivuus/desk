//! Assembling the microphone on the Windows side: the cable, the loop guard, the
//! render thread, and the sink the transport loop feeds.
//!
//! It is the only file of block E2 that **assembles**; everything that can go
//! wrong has been moved out of it and is tested under Linux — the cable designation
//! (`wasapi/peripherique.rs`), the format check (`wasapi/format.rs`), the
//! exclusivity policy (`micro/exclusivite.rs`), the loop guard
//! (`micro/boucle_locale.rs`).
//!
//! ## The order is plan E2's, and it is not commutative
//!
//! 1. join the MTA, then create the enumerator **on the render thread**;
//! 2. `rendu::identifiant_capte` — **if and only if** the loopback really
//!    captures an endpoint;
//! 3. `rendu::resoudre_cable`;
//! 4. `boucle_locale::evaluer` → `Risque` ⇒ **we stop there**, and the `warn!`
//!    names the remedy;
//! 5. `RenduWasapi::ouvrir` (that is where the format is refused);
//! 6. the loop: `attendre_place` → `remplir` → `ecrire`.
//!
//! The guard comes **before** opening, not after: opening the cable then
//! noticing the loop would already have put a render stream on the endpoint
//! the agent captures.
//!
//! ## No COM object crosses a thread boundary
//!
//! `RenduWasapi` is not `Send` (see its header): everything COM is born
//! and dies on the render thread. The consequence is that [`ouvrir`] cannot
//! know the verdict when returning from a `spawn` — it learns it through a channel,
//! with a bound. It is the written price of the "open in the thread" approach that
//! task 7 chose so as to have no `unsafe` promise to keep.
//!
//! ## The reader's `Mutex` is KEPT, and here is why
//!
//! `PuitsMicro::deposer` is called **from the transport loop** and must
//! never make it wait (`transport/piste_micro.rs`). E1 itself
//! writes that "block E2 will have a real WASAPI thread with a hard deadline and will have to
//! decide otherwise — a lock-free queue, or a double buffer"
//! (`demarrage/micro/mesure.rs`). **We keep the `Mutex`.** The render thread only
//! holds it for the time of `remplir` — decoding at most a handful of
//! Opus frames, of the order of ten microseconds — whereas the WASAPI
//! deadline is of the order of 10 ms: three orders of magnitude above. A
//! lock-free queue would be work written before having observed the need.
//!
//! 🔴 **And the need is made OBSERVABLE rather than conjectural**: the thread
//! counts its **missed deadlines** (`retards` of the periodic trace). If this
//! counter stays at zero, the question is settled; if it rises, it is asked
//! with a figure. Without it, it would have been settled by opinion.
//!
//! ✅ **THE COUNTER STAYED AT ZERO, SO THE QUESTION IS SETTLED** (acceptance
//! E2, tasks 12 and 13, August 20th, 2026): `retards=0` in every run,
//! and **cumulative 0 over TWO ten-minute trials** (615 then 630 lines of
//! trace). The `Mutex` stays, and it is no longer a bet — a lock-free queue
//! would be work written against a need measured at zero.

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

/// The named mutex that guarantees a single process writes to the cable.
mod verrou;

/// How long [`ouvrir`] waits for the render thread's verdict.
///
/// ⚠️ **Bounded, and not generously.** This delay is paid by the session's
/// STARTUP: `demarrage::micro::brancher` runs before `run()`. One second
/// is largely enough for three COM calls on local objects; beyond that,
/// something is not answering, and a healthy video session is worth more
/// than a microphone we wait for.
const DELAI_VERDICT: Duration = Duration::from_secs(1);

/// Period of the periodic trace. One second, like E1's measurement
/// sink — the two read side by side in `agent.log`.
const PERIODE_TRACE: Duration = Duration::from_secs(1);

/// Bound of the wait for room at each round. Three times the usual period of the
/// shared audio engine (10 ms): enough never to expire in normal
/// operation, little enough for the thread to wake up and count its delay if the
/// device stops signalling.
const DELAI_PLACE: Duration = Duration::from_millis(30);

/// The sink the transport loop feeds.
///
/// It only does two things: arbitrate exclusivity, and deposit. **It
/// writes nothing to the cable** — it is the render thread that consumes, at its own
/// pace.
pub struct PuitsCable {
    lecteur: Arc<Mutex<LecteurMicro>>,
    exclusivite: Exclusivite<verrou::MutexNomme>,
    session: String,
}

impl PuitsMicro for PuitsCable {
    fn deposer(&mut self, trame: TrameMicro) -> bool {
        // ⚠️ The attempt is redone at EACH deposit; only the LOG is
        // single (Decision 2 of plan E2). A sticky refusal would condemn
        // window B to stay without a microphone for the life of its process after
        // window A died, without any line saying so.
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
                     tant qu'elle le tiendra. Le navigateur L'APPREND (bloc E3) : ce refus \
                     remonte en un message de controle mic-state a granted=false"
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
            // The lock is poisoned: the render thread panicked. We refuse
            // rather than propagating the panic into the transport loop —
            // a microphone defect never kills a video session (spec §10).
            Err(_) => false,
        }
    }
}

/// Opens the cable, arms the loop guard, launches the render thread, and returns the
/// sink — or says **why** it cannot.
///
/// **Never fails the session**: the caller logs and continues without a
/// microphone. `micro_disponible()` stays false, `ready` carries `mic: false`, and the
/// browser's button does not appear.
pub fn ouvrir(config: &Config) -> Result<PuitsCable> {
    let lecteur = Arc::new(Mutex::new(
        LecteurMicro::new().context("creation du lecteur de micro")?,
    ));

    // ⚠️ The question "does the loopback capture an endpoint?" is
    // decided HERE, on the configuration, and never in the thread: in process
    // loopback (`fenetre_hwnd` set) there is NO endpoint —
    // `ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK)` targets
    // a process tree —, and `AUDIO=0` opens no capture. In both
    // cases there is nothing to compare, and querying COM for nothing would cost
    // a device resolution at the startup of every child.
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

/// What the thread returns to [`ouvrir`] when everything went well. **Strings
/// and an `enum`, no COM object**: that is what crosses the thread boundary,
/// and nothing else can.
struct Verdict {
    cable: String,
    format: String,
    reveil: Reveil,
}

/// The render thread: it opens, returns its verdict, then writes until the
/// end of the process.
fn fil_de_rendu(
    lecteur: Arc<Mutex<LecteurMicro>>,
    session: String,
    loopback_de_session: bool,
    envoi: SyncSender<Result<Verdict>>,
) {
    let (mut rendu_wasapi, canaux) = match preparer(loopback_de_session) {
        Ok((r, canaux, verdict)) => {
            // ⚠️ If sending fails, `ouvrir` has already given up (timeout exceeded):
            // we stop rather than write on a cable nobody
            // will feed — the sink does not exist.
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
            // The buffer was still full: it is a missed deadline, not
            // an error. It is THIS counter that will make the `Mutex` question
            // decidable (see the module header).
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
                // ⚠️ `remplir` NEVER blocks and fills with silence
                // (spec §8): the cable must be fed continuously. An
                // application listening to an empty buffer does not perceive
                // silence, it sees a stream that breaks off — it is not the
                // same thing, and it can be heard.
                //
                // ⚠️ The lock is released HERE, at the end of this block, and therefore
                // BEFORE the write: `deposer` must never wait for the end
                // of a WASAPI call.
                lecteur.remplir(cible);
            }
            if ecrire(&mut rendu_wasapi, cible, trames, &session).is_err() {
                return;
            }
            ecrites += trames as u64;
            silence += trames_de_silence(cible, canaux) as u64;
        }

        if Instant::now() >= prochaine_trace {
            // ⚠️ **Both reads happen under THE SAME lock**, and it is
            // not a convenience: `occupation` is a SNAPSHOT, and
            // reading it at a second locking would date it from another moment than
            // the counters, on a buffer the depositing thread moves
            // every 20 ms. Two quantities of the same log line
            // must describe the same instant, otherwise the line invites
            // relating what cannot be related.
            let (compteurs, occupation) = match lecteur.lock() {
                Ok(l) => (l.compteurs(), l.occupation()),
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
                occupation,
            );
            precedents = compteurs;
            ecrites = 0;
            silence = 0;
            retards = 0;
            prochaine_trace = Instant::now() + PERIODE_TRACE;
        }
    }
}

/// The write, isolated so that the loop stays readable. `Err(())` means
/// "the thread stops", and the cause is already logged.
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

/// All the startup COM, in plan E2's order. Returns the opened stream, its
/// number of channels, and the verdict to send back.
fn preparer(loopback_de_session: bool) -> Result<(RenduWasapi, usize, Verdict)> {
    rejoindre_mta()?;
    // SAFETY: the current thread has just joined the MTA (`rejoindre_mta`
    // above refuses if it already belonged to an STA).
    let enumerateur: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .context("creation de l'enumerateur de peripheriques audio (micro)")?;

    let capte = if loopback_de_session {
        Some(rendu::identifiant_capte(&enumerateur)?)
    } else {
        None
    };

    let (peripherique, identifiant) = rendu::resoudre_cable(&enumerateur)?;

    // 🔴 The local loop guard, BEFORE any opening. Measured necessary
    // on August 20th, 2026: this VM's default render device IS the cable, and
    // `LoopbackCapture::open` captures the default when `AUDIO_PERIPHERIQUE` is
    // absent. The user would hear themselves with the latency of the full
    // round trip; without headphones, the acoustic loop would close through the
    // speakers.
    if evaluer(capte.as_deref(), &identifiant) == Boucle::Risque {
        let disponibles = rendu::enumerer(&enumerateur).unwrap_or_default();
        // ⚠️ **It is the MICROPHONE that yields, never the sound** (Decision 3): sound
        // is a workstream delivered since workstream A, the microphone is what is being
        // added. The remedy is NAMED, on the pattern of the `Choix::Ambigu` arm
        // of `resoudre`, which already lists its candidates.
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
    // The number of channels is not reread from the stream: `RenduWasapi::ouvrir`
    // REFUSED any format that is not stereo (`wasapi/format.rs`), so it
    // is `CANAUX` or the opening failed. Rereading it would open the door to
    // the two diverging.
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

/// The periodic trace.
///
/// ⚠️ **`session` is MANDATORY**: `agent.log` mixes the supervisor and all
/// its children since D4, and D6 had to re-attribute two traces in the middle of acceptance
/// for lack of this field. A trace without it is a number in an anonymous
/// multiset.
#[allow(clippy::too_many_arguments)]
fn tracer(
    session: &str,
    ecrites: u64,
    silence: u64,
    retards: u64,
    reveil: Reveil,
    compteurs: &CompteursMicro,
    precedents: &CompteursMicro,
    occupation: std::time::Duration,
) {
    // The counters are DELTAS of the elapsed second, not cumulative totals: a
    // cumulative total would drag a single incident along for the rest of the session.
    let d = |maintenant: u64, avant: u64| maintenant.saturating_sub(avant);
    tracing::info!(
        session = %session,
        ecrites,
        // ⚠️ `ecrites` and `silence` SIDE BY SIDE: `remplir` fills with silence
        // without ever saying so, so "48,000 frames written" is written
        // exactly the same for a microphone that speaks and for a microphone that is
        // silent. Without the second, this trace proves nothing.
        silence,
        retards,
        reveil = reveil.libelle(),
        deposees = d(compteurs.deposees, precedents.deposees),
        sauts = d(compteurs.sauts, precedents.sauts),
        insertions = d(compteurs.insertions, precedents.insertions),
        plc = d(compteurs.plc, precedents.plc),
        // ⚠️ **`plc` and `plc_plafonnees` SIDE BY SIDE**, and it is the heart of
        // this pair: both arise from a missing frame, and without the
        // second one cannot distinguish "concealment is working" from "the
        // ceiling has bitten and the sink is silent". See `micro/dissimulation.rs`.
        plc_plafonnees = d(compteurs.plc_plafonnees, precedents.plc_plafonnees),
        // ── Bloc E3 : le legs n°7 de E2 ────────────────────────────────────
        //
        // 🔴 **These two quantities ALREADY EXISTED and were read by
        // NOBODY on the production path.** `LecteurMicro::occupation()`
        // has been public since E1 and `famines` has been a field of `CompteursMicro`
        // since E1; both only lived in the trace of the MEASUREMENT SINK
        // (`MICRO_MESURE=1`), which **cannot coexist with the cable** — the
        // measurement sink consumes the buffer, the cable too. E2 noted it and
        // could not fix it within its scope. There was nothing to
        // compute, only to trace.
        //
        // 🔴 **`occupation_ms` is a SNAPSHOT; `famines` is a DELTA.**
        // All the other counters of this trace are deltas *on purpose*
        // — "a cumulative total would drag a single incident along for the rest of the
        // session". Mixing the two without saying so would make the line
        // unreadable: one would read "120" for an occupation and "3" for
        // starvations believing both comparable over the same second, whereas
        // the first describes the trace's instant and the second the second
        // preceding it.
        //
        // ⚠️ **`famines` and `occupation_ms` SIDE BY SIDE**, on the pattern of the
        // two preceding pairs: a starvation is an emptied buffer, hence an
        // occupation that touched zero. The second alone would not say how many
        // times; the first alone would not say how far we are from the edge.
        //
        // ⚠️ **Ce que cela NE donne PAS** : la latence de bout en bout, que
        // RIEN ne mesure dans ce dépôt depuis D1. C'est la SECONDE des deux
        // composantes de la latence ajoutée par l'agent, dont E2 n'avait que la
        // première (`retards`). Le troisième critère de la spec §13 reste NON
        // JUGÉ.
        famines = d(compteurs.famines, precedents.famines),
        occupation_ms = occupation.as_millis(),
        "micro ecrit sur le cable"
    );
}
