//! **The bridge thread**: it owns the table, completes ProjFS commands, and
//! sweeps expiries. It is category 3 of the threading discipline described
//! at the head of [`crate::pont::projfs`].
//!
//! It owns **neither** the `Rtc` **nor** the socket — it is
//! [`crate::pont::transport::tourner`] that holds them, on its own thread, and
//! the two talk through the two `mpsc`s the plan defines. The reason for this
//! split is written at the head of [`crate::pont::projfs`], under the divergence
//! it constitutes.
//!
//! # What this thread NEVER does
//!
//! - **It does not walk the root.** A `read_dir` on the root would cross
//!   ProjFS, hence would trigger our own enumeration callbacks, which
//!   register a command that **this thread** must complete: it
//!   would wait for itself. That is why the hydration survey counts what
//!   the bridge writes instead of measuring the disk.
//!
//!   ⚠️ **AND THAT IS EXACTLY WHY F2'S WRITE THREAD IS
//!   DISTINCT FROM IT**: that one, for its part, READS files of the root
//!   (`pont::ecriture::fil`). Putting it here would replay the sentence above.
//! - **It never replays an expired command.** A replayed request
//!   would produce a second response without a recipient (spec §5.3).

mod annonces;
mod recensement;
mod reponses;
mod verbes;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::HRESULT;

use crate::pont::ecriture::fil::Ordre;
use crate::pont::enumeration::Session;
use crate::pont::erreurs::Erreur;
use crate::pont::latence::Famille;
use crate::pont::projfs::{ContexteProjFs, Etat, PERIODE_HYDRATATION};
use crate::pont::transport::DuNavigateur;
use proto::fichiers::entetes;
use recensement::{mesure_armee, recenser, tout_completer};

/// Period of the expiry sweep.
///
/// ⚠️ **NOT CALIBRATED.** Set, not measured. It bounds the delay with which
/// an expired command is completed: an application therefore waits at worst its
/// budget plus this period.
///
/// ✅ **F4 GAVE WHAT IT TAKES TO JUDGE IT, and what it gives is that it DOES NOT
/// BITE.** The residue `wall-to-wall − Σ(traversals)` — which contains this period,
/// the entry into the callback, the table registration and `PrjCompleteCommand` —
/// is **0.3% to 0.8%** of wall-to-wall from the 100-entry rank on, and 4 to 31 ms in
/// absolute terms. ⚠️ **This residue is NAMED, not MEASURED**: it is a subtraction between
/// two clocks on two machines. It therefore does not calibrate the constant; it
/// establishes that it is not the dominant term. **The judgement in use remains to be
/// made.**
pub const PERIODE_BALAYAGE: Duration = Duration::from_millis(250);

/// Period of the **census**: an `info!` line naming the twelve causes and
/// their counts.
///
/// ⚠️ **NOT CALIBRATED.** Set, not measured, like the five other time constants
/// of this bridge.
///
/// 🔴 **ONE LINE PER PERIOD, NEVER ONE PER FAILURE.** The TURN work item paid
/// 18,619 lines in a few seconds for a per-packet trace, written to a
/// CIFS share from the loop: **the measurement destroyed what it
/// measured**. And the naive alternative — running the bridge with `RUST_LOG=debug` —
/// would produce one line per callback, that is, the same defect through another
/// door.
///
/// ⚠️ **Shorter than [`PERIODE_HYDRATATION`] (60 s), and on purpose**: the
/// hydration survey states a quantity that grows slowly, the census
/// serves to decide whether an acceptance run exercised what it believes it exercised.
pub const PERIODE_RECENSEMENT: Duration = Duration::from_secs(10);

