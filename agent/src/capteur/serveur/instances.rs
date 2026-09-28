//! Creating and accepting the instances of the sensor's named pipe.
//!
//! **Extracted from `serveur.rs` on 6 August 2026**, which was at 490 lines for a
//! cap of 500 — `CLAUDE.md` requires for this file "an extraction, never
//! a compression of the `TAMPON` comment", and it is indeed the comment
//! on `TAMPON` that moves here WITH its constant, next to which it must
//! stay. **No value, no order of operations changed.**

use std::time::Duration;

use anyhow::{bail, Context, Result};
use windows::core::HRESULT;
use windows::Win32::Foundation::{ERROR_PIPE_CONNECTED, HANDLE};
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use crate::capteur::protocole::NOM_TUBE;

/// Pipe buffer, in both directions. **Sized in access units, not
/// in round megabytes** (fix I4 from the final branch review).
///
/// It is this buffer that sets the REAL depth of the frame queue, and not
/// `CAPACITE_ECRITURES` nor `CAPACITE_FILE`: those two are 8 and reason
/// about ≈90 ms of video, but a larger OS buffer makes them ineffective — it
/// fills up behind them. At 1 MiB, with an access unit weighing 10 to 30 KB, the
/// pipe held 30 to 100 frames, that is **0.3 to 1.1 s of queued video**. Yet
/// each frame carries its original capture instant: a hiccup is not
/// caught up, it is replayed as a burst of old frames.
///
/// 128 KiB brings that down to ≈4 to 12 frames, of the same order as the two capacities
/// above, without going so low that a normal hiccup blocks the capture
/// thread.
///
/// ⚠️ **Latency has never been measured on this path**: the acceptance run of
/// sub-block D4 recorded frame rates, never an end-to-end delay. This
/// sizing is a reasoning on observed access unit sizes,
/// not a calibrated setting.
const TAMPON: u32 = 128 * 1024;

/// Breather between two pipe instance creation attempts after a failure
/// (fix I5). Short enough for a transient failure to cost nothing
/// noticeable to the waiting child, long enough for a persistent failure
/// not to become a tight loop. Accepted upper bound, not calibrated.
pub(super) const SOUFFLE_CREATION_INSTANCE: Duration = Duration::from_millis(200);

/// Blocks until a child connects to `tube`.
///
/// **`ERROR_PIPE_CONNECTED` is a SUCCESS disguised as an error.** It signals
/// that a child connected in the interval between `CreateNamedPipeW` and
/// this call — a mundane race, expected under `PIPE_UNLIMITED_INSTANCES` —
/// and not a failure. Confusing it with a real failure would kill the whole
/// sensor process (hence the N windows with it) at the first race.
pub(super) fn connecter(tube: HANDLE) -> Result<()> {
    match unsafe { ConnectNamedPipe(tube, None) } {
        Ok(()) => Ok(()),
        Err(error) if error.code() == HRESULT::from_win32(ERROR_PIPE_CONNECTED.0) => Ok(()),
        Err(error) => Err(error).context("waiting for a child"),
    }
}

pub(super) fn create_instance() -> Result<HANDLE> {
    let nom: Vec<u16> = NOM_TUBE.encode_utf16().chain(std::iter::once(0)).collect();
    let tube = unsafe {
        CreateNamedPipeW(
            windows::core::PCWSTR(nom.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            TAMPON,
            TAMPON,
            0,
            None,
        )
    };
    if tube.is_invalid() {
        bail!("CreateNamedPipeW returned an invalid handle");
    }
    Ok(tube)
}
