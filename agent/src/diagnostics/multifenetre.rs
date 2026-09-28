//! Probe prior to work stream D: which capture path returns a correct
//! frame per window when windows overlap, and at what cost.
//!
//! Spec : `docs/superpowers/specs/2026-07-30-sonde-capture-multifenetre-design.md`.
//!
//! Each path is enabled by ITS OWN environment variable, and each
//! run of the binary tests only one: `captureservice.dll` crashed
//! with `0xc0000005` at milestone 1, and a crash of that kind takes down the
//! whole process. Testing them together would lose the others with the
//! first.

pub(super) mod banc;
pub(super) mod capture_virtuelle;
pub(super) mod compteurs;
pub(super) mod contrat;
pub(super) mod disponibilite;
pub(super) mod mires;
// `pub(crate)` and not `pub(super)`: two PRODUCTION consumers still read
// here. `moniteurs_virtuels::purge` (promoted out of this tree in
// task 4) takes `relever_topologie` and `DELAI_TOPOLOGIE` from it;
// `superviseur::boucle` takes `relever_topologie` and `noms_attaches`,
// that is the PAIRING of a freshly created output with its DXGI place —
// the central piece of the set-up. `SEARCH_CEILING`, on the other hand, is no longer
// borrowed: it now lives in `moniteurs_virtuels::numeros` (fix I1).
mod mode_sortie;
pub(crate) mod montee;
pub(super) mod nvenc;
pub(super) mod paralleles;
pub(super) mod plafond;
pub(super) mod pointeur_virtuel;
pub(super) mod recyclage;
pub(super) mod replis;
pub(super) mod reprise;
pub(super) mod voies;
pub(super) mod wgc;

use anyhow::{Context, Result};

/// True if the variable is set **to anything other than `0`**.
///
/// **Never enable a probe on the mere PRESENCE of its variable.** Three
/// of them create virtual monitors that **outlive the process**:
/// read by presence, `MULTIFENETRE_VDD=0` and `MULTIFENETRE_VDD_VEILLE=0`
/// launched their measurement and left up to ten outputs behind them, where
/// anyone writing `=0` asks for the opposite. The project already compares with `"0"`
/// elsewhere (`main.rs`, `INPUT_LINEARITY_NEUTRALISER`).
///
/// The pattern applies to **all** probes read by presence, including
/// those that create no state: a single exception, and it is the one that gets
/// forgotten. Accepted corollary on `MULTIFENETRE_VDD_VEILLE`, whose value also serves
/// as a duration (`montee.rs`): `=0` now switches the probe off instead of
/// asking for a zero standby.
fn sonde_demandee(variable: &str) -> bool {
    std::env::var(variable).is_ok_and(|value| value != "0")
}