/// The bridge thread's loop. Returns when the channel closes or the transport
/// stops.
pub fn tourner(etat: Arc<Etat>, entrant: Receiver<DuNavigateur>) {
    // **F4** — force reading `PONT_MESURE` HERE, and not at the first
    // census.
    //
    // ⚠️ **Otherwise the arming trace would only come out after `PERIODE_RECENSEMENT`
    // (10 s)**, that is, AFTER the first gestures of a short acceptance run —
    // and an operator not seeing it would conclude the variable did not
    // reach the process, whereas it did and the line is
    // merely late. *The check's trace must precede what it
    // checks.*
    mesure_armee();
    let mut dernier_releve = Instant::now();
    let mut dernier_recensement = Instant::now();
    loop {
        match entrant.recv_timeout(PERIODE_BALAYAGE) {
            Ok(DuNavigateur::CanalOuvert) => {
                // 🔴 **It is this flag that arms the refusal of writes by STATE.**
                // As long as it is false, `PRE_CONVERT_TO_FULL` returns
                // `ERROR_IO_DEVICE` — the only moment an application can
                // still learn that the browser is not there.
                etat.canal_ouvert.store(true, Ordering::Relaxed);
                tracing::info!("canal du pont ouvert : le navigateur peut servir les requêtes");
            }
            Ok(DuNavigateur::CanalFerme) => {
                etat.canal_ouvert.store(false, Ordering::Relaxed);
                tracing::warn!("canal du pont fermé : les commandes en vol sont abandonnées");
                // 🔴 A command **IN FLIGHT** IS ABANDONED; A COMMAND THAT
                // ARRIVES AFTER IS REFUSED. They are two distinct moments, and
                // they carry two distinct codes — `ERROR_OPERATION_ABORTED`
                // versus `ERROR_IO_DEVICE`.
                //
                // ⚠️ **This site returned `CanalFerme` for both**, and yet the log
                // line just above said "abandoned":
                // the code and its own trace contradicted each other. Measured
                // consequence: `Erreur::Abandonnee` was **defined, counted,
                // translated — and returned by NO production site**. The
                // acceptance run of criterion ④ found it by cutting the channel on a
                // command really in flight: it recorded `canal-ferme=1` where
                // the table promised `abandonnee`.
                tout_completer(&etat, Erreur::Abandonnee);
                // ⚠️ **A LAST LINE AT SHUTDOWN, on BOTH exits.**
                // Without it, a session shorter than `PERIODE_RECENSEMENT`
                // would return NO census — and a criterion (4) read on an
                // empty log would be indistinguishable from a criterion not met.
                // It is D8's `grep` trap: a check that returns zero
                // for two opposite reasons.
                recenser(&etat);
                return;
            }
            Ok(DuNavigateur::Reponse { correlation, trame }) => {
                traiter(&etat, correlation, &trame);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::info!("transport du pont arrêté : le fil du pont s'arrête");
                tout_completer(&etat, Erreur::CanalFerme);
                recenser(&etat);
                return;
            }
        }
        balayer(&etat);
        if dernier_releve.elapsed() >= PERIODE_HYDRATATION {
            etat.tracer_hydratation();
            dernier_releve = Instant::now();
        }
        if dernier_recensement.elapsed() >= PERIODE_RECENSEMENT {
            recenser(&etat);
            dernier_recensement = Instant::now();
        }
    }
}

/// Tells the write thread that one of its correlations is dead.
///
/// **Nothing is done for a ProjFS command**: the write thread only knows
/// its own, and signalling another one to it would make it close a push
/// that is not its own.
pub(super) fn prevenir_l_ecriture(
    etat: &Etat,
    commande: Option<i32>,
    correlation: u32,
    cause: Erreur,
) {
    if commande.is_some() {
        return;
    }
    let code = match cause {
        Erreur::DelaiDepasse => proto::fichiers::CodeEchec::Interne,
        Erreur::CanalFerme => proto::fichiers::CodeEchec::AccesRefuse,
        _ => proto::fichiers::CodeEchec::Interne,
    };
    let _ = etat.vers_ecriture.send(Ordre::Echec { correlation, code });
}

/// Removes expired commands and completes them as timed out.
///
/// **Nothing is replayed**, ever: an expired command whose request we replayed
/// would produce a second response without a recipient, and the browser
/// would answer a correlation the table no longer knows.
fn balayer(etat: &Etat) {
    let echues = match etat.table.lock() {
        Ok(mut table) => table.expirees(Instant::now()),
        Err(_) => return,
    };
    for (commande, correlation) in echues {
        tracing::warn!(
            ?commande,
            correlation,
            "commande expirée : le navigateur n'a pas répondu"
        );
        oublier_contexte(etat, correlation);
        prevenir_l_ecriture(etat, commande, correlation, Erreur::DelaiDepasse);
        verbes::completer(
            etat,
            commande,
            HRESULT(etat.compteurs.rendre(Erreur::DelaiDepasse)),
        );
    }
}

/// Removes the ProjFS context of a correlation, if one remains.
pub(super) fn oublier_contexte(etat: &Etat, correlation: u32) -> Option<ContexteProjFs> {
    etat.en_attente.lock().ok()?.remove(&correlation)
}

