//! Routing of the diagnostic modes, all enabled by environment
//! variable. None opens a WebRTC session: they observe and
//! record, they build nothing.
//!
//! "Observe" does not mean "without effect" though: two probes
//! deliberately disturb the state of the Windows session, because it is the
//! only way to obtain the measurement. `INPUT_LINEARITY_PROBE` injects real
//! mouse movements and repositions the cursor (`SetCursorPos`), and
//! `CAPTURE_TEST` moves the observed window by one pixel in a loop
//! (`SetWindowPos`) to force the desktop to recompose. Do not
//! run them on a session whose state you want to preserve.

use anyhow::Result;

#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod capture;
mod entree;
#[cfg(windows)]
pub(crate) mod exceptions;
// `pub(crate)` and not private: `moniteurs_virtuels::purge` (production, promoted
// out of this tree) still reads `multifenetre::montee` for the DXGI topology
// and the search cap — the only remaining link in that direction, accepted, see
// the comment of `moniteurs_virtuels/purge.rs`.
#[cfg(windows)]
pub(crate) mod multifenetre;
#[cfg(windows)]
pub(crate) mod pixels;
#[cfg(windows)]
mod presse_papier;

/// Returns `true` if a probe ran — `main` must then stop there.
///
/// The order of the probes reproduces exactly the one `main()` had
/// before the split (except for `PROCESS_LOOPBACK_CAPTURE`, added
/// by sub-block D7 directly before its neighbour `PROCESS_LOOPBACK_PROBE`,
/// the two variables sharing a prefix), and it is not indifferent:
/// `INPUT_LINEARITY_PROBE`
/// (the last) reads `INPUT_LINEARITY_NEUTRALISER`, whose effect depends on the
/// SPI neutralisation that `main()` has already — or not — applied before
/// calling this function. See the comment of `main()` on this.
pub(crate) fn aiguiller() -> Result<bool> {
    // Diagnostic mode: CAPTURE_TEST=firefox checks window finding and capture.
    #[cfg(windows)]
    if let Ok(fragment) = std::env::var("CAPTURE_TEST") {
        capture::executer(&fragment)?;
        return Ok(true);
    }

    // Audio probe (`AUDIO_PROBE=1`): answers questions no. 1 and no. 2 of the
    // specification of work stream A — which render device is KEPT
    // for THIS session, what is its mix format, and does a loopback there
    // really capture what the applications play.
    //
    // ⚠️ "Kept", no longer "default", since the "A-bis" fix: the
    // probe goes through `LoopbackCapture::open`, hence through `AUDIO_PERIPHERIQUE`
    // when it is set. It moreover returns the DOMINANT FREQUENCY of what
    // it captures, and not only a peak — that is what makes it
    // A-bis's measuring instrument, run twice, with and without the
    // variable, on the same machine.
    #[cfg(windows)]
    if std::env::var("AUDIO_PROBE").is_ok() {
        audio::executer_sonde_audio()?;
        return Ok(true);
    }

    // Pivotal measurement of sub-block D7: `PROCESS_LOOPBACK_CAPTURE=<pid>` goes
    // as far as where `PROCESS_LOOPBACK_PROBE` (just below) stops —
    // `Initialize`, `GetService`, `Start`, and a real read. Placed
    // BEFORE this arm: the two variables share a prefix
    // (`PROCESS_LOOPBACK_`), the exact trap of `MULTIFENETRE_NVENC_CYCLES` in
    // D5, where the more specific variable must be tested first.
    #[cfg(windows)]
    if let Ok(pid_texte) = std::env::var("PROCESS_LOOPBACK_CAPTURE") {
        audio::executer_capture_process_loopback(&pid_texte)?;
        return Ok(true);
    }

    // Probe no. 4 of work stream A's spec: *process loopback*
    // (Windows 10 build 19041+) isolates the audio of a single process, which
    // the multi-window model of work stream D requires. The VM is on build
    // 20348, hence eligible on paper. NOTHING IS BUILT ON IT here:
    // we only observe whether activation succeeds, and the result is
    // recorded for work stream D.
    #[cfg(windows)]
    if let Ok(pid_texte) = std::env::var("PROCESS_LOOPBACK_PROBE") {
        audio::executer_process_loopback(&pid_texte)?;
        return Ok(true);
    }

    // Probe of work stream B (§11, unknowns no. 1 and no. 2): does ViGEmBus accept
    // plugging in a virtual Xbox 360 gamepad, and does its
    // notification callback faithfully return the vibration magnitudes a game
    // requests? NOTHING IS BUILT ON IT here: we observe, and the result
    // pins the API actually available for task 10.
    #[cfg(windows)]
    if std::env::var("VIGEM_PROBE").is_ok() {
        entree::executer_vigem()?;
        return Ok(true);
    }

    // Probe no. 1 of work stream B's acceptance run: is aiming linear
    // 1:1? We inject a known sum of relative movements and
    // compare with the cursor's real movement.
    //
    // `INPUT_LINEARITY_NEUTRALISER=0` skips the SPI neutralisation — here AND
    // at the very start of `main()` (see the comment there): that is what
    // makes the measurement demonstrative rather than reassuring — the gap observed
    // without neutralisation quantifies what neutralisation brings. Skipping
    // only the call below, without touching the start-up one,
    // would have let the latter neutralise the session even before the
    // probe runs (a real bug of the first version of this
    // task, fixed in review round 1).
    #[cfg(windows)]
    if std::env::var("INPUT_LINEARITY_PROBE").is_ok() {
        entree::executer_linearite()?;
        return Ok(true);
    }

    // Probe P0 of sub-block P1 (clipboard): `PRESSE_PAPIER_SONDE=<secondes>`
    // measures what the specification (§8) declares NOT MEASURED — does the
    // `GetClipboardSequenceNumber` counter exist, is it stable at rest, does it move
    // on a copy, and does it move on an IDENTICAL REWRITE. It is an eliminatory
    // gate: three of its five verdicts make the chosen detection mechanism
    // non-shippable as is.
    //
    // ⚠️ This probe WRITES the VM's clipboard (phases C and D) and therefore destroys
    // it. The product, for its part, never writes it in P1.
    //
    // 🔴 **This variable must NEVER coexist with `SUPERVISEUR`** — it is
    // the plan's divergence E10. `main()` calls `diagnostics::aiguiller()` at
    // `main.rs:172`, BEFORE the `CAPTEUR` branch (`:180`), before enrolment,
    // before `PONT` (`:279`) and before the supervisor branch: **whatever the
    // requested mode**, an agent carrying this variable runs the probe and
    // stops.
    //
    // ❌ **THE MECHANISM E10 DESCRIBES IS NOT REACHABLE, and writing it here
    // is better than letting a false threat run** (task 14, 20 August
    // 2026). E10 announces that the probe would be run "by the SENSOR
    // process, which would stop at once — and the supervisor would restart it in
    // a loop". That would assume the CHILD carries the variable and not its PARENT.
    // Yet `superviseur/lanceur.rs` launches its children through `std::process::Command`,
    // which inherits the parent's environment: if the sensor carries it, the
    // supervisor already carried it — and so it stopped at `main.rs:172`,
    // BEFORE having launched anything. There is no path, in this
    // repository, that sets this variable on a child without having set it on its
    // parent: `run-agent.sh` writes a single bootstrap script, and `lanceur.rs`
    // never adds a diagnostic variable.
    //
    // ✅ **WHAT IS TRUE, AND IS ENOUGH TO JUSTIFY THE SAME INSTRUCTION**: the
    // variable degenerates ANY agent launch into a probe, supervisor included.
    // The symptom is not a loop but a silence — no session
    // is established, and the probe's output is the only clue. **The probe is
    // run ALONE**, without `SUPERVISEUR`.
    //
    // ⚠️ The plan sets NO `env_remove` for this variable, and its reason
    // ("it would be a convention the eight `MULTIFENETRE_*` do not
    // follow") is weak in view of the doctrine `lanceur.rs` carries in its
    // own code — "a test order is a property that changes, an
    // `env_remove` is not", written on 20 August 2026 when removing the platform
    // identity from the children. **But the conclusion holds for ANOTHER reason,
    // and it is decisive**: the parent stops before reaching `lanceur.rs`,
    // so an `env_remove` set there would prevent NOTHING. The only remedy that
    // would bite would be to move the probe routing after the mode
    // branches, which would change the contract of `diagnostics::aiguiller` for
    // ALL its variables — seven read right here, plus those
    // `multifenetre::aiguiller` reads at the end. Out of P1's scope, and named
    // here rather than dormant. ⚠️ This sentence announced "its twelve
    // variables": no count gives twelve, and a number that cannot be
    // redone is worse than no number (cross-cutting review,
    // 20 August 2026).
    #[cfg(windows)]
    if let Ok(secondes) = std::env::var("PRESSE_PAPIER_SONDE") {
        presse_papier::executer(&secondes)?;
        return Ok(true);
    }

    // Probes of work stream D (multi-window capture). Placed last:
    // they create their own windows and interfere with none of the
    // probes above, but they disturb the desktop layout.
    #[cfg(windows)]
    if multifenetre::aiguiller()? {
        return Ok(true);
    }

    Ok(false)
}
