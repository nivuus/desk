//! The minimal probe: D DXGI duplications, held, and as little as possible
//! around them — **"nothing else" has a precise scope, not an absolute one.**
//!
//! **What IS excluded**: encoder (NVENC/Media Foundation), colour
//! converter, window, WebRTC. If the refusal of the 5th duplication observed in
//! sub-block D2 does not reproduce here, the ceiling does not bear on
//! duplication but on one of THOSE elements (hypothesis H3 of the spec)
//! — and that is a result, not a failure of the probe.
//!
//! **What is NOT excluded, and cannot be with the imposed
//! interface**: `DesktopCapture::sur_sortie` calls `ouvrir`, which calls
//! `create_device_and_context` (`agent/src/capture/ouverture.rs:83-139`) — and this
//! function INEVITABLY builds a real `ID3D11Device` + `ID3D11DeviceContext`
//! per duplication (`D3D11CreateDevice` with
//! `D3D11_CREATE_DEVICE_BGRA_SUPPORT`), then sets on them
//! `SetMultithreadProtected(true)` — precisely the preparation documented
//! as necessary for the Media Foundation sharing an encoder would use.
//! The probe follows this interface without modifying it (modifying it was out of
//! scope); it CANNOT open a duplication without this device.
//! **Consequence for reading the campaign**: the "add a
//! D3D11 device" stage of the H3 escalation is already crossed BY CONSTRUCTION at
//! this probe rank — if H3 must be deepened by a thicker probe,
//! the first stage to add there is the encoder, not the device, already
//! present here.
//!
//! The probe's rank comes from `MULTIFENETRE_PLAFOND_RANG`; each line
//! carries it, because `agent.log` mixes the bearer and all its probes through inheritance of
//! `stdout` and nothing else would distinguish the emitter (trap noted in D1).
//!
//! `DesktopCapture::sur_sortie` retries an opening refused for
//! transient unavailability during `capture_reprise::DUREE_FENETRE_OUVERTURE`
//! (3 s). A refusal reported here is therefore **already a lasting refusal**, not a
//! transient — it is no stronger than that.

use anyhow::{Context, Result};

/// The bearer's marker file: its presence orders the exit.
pub(super) fn chemin_arret() -> std::path::PathBuf {
    std::env::temp_dir().join("plafond-arret")
}

/// Verdict file of a probe.
pub(super) fn chemin_verdict(rang: u8) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("plafond-sonde-{rang}.verdict"))
}

/// Fallback verdict of an unreadable rank (FIXED name, see `sonder` below
/// for why). Never read by the bearer — it is only a diagnostic for
/// whoever digs through `%TEMP%` — but named here so that the bearer can
/// clean it up before a draw without duplicating the file name.
pub(super) fn chemin_verdict_rang_invalide() -> std::path::PathBuf {
    std::env::temp_dir().join("plafond-sonde-rang-invalide.verdict")
}

// `pub(in super::super)` and not `pub(crate)`: it is the narrowest
// visibility that still satisfies the `pub(super) use sonde::sonder;` re-export of
// `plafond.rs` — `super::super` designates `multifenetre` from `sonde`, which
// corresponds exactly to the `pub(in multifenetre)` scope this re-export
// declares. A `pub(super)` here (scope `plafond` only) would be too narrow and
// would make this re-export fail with E0364 ("sonder is private, and cannot be
// re-exported") — checked at Task 8. `pub(crate)` would compile too but
// would open `sonder` to the whole crate for no reason: nothing outside
// `multifenetre` needs it.
pub(in super::super) fn sonder(sorties: &[String]) -> Result<()> {
    let rang_brute = std::env::var("MULTIFENETRE_PLAFOND_RANG").unwrap_or_else(|_| "0".to_string());
    let rang: u8 = match rang_brute
        .parse()
        .context("MULTIFENETRE_PLAFOND_RANG must be an integer")
    {
        Ok(rang) => rang,
        Err(error) => {
            // Without this block, the original `?` exited BEFORE any verdict
            // was written: the bearer (Task 10) bounds its wait (see
            // `attendre_le_verdict`, `plafond.rs`) and returns MORTE if nothing
            // arrives, but a malformed rank would then read as a
            // probe crash (0xc0000005 and the like) rather than as what
            // it is. No `u8` exists here to name the
            // file `chemin_verdict` would normally produce: we therefore drop
            // a fallback verdict, under a FIXED name rather than one derived
            // from the raw value — interpolating it into a path component
            // would let through `..` sequences meaningful for
            // Windows resolution (`Path` treats `\` and `/` there as
            // separators) and could write outside `%TEMP%`. The raw
            // value stays in the trace below, where interpolating it poses
            // no risk. Best-effort (a write failure makes nothing worse:
            // this trace remains the reference diagnostic).
            tracing::error!(rang_brute = %rang_brute, %error, "MULTIFENETRE_PLAFOND_RANG unreadable");
            let secours = chemin_verdict_rang_invalide();
            let _ = std::fs::write(&secours, format!("KO RANG_INVALIDE {rang_brute}"));
            return Err(error);
        }
    };

    tracing::info!(sonde = rang, sorties = ?sorties, "probe started");

    // The duplications are HELD in this vector: releasing them would free
    // the slot and the measurement would no longer measure anything.
    let mut tenues = Vec::new();
    let mut verdict = String::from("OK");
    for (rang_local, nom) in sorties.iter().enumerate() {
        match crate::capture::DesktopCapture::sur_sortie(nom) {
            Ok(duplication) => {
                tracing::info!(
                    sonde = rang,
                    duplication = rang_local + 1,
                    %nom,
                    "duplication opened"
                );
                tenues.push(duplication);
            }
            Err(error) => {
                // `{error:#}` and not `{error}`: `anyhow`'s simple Display
                // only renders the OUTERMOST context ("screen output
                // duplication"), and the HRESULT — the only data the
                // matrix uses — would stay in the causes, invisible here.
                // It only survived in the logs of the D3 campaign
                // because `agent::capture::ouverture` logs it on its
                // own account, one line above: a fortuitous dependency on
                // a trace from ANOTHER module, which nothing guaranteed. The
                // alternate format renders the full chain of causes, hence
                // the HRESULT, in the trace as in the verdict.
                let causes = format!("{error:#}");
                tracing::error!(
                    sonde = rang,
                    duplication = rang_local + 1,
                    %nom,
                    error = %causes,
                    "duplication REFUSED"
                );
                verdict = format!("KO {causes} {nom}");
                break;
            }
        }
    }

    // Pre-existing, out of scope of this fix: a write failure
    // HERE (valid rank, legitimate verdict) exits through `?` with no verdict dropped.
    std::fs::write(chemin_verdict(rang), &verdict)
        .with_context(|| format!("writing the verdict of probe {rang}"))?;
    tracing::info!(sonde = rang, ouvertes = tenues.len(), %verdict, "verdict written");

    // Hold until the bearer's signal. The duplications stay open as long
    // as `tenues` is alive.
    while !chemin_arret().exists() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    tracing::info!(sonde = rang, "stop requested, releasing the duplications");
    Ok(())
}