/// Handles a browser response.
fn traiter(etat: &Etat, correlation: u32, octets: &[u8]) {
    let trame = match proto::fichiers::decoder(octets) {
        Ok(trame) => trame,
        Err(erreur) => {
            tracing::warn!(correlation, %erreur, "réponse du navigateur illisible, jetée");
            return;
        }
    };
    // ════════════════════════════════════════════════════════════════════
    // 🔴 **F5 — THE FOURTH FAMILY IS ROUTED HERE, AND IT IS THE DECISION THAT
    // CARRIES THE WHOLE SUB-BLOCK.**
    //
    // The announcements going UP — `Bonjour`, `Rafraichir` — have **no
    // correlation in the table**, and their correlation is ignored. Let through
    // to the `resoudre` below, they would fall into its `debug!`
    // "late or unknown response: thrown away" — **invisible under
    // `RUST_LOG=info`**, which is the setting of `scripts/run-agent.sh` and the
    // operations doctrine of this repository. *The button would do nothing, and nothing
    // would say so.*
    //
    // It is the pattern this repository paid for **five times** on
    // `capteur/pont_media.rs` (D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8
    // `PleinEcran`, P1 clipboard) and a sixth on
    // `superviseur/signalisation.rs` (P3, the refusal filed with `ice-config`).
    // **Sixth and seventh variants, and the first time it is closed
    // IN ADVANCE rather than after the fact.**
    //
    // ⚠️ **WHY HERE AND NOT IN `transport.rs`**: the transport only reads
    // a correlation and interprets **no** type. Putting a per-type routing there
    // would make it know the protocol, and it would need one more variant
    // in `DuNavigateur` — whose `match` in `tourner` is **exhaustive**,
    // hence a breaking commit. **A single place, and it is the one that already
    // knows the types.**
    // ════════════════════════════════════════════════════════════════════
    match trame.type_message {
        proto::fichiers::TYPE_RAFRAICHIR => {
            annonces::rafraichir(etat);
            return;
        }
        proto::fichiers::TYPE_BONJOUR => {
            annonces::bonjour(etat, trame.entete);
            return;
        }
        _ => {}
    }

    // ⚠️ **`resoudre` returns `None` for a cancelled, expired or
    // unknown correlation, and the response is then THROWN AWAY.** Applying a response whose
    // ProjFS command has already been completed would write into a buffer the
    // system has taken back.
    let Some((commande, attendue, traversee)) = etat
        .table
        .lock()
        .ok()
        .and_then(|mut t| t.resoudre(correlation, Instant::now()))
    else {
        tracing::debug!(correlation, "réponse tardive ou inconnue : jetée");
        return;
    };
    // **F4** — the only traversal the bridge can measure, and it is
    // observed ONLY if it completed: an expired command does not go through here.
    etat.latences.observer(Famille::de(&attendue), traversee);
    let contexte = oublier_contexte(etat, correlation);

    if trame.type_message == proto::fichiers::TYPE_ECHEC {
        let code = match serde_json::from_slice::<entetes::Echec>(trame.entete) {
            Ok(echec) => Some(echec.code),
            Err(erreur) => {
                tracing::warn!(correlation, %erreur, "échec au code illisible");
                None
            }
        };
        let cause = code.map_or(Erreur::Inattendue, cause_de);
        // ✅ **`warn!` AND NOT `debug!`, AND THE LINE CARRIES THE WIRE CODE IN ADDITION
        // TO THE CAUSE.** *(This trace was a `debug!` carrying only the
        // cause.)*
        //
        // 🔴 **TWO REASONS, AND THE SECOND IS A DECLARED DIVERGENCE.**
        //
        // 1. `scripts/run-agent.sh` sets `RUST_LOG=info` by default, and the
        //    doctrine of this repository is that operations run with it: **on an
        //    ordinary acceptance run, NO cause was observable**. F3's criterion
        //    (4) was unsatisfiable because of it.
        // 2. `cause_de` makes **THREE** distinct wire codes —
        //    `TropGrand`, `Interne` and `CasseAmbigue` — fall back onto the SAME cause
        //    `Inattendue`, hence the same `HRESULT`, whereas spec §5.1
        //    states "two distinct causes never share a code".
        //    **F3 does NOT create a thirteenth variant**: from the
        //    application's point of view, all three are "a defect on our side", and
        //    inventing an `HRESULT` to distinguish our own bugs would
        //    grow a table that criterion (4) then forces us to exercise.
        //    **What F3 fixes is the loss in the LOG**: the distinction
        //    survives where it serves — diagnosis —, and the §5.1 line is
        //    respected in its intention (the old bridge returned `EPERM` at nine
        //    sites) without being so in its letter.
        tracing::warn!(
            ?commande,
            correlation,
            ?code,
            ?cause,
            "le navigateur refuse"
        );
        // A refused write: the protocol code travels AS IS to the
        // thread, which names it in the log. Translating it into an `Erreur` first
        // would lose the distinction between "disk full" and "ambiguous case",
        // which `pont::erreurs` does not carry — and it is the log, not the
        // `HRESULT`, that is the only recipient (see `pont::notifications`).
        if commande.is_none() {
            let _ = etat.vers_ecriture.send(Ordre::Echec {
                correlation,
                code: code.unwrap_or(proto::fichiers::CodeEchec::Interne),
            });
        }
        return reponses::terminer(
            etat,
            commande,
            contexte,
            HRESULT(etat.compteurs.rendre(cause)),
        );
    }

    let issue = reponses::appliquer(
        etat,
        correlation,
        commande,
        attendue,
        &trame,
        contexte.as_ref(),
    );
    match issue {
        reponses::Suite::Termine(resultat) => {
            reponses::terminer(etat, commande, contexte, resultat)
        }
        // The read continues: the command is already re-registered, and its
        // context stayed in place — above all do not complete it.
        reponses::Suite::Poursuit => {}
    }
}

