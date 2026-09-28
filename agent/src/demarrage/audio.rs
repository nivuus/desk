//! Construction and wiring of a session's audio source.
//!
//! Extracted from `demarrage.rs` in task 7 of sub-block D7: adding the
//! per-process mode (two branches, plus the `pid_de_fenetre` helper) took
//! the parent file above the repository's 500-line cap — same
//! rule, same remedy as `demarrage/source.rs` (see its head
//! comment): the addition comes with its extraction rather than a
//! compression of the comment it carries.

#![cfg(windows)]

use std::time::Instant;

use crate::transport::Session;
use crate::{windows_audio, Config};

/// Opens the audio source suited to the agent's mode, and wires it to
/// `session` if opening succeeds.
///
/// Its absence never compromises the video session: on a test source
/// (`TEST_FILE`), there is nothing to capture, and if the capture refuses to
/// open, we log and the session carries on, silent.
///
/// **Two modes, and the fallback is NEVER the global mix.** With
/// `FENETRE_HWND`, the child captures the sound of the sole process owning its
/// window, and the sensor arbitrates between the windows that share one.
/// Without it, it captures the session mix: that is the single-window mode from before
/// sub-block D7, and it must not regress.
///
/// Falling back on the global mix when process loopback fails would make
/// a window hear the sound of ALL the others, under the guise of
/// isolation — it is the fallback explicitly ruled out at framing.
pub(super) fn brancher(config: &Config, session: &mut Session, clock_origin: Instant) {
    if config.test_file.is_none() && config.audio {
        let ouverture = match config.fenetre_hwnd {
            Some(hwnd) => match pid_de_fenetre(hwnd) {
                Ok(pid) => windows_audio::WindowsAudioSource::pour_processus(pid, clock_origin),
                Err(e) => Err(e),
            },
            None => windows_audio::WindowsAudioSource::new(clock_origin),
        };
        match ouverture {
            Ok(source_audio) => {
                tracing::info!(
                    format = source_audio.description(),
                    pid = source_audio.pid(),
                    "audio enabled"
                );
                session.set_audio_source(Box::new(source_audio));

                // SINGLE-WINDOW mode: no sensor will ever arbitrate this
                // session, so `appliquer_audio` — the only writer of
                // `audio_porteuse` on the `transport` side — will never be called there
                // (its guard is `VideoSource::audio_a_appliquer`, which returns
                // `None` by default and has `capteur/distante.rs` as its only
                // override). Without this line, any rebuilt capture is
                // put back to silence by the re-arm
                // `set_actif(self.audio_porteuse)` of
                // `reconstruire_ou_signaler`, and D10's remedy is INERT
                // in the MAJORITY case — one application, one window.
                // D10's hand-over 4.
                //
                // ⚠️ `is_none()` and not a copied `match`: the `match` of the
                // source choice above has already consumed the value, and
                // redoing a third branch on the mode here
                // would duplicate what this file has already paid for twice.
                if config.fenetre_hwnd.is_none() {
                    session.set_audio_porteuse(true);
                }

                // The SAME mode choice as above, redone identically.
                // The fallback is NEVER the global mix: a window that
                // would hear all the others under the guise of isolation is
                // the trade-off explicitly ruled out at D7's framing.
                let hwnd = config.fenetre_hwnd;
                session.set_audio_reconstructeur(Box::new(move || {
                    let source = match hwnd {
                        Some(hwnd) => windows_audio::WindowsAudioSource::pour_processus(
                            pid_de_fenetre(hwnd)?,
                            clock_origin,
                        )?,
                        None => windows_audio::WindowsAudioSource::new(clock_origin)?,
                    };
                    Ok(Box::new(source) as Box<dyn crate::audio::AudioSource + Send>)
                }));
            }
            Err(e) => {
                tracing::warn!(error = %e, "audio unavailable, the session goes on without sound");
            }
        }
    }
    if !config.audio {
        tracing::info!("sound disabled on this agent by AUDIO=0");
    }
}

/// The PID of the process owning a window.
///
/// Derived from the `hwnd`, exactly as the sensor derives it on its side
/// (`capteur/fenetre.rs`). The PID travels on no protocol: two
/// independent derivations of the same stable identifier are better than
/// one more field to keep consistent.
fn pid_de_fenetre(hwnd: u64) -> anyhow::Result<u32> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let handle = HWND(hwnd as *mut core::ffi::c_void);
    let mut pid = 0u32;
    // SAFETY: an invalid handle makes it return 0, which the `ensure` catches.
    unsafe { GetWindowThreadProcessId(handle, Some(&mut pid)) };
    anyhow::ensure!(pid != 0, "no PID for window {hwnd:#x}");
    Ok(pid)
}
