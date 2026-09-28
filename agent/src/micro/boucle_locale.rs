//! **PURE — no `cfg`.** The local loop guard: is the endpoint
//! E2 is going to write to the one work stream A's loopback captures?
//!
//! 🔴 **Measured necessary on 20 August 2026, and the spec says the opposite.** Its
//! §3 asserts "CABLE Input is not the default render device,
//! so work stream A's loopback does not capture it. […] No local loop
//! is created by construction" — **false in all three sentences**: the survey
//! of 20 August 2026 returns `Haut-parleurs (VB-Audio Virtual Cable)` as default
//! render on all **three** roles, and `LoopbackCapture::open` captures
//! Windows' default when `AUDIO_PERIPHERIQUE` is absent. In
//! single-window mode, the agent would therefore capture the voice E2 has just written and
//! send it back to the browser, with the latency of the full round trip; without a headset, the
//! acoustic loop would close through the speakers.
//!
//! **It is the MICROPHONE that gives way, never the sound** (Decision 3): sound is a
//! work stream delivered since A, the microphone is what we are adding.
//!
//! ⚠️ **Not to be confused with the echo of E1's Decision 7** — that one is
//! acoustic, multi-window and out of our reach; this one is
//! intra-agent, single-window, and of our own making.
//!
//! ⚠️ **Multi-window is structurally safe**:
//! `pour_processus` has never resolved an endpoint —
//! `ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK)` targets
//! a **process tree**. The loop only exists in
//! single-window mode, that is in the mode where all the audio acceptance runs of this
//! repository are played.

use crate::wasapi_peripherique::{choisir, Choix, Peripherique};

/// What the guard concludes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boucle {
    /// Nothing to fear: process loopback, audio off, or two distinct
    /// endpoints.
    Absent,
    /// The loopback would capture the cable we are going to write to.
    Risque,
}

/// `capte` is the endpoint identifier THIS process's loopback
/// would capture, or `None` when it captures none (process loopback, or
/// `AUDIO=0`). `cable` is the cable's identifier.
///
/// ⚠️ **Comparison on the IDENTIFIER, never on the name**: two devices
/// can carry the same friendly name — the VM has two starting with
/// "Haut-parleurs (…)" —, and the repository paid in D1 for having designated an
/// output by a rank rather than by a stable identifier.
pub fn evaluer(capte: Option<&str>, cable: &str) -> Boucle {
    let Some(capte) = capte else {
        return Boucle::Absent;
    };
    // ⚠️ Empty is NOT an identifier: `decrire` returns `String::new()`
    // on an indescribable device, and two empties would be equal. Without
    // this exception, an agent whose captured device is indescribable
    // would lose its microphone without any loop existing.
    if capte.is_empty() || cable.is_empty() {
        return Boucle::Absent;
    }
    if capte.eq_ignore_ascii_case(cable) {
        Boucle::Risque
    } else {
        Boucle::Absent
    }
}

/// What THIS process's loopback would capture, **without opening anything
/// at all**: `Some(identifier)` when `AUDIO_PERIPHERIQUE` elects a
/// device, `None` when it is Windows' default that will be captured.
///
/// 🔴 **This function MUST return the same verdict as `wasapi::rendu::resoudre`,
/// and that is all it has that is difficult to do.** The guard above compares
/// what we capture with what we write to: if the two rules diverged by one
/// case, the guard would compare the wrong device and be wrong in both
/// directions — letting a real loop through, or cutting a healthy microphone. The
/// three fallback branches of `resoudre` (`Defaut`, `Introuvable`, `Ambigu`)
/// therefore all return `None` here, exactly as all three fall back
/// on `GetDefaultAudioEndpoint`.
///
/// ⚠️ **Why it lives HERE and not in `wasapi/peripherique.rs`.** Two
/// reasons, in this order: it is the loop guard that needs this
/// verdict and no one else, and `peripherique.rs` is at 461 lines (margin
/// 39) while this file has close to 400 — this repository extracts before
/// adding, it does not compress afterwards.
///
/// ⚠️ **`None` is NOT "no device captured".** It is "Windows'
/// default", which only a COM call can name. The "nothing captured
/// at all" case — process loopback, `AUDIO=0` — does not go through here: it is decided
/// before, in the caller, and arrives at [`evaluer`] in the form of a `None`
/// for its own `capte` parameter.
pub fn identifiant_capte(disponibles: &[Peripherique], demande: Option<&str>) -> Option<String> {
    match choisir(disponibles, demande) {
        Choix::Elu { peripherique, .. } => Some(peripherique.identifiant.clone()),
        // The three fallbacks of `resoudre`, together: in all three cases it is
        // Windows' default that will be captured.
        Choix::Defaut | Choix::Introuvable { .. } | Choix::Ambigu { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The identifiers are those **surveyed on the VM on 20 August 2026** by
    /// `micro-format-e1.ps1`: the cable and the Steam speakers.
    const CABLE: &str = "{0.0.0.00000000}.{deec1914-6490-47ee-9475-091b9a2ea537}";
    const AUTRE: &str = "{0.0.0.00000000}.{8695a111-abf2-4199-88c0-fe4a9176f3e9}";

    /// 🔴 **The defect measured on 20 August 2026.**
    #[test]
    #[allow(non_snake_case)]
    fn le_cable_capte_par_le_loopback_est_un_RISQUE() {
        assert_eq!(evaluer(Some(CABLE), CABLE), Boucle::Risque);
    }

    #[test]
    fn deux_endpoints_distincts_ne_bouclent_pas() {
        assert_eq!(evaluer(Some(AUTRE), CABLE), Boucle::Absent);
    }

    #[test]
    fn sans_capture_il_n_y_a_pas_de_boucle() {
        assert_eq!(evaluer(None, CABLE), Boucle::Absent);
    }

    /// Windows returns its endpoint GUIDs in lower case, but nothing
    /// forces it to. The identifiers are pure ASCII — braces, dots,
    /// hexadecimal digits —, hence `eq_ignore_ascii_case`.
    #[test]
    #[allow(non_snake_case)]
    fn la_comparaison_est_INSENSIBLE_a_la_casse_de_l_identifiant() {
        assert_eq!(evaluer(Some(&CABLE.to_uppercase()), CABLE), Boucle::Risque);
    }

    /// 🔴 This test cannot be deduced from the statement: it comes from a
    /// **written** property of `wasapi::rendu::decrire`, "never returns an error […]
    /// an empty string". Two empties would be equal and would trigger a false
    /// `Risque` — an agent whose captured device is indescribable
    /// would lose its microphone for no reason.
    #[test]
    #[allow(non_snake_case)]
    fn un_identifiant_VIDE_ne_boucle_pas() {
        assert_eq!(evaluer(Some(""), ""), Boucle::Absent);
        assert_eq!(evaluer(Some(""), CABLE), Boucle::Absent);
        assert_eq!(evaluer(Some(CABLE), ""), Boucle::Absent);
    }
}

#[cfg(test)]
#[path = "boucle_locale/tests_capte.rs"]
mod tests_capte;
