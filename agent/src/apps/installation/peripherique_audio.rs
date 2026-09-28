//! The default render device, read BEFORE and AFTER an installation.
//!
//! 🔴 WHY THIS MODULE EXISTS, AND IT IS NOT A THEORETICAL PRECAUTION.
//! **On 19 August 2026, installing VB-Cable switched Windows' default
//! render device to a virtual cable that nothing feeds**, and the loopback
//! of work stream A — which followed that default — started capturing silence **without
//! any log line saying why**. The diagnosis cost a
//! campaign. **It is the NOMINAL case, not an edge case**: every audio
//! installer will replay it.
//!
//! 🔴 TWO LINES, BEFORE AND AFTER, WITH THE SAME FIELD NAME AND A
//! `moment=before|apres`. Tracing only AFTER would make a change
//! UNATTRIBUTABLE — we would know which device is the default, never whether it
//! already was. That is exactly what cost the campaign.
//!
//! ⚠️ **NO RESTORATION, NO SAFEGUARD** (spec D8, plan decision D19).
//! Forcibly putting back the previous device would decide on behalf of
//! the user who has just installed an audio device ON PURPOSE. This module
//! observes; it does not repair, and it does not pretend to.
//!
//! ⚠️ THIS MODULE CARRIES A `cfg`, like `execution`. There is nothing pure to
//! take out of it: the device selection rule is already PURE and already tested
//! elsewhere (`wasapi_peripherique`, work stream A-bis); here we only
//! query it and write a line.

use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

/// Where the installation stands when we take the reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    Before,
    After,
}

impl Moment {
    fn mot(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

/// Reads the captured render device and writes it to the log.
///
/// ⚠️ **IT RETURNS NOTHING, AND CANNOT FAIL FOR THE CALLER.** An
/// impossible reading must not prevent an installation: it is logged as
/// `warn!` and we carry on. The reverse — refusing to install because we could
/// not name a sound card — would be a far worse failure than the risk
/// it claims to cover.
pub fn tracer(installation: &str, moment: Moment) {
    match relever() {
        Ok(identifiant) => tracing::info!(
            installation,
            moment = moment.mot(),
            peripherique_rendu = %identifiant,
            "default render device, around an installation"
        ),
        Err(error) => tracing::warn!(
            installation,
            moment = moment.mot(),
            %error,
            "default render device unreadable: a possible \
             change will not be attributable"
        ),
    }
}

fn relever() -> anyhow::Result<String> {
    // ⚠️ THE COM APARTMENT IS THE CALLER'S. This thread is the installation
    // thread, which has already called `CoInitializeEx` — as the discovery
    // thread does for `IShellLinkW`. Doing it again here would be at best
    // useless, at worst a change of apartment model under the feet
    // of a live object.
    //
    // SAFETY: `CoCreateInstance` on a thread whose apartment is initialised.
    let enumerateur: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
    crate::wasapi::rendu::identifiant_capte(&enumerateur)
}