/// The protocol's failure code → the local cause.
///
/// **Exhaustive** `match`: a new protocol code cannot fall into a
/// catch-all arm and silently inherit another's cause — it is the
/// defect `pont::erreurs` exists not to replay.
fn cause_de(code: proto::fichiers::CodeEchec) -> Erreur {
    use proto::fichiers::CodeEchec;
    match code {
        CodeEchec::Introuvable => Erreur::Introuvable,
        CodeEchec::CheminIntrouvable => Erreur::CheminIntrouvable,
        CodeEchec::AccesRefuse => Erreur::AccesRefuse,
        CodeEchec::ProtegeEnEcriture => Erreur::ProtegeEnEcriture,
        CodeEchec::NonSupporte => Erreur::NonSupporte,
        // A range larger than what the browser can return. The bridge
        // already splits at `TAILLE_TRAME_MAX`; receiving this code signals a
        // constant disagreement between the two ends, not a runtime
        // condition.
        CodeEchec::TropGrand => Erreur::Inattendue,
        CodeEchec::Interne => Erreur::Inattendue,
        // F2's THREE codes. They arise from a WRITE push, hence
        // from a command that completes no ProjFS callback.
        //
        // 🔴 **NONE OF THEM REACHES A WINDOWS APPLICATION, and the translation
        // below ONLY SERVES THE LOG.** By the time they arrive,
        // the application has long closed its handle and believed it
        // saved: there is nothing left to complete. Without this sentence, a
        // successor would read `ERROR_DISK_FULL` as a code returned to someone.
        CodeEchec::DisquePlein => Erreur::DisquePlein,
        CodeEchec::DejaPresent => Erreur::DejaPresent,
        // ⚠️ `CasseAmbigue` SHARES `Inattendue` with `TropGrand`, and it is
        // deliberate: `pont::erreurs` has no variant for this refusal,
        // creating one would belong to the complete table of twelve `HRESULT`s of
        // **F3**, and spec §5.1 forbids the same code serving two distinct
        // causes — the constraint bears on the CODE, not on the catch-all,
        // whose very role is to be named as such. What carries
        // the cause is the LOG and the shell page, which name the file.
        CodeEchec::CasseAmbigue => Erreur::Inattendue,
        // 🔵 **THE ONLY ONE FROM F3, AND THE ONLY DIAGNOSTIC ONE.** It is
        // not a catch-all: the browser refuses to delete a NON-EMPTY
        // directory because F3 calls `removeEntry(nom)` **without
        // `recursive`** — a gesture in the VM must not trigger a
        // recursive destruction on the local workstation on the strength of a mirror no
        // proof says is up to date. Receiving it therefore means **the mirror has
        // drifted**, and `ERROR_DIR_NOT_EMPTY` is exactly what a
        // successor will look for in the log.
        CodeEchec::RepertoireNonVide => Erreur::RepertoireNonVide,
    }
}

/// Fills an entry buffer from an already loaded session.
///
/// Exposed because the `GetDirectoryEnumeration` callback borrows it
/// **directly** when the session is already loaded: it is the nominal case
/// after the first turn, it does not consult the browser, and going back through the
/// bridge thread would make a thread round trip for a list already in memory.
///
/// ⚠️ **It is the only work a callback does itself**, and it is bounded:
/// a copy of names into a buffer ProjFS provided, under the sessions
/// lock, **without any I/O**. The threading discipline forbids waiting, not
/// computing.
pub fn remplir_session(
    etat: &Etat,
    session: &mut Session,
    tampon: windows::Win32::Storage::ProjectedFileSystem::PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> HRESULT {
    verbes::remplir(etat, session, tampon)
}