/// Returns `true` if a probe of this work stream ran.
pub(super) fn aiguiller() -> Result<bool> {
    // Sub-block D3 — the minimal probe, launched by the bearer of the concurrency
    // cap. **At the head of the WHOLE routing**, not only before the
    // bearer: `run-agent.sh` passes on all the
    // `MULTIFENETRE_*` switches, and the child that `conduire_les_sondes` launches inherits
    // them all — `MULTIFENETRE_PLAFOND` is explicitly removed before the
    // `spawn`, but nothing removes `MULTIFENETRE_VDD_PURGE`,
    // `MULTIFENETRE_DXGI`, etc. if an operator left them in the
    // environment of a previous session. If one of these six branches
    // came before this one, a leftover `MULTIFENETRE_VDD_PURGE=1` would make
    // `purger()` run in the child — which deterministically removes the
    // GUIDs `1..PLAFOND_NUMEROS`, hence the bearer's LIVE outputs, in the
    // middle of the measurement, without any trace linking it to the real
    // cause. Low probability (operator error), maximal and
    // silent consequence: the probe must win whatever lingers
    // elsewhere in the environment.
    //
    // The EMPTY STRING is rejected, for its part: the six neighbouring branches go through
    // `sonde_demandee`, and this one cannot — it needs the value,
    // which is the list of outputs. Taken on mere presence, a
    // `MULTIFENETRE_PLAFOND_SONDE=` exported (or emptied) in a shell would hijack
    // the WHOLE routing, since this branch comes first: the process
    // would probe zero outputs and return `OK` instead of running the requested
    // switch. `is_empty` after `trim`: a value that only carries
    // separators names no output either.
    match std::env::var("MULTIFENETRE_PLAFOND_SONDE") {
        Ok(list) if !list.trim().is_empty() => {
            let sorties: Vec<String> = list.split(',').map(|s| s.trim().to_string()).collect();
            plafond::sonder(&sorties)?;
            return Ok(true);
        }
        Ok(_) => tracing::warn!(
            "MULTIFENETRE_PLAFOND_SONDE posée mais vide : sonde ignorée, aiguillage poursuivi"
        ),
        Err(_) => {}
    }
    // DXGI survey: which outputs exist, which one carries the desktop.
    if sonde_demandee("MULTIFENETRE_DXGI") {
        disponibilite::relever_dxgi()?;
        return Ok(true);
    }
    // Path 1: Windows.Graphics.Capture, honest re-test (abandoned at
    // milestone 1 — see the header comment of `wgc.rs`).
    if sonde_demandee("MULTIFENETRE_WGC") {
        wgc::eprouver()?;
        return Ok(true);
    }
    // Path 4: the per-window fallbacks, probed last — the least
    // promising (see the header comment of `replis.rs`).
    if sonde_demandee("MULTIFENETRE_REPLIS") {
        replis::eprouver()?;
        return Ok(true);
    }
    // Phase 2: the bench, on the requested path and number of windows.
    //
    // `MULTIFENETRE_SORTIE` (a DXGI output name, `\\.\DISPLAYn`) designates
    // an output other than the desktop's. When absent, the bench measures the
    // desktop — its original behaviour. It serves to replay BY HAND the
    // bench on an output known to be alive; measurement ③, for its part, goes
    // through `MULTIFENETRE_VDD_CAPTURE`, since the virtual output does not survive the
    // process that creates it.
    //
    // A name, no longer an `adapter:output` index pair: it is the same
    // change as `SORTIE_DXGI` (`main.rs`), for the same reason — the
    // indices are positional and `DesktopCapture::sur_sortie` no longer takes them.
    if let Ok(voie) = std::env::var("MULTIFENETRE_BANC") {
        let count: u8 = std::env::var("MULTIFENETRE_N")
            .unwrap_or_else(|_| "8".to_string())
            .parse()
            .context("MULTIFENETRE_N doit être un entier")?;
        let sortie = std::env::var("MULTIFENETRE_SORTIE").ok();
        banc::executer(&voie, count, sortie.as_deref())?;
        return Ok(true);
    }
    // Measurement ① — validation of the virtual display driver's contract, before
    // any output is created. The interface GUID and 2 of the 6 IOCTL codes
    // are confirmed byte by byte in the installed DLL; the buffer layout,
    // for its part, is an upstream reading of a header eleven months older
    // than the driver. Without this probe, the next measurement would discover a wrong
    // contract AT THE SAME TIME as it takes its measurement, and the two failures would be
    // indistinguishable. It creates no monitor.
    if sonde_demandee("MULTIFENETRE_CONTRAT") {
        contrat::valider_contrat()?;
        return Ok(true);
    }
    // Cleanup: destroys the virtual outputs left by a run
    // killed outright, which the guard of `moniteurs_virtuels::Sorties` cannot
    // cover. Placed before ANY probe that creates outputs — not only
    // `MULTIFENETRE_VDD`, but also `MULTIFENETRE_VDD_VEILLE` just
    // below, which creates one as well: if both variables are
    // set, a measurement must never win over a requested purge.
    if sonde_demandee("MULTIFENETRE_VDD_PURGE") {
        crate::moniteurs_virtuels::purge::purger()?;
        return Ok(true);
    }
    // Sub-block D3 — the bearer: K = P×D virtual outputs, P probes. The
    // probe itself (`MULTIFENETRE_PLAFOND_SONDE`) is routed at the head of
    // this function, not here: see the comment at that place for
    // why it must win over any other switch.
    if let Ok(value) = std::env::var("MULTIFENETRE_PLAFOND") {
        let (processus, duplications) = plafond::analyser(&value)?;
        plafond::mesurer(processus, duplications)?;
        return Ok(true);
    }
    // P1 of sub-block D8: does a virtual output accept a mode other
    // than the one it was created with? Enumeration is not enough — a driver can
    // advertise a mode and refuse it. Creates an output, hence comes after
    // `MULTIFENETRE_VDD_PURGE`, like any neighbouring probe that creates one.
    // No variable of this file shares its prefix, but the rule is
    // respected on addition: placed before `MULTIFENETRE_POINTEUR` by mere
    // order of introduction, not out of necessity.
    if let Ok(consigne) = std::env::var("MULTIFENETRE_MODE_SORTIE") {
        mode_sortie::executer(&consigne)?;
        return Ok(true);
    }
    // Task 1 of work stream D1: does the virtual desktop extend to a
    // virtual output, and does the pointer get there? Creates an output, hence
    // comes after `MULTIFENETRE_VDD_PURGE`.
    if std::env::var("MULTIFENETRE_POINTEUR").is_ok() {
        pointeur_virtuel::sonder()?;
        return Ok(true);
    }
    // Measurement ① — the watchdog test, prerequisite to scaling up in N.
    // It comes BEFORE `MULTIFENETRE_VDD` in this routing because it is its
    // validity condition: the driver advertises `delai = 3` in an undocumented
    // unit, and if that unit is the second, a scale-up in N without pings
    // would measure the watchdog's ceiling and not the driver's.
    if sonde_demandee("MULTIFENETRE_VDD_VEILLE") {
        montee::eprouver_chien_de_garde()?;
        return Ok(true);
    }
    // Measurement ① of the spec: how many simultaneous virtual outputs this
    // driver accepts. Without this figure, the "one virtual monitor per
    // window" path cannot be specified.
    if sonde_demandee("MULTIFENETRE_VDD") {
        montee::monter_en_n()?;
        return Ok(true);
    }
    // Measurement ③: image correctness on the virtual output, never measured
    // by the probe — path 2 was only guaranteed there "by construction".
    // It creates an output, hence it comes after `MULTIFENETRE_VDD_PURGE`.
    if let Ok(texte) = std::env::var("MULTIFENETRE_VDD_CAPTURE") {
        let count: u8 = texte
            .parse()
            .context("MULTIFENETRE_VDD_CAPTURE doit être un entier (nombre de mires)")?;
        capture_virtuelle::capturer_sur_virtuelle(count)?;
        return Ok(true);
    }
    // The measurement of this work stream: N virtual outputs, one window and one
    // DXGI duplication each — the arrangement the recommended path
    // actually proposes. It creates outputs, hence it comes after
    // `MULTIFENETRE_VDD_PURGE`.
    if let Ok(texte) = std::env::var("MULTIFENETRE_VDD_PARALLELE") {
        let count: u8 = texte
            .parse()
            .context("MULTIFENETRE_VDD_PARALLELE doit être un entier (nombre de sorties)")?;
        paralleles::mesurer(count)?;
        return Ok(true);
    }
    // The test of sub-block D2's founding inference: do k running duplications
    // resume after an output is created on top? Stop point
    // of the acceptance run (spec §6.1). Creates outputs — k, plus the disruptor —
    // hence comes after `MULTIFENETRE_VDD_PURGE`.
    if let Ok(texte) = std::env::var("MULTIFENETRE_REPRISE") {
        let count: u8 = texte
            .parse()
            .context("MULTIFENETRE_REPRISE doit être un entier (nombre de duplications)")?;
        reprise::mesurer(count)?;
        return Ok(true);
    }
    // Pivot measurement of sub-block D5: does destroying an encoder free the
    // slot? Placed BEFORE `MULTIFENETRE_NVENC`: both variables share a
    // common prefix, and the order makes the intent unambiguous if both
    // are set by mistake.
    if let Ok(texte) = std::env::var("MULTIFENETRE_NVENC_CYCLES") {
        let cycles: usize = texte
            .parse()
            .context("MULTIFENETRE_NVENC_CYCLES doit être un entier (nombre de recyclages)")?;
        recyclage::mesurer(cycles)?;
        return Ok(true);
    }
    // Measurement ②: the encoder ceiling, on a shared device (the probe's
    // measurement) or on separate devices (the question it leaves open).
    if let Ok(mode) = std::env::var("MULTIFENETRE_NVENC") {
        nvenc::plafond(&mode)?;
        return Ok(true);
    }
    // Task 6 of work stream D1: the window detection hook
    // (`SetWinEventHook`), tested outside the fake descriptions of
    // `fenetres` — the only way to know whether the filter holds on real
    // Windows windows (menus, dialog boxes) and whether the message
    // pump really keeps the `WINEVENT_OUTOFCONTEXT` callback alive. `sonde_demandee`
    // and not mere presence: this probe does not outlive the process (the
    // `Hook` guard removes the hook when this branch returns), but the
    // file's pattern admits no exception.
    if sonde_demandee("SUPERVISEUR_HOOK") {
        // No `#[cfg(windows)]` here: this whole module is already placed
        // behind `#[cfg(windows)]` in `diagnostics.rs`.
        let (tx, rx) = std::sync::mpsc::channel();
        for (fenetre, titre) in crate::superviseur::hook::enumerer_existantes() {
            tracing::info!(id = fenetre.0, titre, "fenêtre déjà ouverte");
        }
        let _garde = crate::superviseur::hook::poser(tx)?;
        tracing::info!("hook posé — ouvrez et fermez des fenêtres pendant 60 s");
        let fin = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while std::time::Instant::now() < fin {
            match rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(evenement) => tracing::info!(?evenement, "événement de fenêtre"),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "canal du hook rompu");
                    break;
                }
            }
        }
        return Ok(true);
    }
    Ok(false)
}

/// Full chain of an error's causes, from the outermost context to the
/// underlying HRESULT — without it, an error contextualised by
/// `H264Encoder::new` (e.g. `.context("partage du périphérique D3D avec
/// l'encodeur")`) would only show that context and lose the native
/// error code. See the equivalent defect fixed in task 6 of the probe.
pub(super) fn causes(error: impl Into<anyhow::Error>) -> String {
    error
        .into()
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(" : ")
}
