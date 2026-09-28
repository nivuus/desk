//! The bridge's **census**: the line of the twelve causes, the one of F4's
//! latency bench, and the mass completion accompanying them.
//!
//! **EXTRACTED RATHER THAN COMPRESSED**, and **BEFORE the addition that required it** —
//! the repository's doctrine is to extract before having crossed the
//! ceiling, not after. `service.rs` was at **458 lines for a gate at
//! 460** (F4 plan, §2.4) when F4 wired its histogram there; the three blocks
//! below left it **verbatim**, doc comments included.
//!
//! ⚠️ **An ORDINARY `mod`, inside its parent**: there is no
//! `#[cfg(windows)]` boundary to cross here — all of `pont::service` is already
//! `#[cfg(windows)]` —, so `CLAUDE.md`'s `#[path]` convention **does not
//! apply**, as for D11's two extractions.

use std::sync::atomic::Ordering;
use std::time::Instant;

use windows::core::HRESULT;

use super::{oublier_contexte, prevenir_l_ecriture, verbes};
use crate::pont::errors::Error;
use crate::pont::projfs::Etat;

/// **F4** — is the latency bench armed?
///
/// 🔴 **`=1` ARMS; ABSENCE disarms.** It is `MICRO_MESURE`'s convention,
/// **and NOT that of `PONT`, `PONT_ECRITURE` and `PONT_MUTATION`**, which are
/// disarmed by `=0`. The repository's rule is: *we disarm on `=0` what is
/// SHIPPED, we arm on `=1` what is not.* The bridge, writing and
/// mutations are shipped; the latency line is a **bench instrument,
/// never a shipped configuration**.
///
/// ⚠️ **What it arms is EMISSION, not COLLECTION**: see
/// [`crate::pont::projfs::Etat::latences`].
pub(super) fn mesure_armee() -> bool {
    static ARMEE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ARMEE.get_or_init(|| {
        let armee = std::env::var("PONT_MESURE")
            .map(|v| v == "1")
            .unwrap_or(false);
        if armee {
            // ⚠️ **`warn!`, like `PART_SONDAGE` and `AUDIO_FAUTE_*`**: it is what
            // makes it visible under `RUST_LOG=info` and what prevents
            // confusing it with an ordinary configuration.
            tracing::warn!(
                "banc de latence du pont ARME (PONT_MESURE=1) : instrument de banc, \
                 jamais une configuration livree"
            );
        }
        armee
    })
}

/// The census line — **the instrument of F3's criterion (4)**.
///
/// ```text
/// codes rendus total=17 introuvable=3 chemin-introuvable=1 acces-refuse=0 …
/// ```
///
/// 🔴 **A CODE NEVER PRODUCED SHOWS `0`, AND THAT IS THE WHOLE POINT.** The
/// criterion (4) — "each of the twelve is observed at least once" — then becomes
/// a `grep` on ONE line, and it **cannot be satisfied by
/// accident**: a run exercising nothing returns twelve zeros.
///
/// ⚠️ **WHAT THIS CENSUS CANNOT SAY, measured by F3's acceptance run**:
/// it is emitted **when the channel closes**, and the service thread RETURNS
/// right after. A code produced AFTER that closing — `CanalFerme` on a gesture
/// arriving when there is no browser any more — is indeed **counted**, and
/// **no one prints it**. The counter is right; the line that makes it
/// observable has already gone.
///
/// ⚠️ **`info!` and not `debug!`**: `scripts/run-agent.sh` sets `RUST_LOG=info`
/// by default, and the doctrine of this repository is that operations run with it. A
/// mute mitigation is not one — it is the reason written for the two
/// traces of `encode/arret.rs`, applied here.
pub(super) fn recenser(etat: &Etat) {
    // ── THE TABLE SURVEY — the instrument of F1's legacy no. 4 ─────────────
    //
    // 🔴 **It is what decides between the four hypotheses**, and none was
    // decidable until now. F1 measured reads that STALL without ever
    // expiring — the expired-command count stays at 0 for 540 s — and declares that we
    // do not know WHERE the blockage happens. See `pont::table::plus_ancienne`,
    // which carries the reading table.
    //
    // ⚠️ **One line every 10 s, never one per callback.** The TURN work item
    // paid 18,619 lines in a few seconds for a per-packet trace,
    // written to a CIFS share from the loop: the measurement destroyed what
    // it measured.
    let maintenant = Instant::now();
    let (en_vol, sans_commande, plus_ancienne_ms) = match etat.table.lock() {
        Ok(table) => (
            table.en_vol(),
            table.sans_commande(),
            table
                .plus_ancienne(maintenant)
                .map(|d| d.as_millis())
                .unwrap_or(0),
        ),
        // ⚠️ **A poisoned lock is SAID, not kept quiet.** Returning zeros would
        // read "nothing in flight" where the table is inaccessible — that is,
        // the FIRST line of the reading table, which would blame the callback.
        Err(_) => {
            tracing::warn!("recensement impossible : le verrou de la table est empoisonne");
            return;
        }
    };
    let sessions = etat.sessions.lock().map(|s| s.len()).unwrap_or(0);
    tracing::info!(
        "pont en vol={} sans_commande={} plus_ancienne_ms={} sessions={} octets_hydrates={} \
         entrees_hydratees={}",
        en_vol,
        sans_commande,
        plus_ancienne_ms,
        sessions,
        etat.octets_hydrates.load(Ordering::Relaxed),
        etat.entrees_hydratees.load(Ordering::Relaxed),
    );

    let manquants: Vec<&str> = etat
        .compteurs
        .manquants()
        .into_iter()
        .map(crate::pont::compteurs::nom)
        .collect();
    tracing::info!(
        // ⚠️ **A `tracing` field would carry ANSI sequences between its name and
        // its value in a RAW log** — it is the trap D8's input acceptance
        // run paid for, and that F1's `grep` replayed three times.
        // The census is therefore **a single string**, `name=value` separated
        // by spaces, and it is read as is without `sed`.
        "codes rendus {} | jamais rendus : {}",
        etat.compteurs.recensement(),
        if manquants.is_empty() {
            "aucun".to_string()
        } else {
            manquants.join(",")
        }
    );

    // ── THE LATENCY BENCH (F4) — emitted ONLY if `PONT_MESURE=1` ───────────
    //
    // ⚠️ **The counters are CUMULATIVE since the bridge started**: a
    // measurement is read by DIFFERENCE between two censuses, never on an
    // isolated line. That is why any measurement step lasts at least six
    // periods.
    if mesure_armee() {
        // ⚠️ **A single string, never `tracing` fields** — same reason as for the
        // census of codes, three lines above.
        tracing::info!("{}", etat.latences.recensement());
    }
}

/// Completes with an error everything left in flight. Called when no more
/// response can arrive.
pub(super) fn tout_completer(etat: &Etat, cause: Error) {
    let restantes = match etat.table.lock() {
        Ok(mut table) => table.drain(),
        Err(empoisonne) => empoisonne.into_inner().drain(),
    };
    for (commande, correlation) in restantes {
        oublier_contexte(etat, correlation);
        // ⚠️ **An abandoned write must be TOLD to the write thread**, otherwise
        // its push would stay "in flight" forever and the queue would no longer
        // advance. The entry, for its part, STAYS in the journal — it is the thread that decides, and
        // it is what makes it recoverable.
        prevenir_l_ecriture(etat, commande, correlation, cause);
        verbes::completer(etat, commande, HRESULT(etat.compteurs.rendre(cause)));
    }
}
