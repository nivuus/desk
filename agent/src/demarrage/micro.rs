//! **The mic ROUTING**: which of the sinks receives the upstream flow, and
//! why there are never two.
//!
//! Three outcomes, and a single rule (Decision 10 of plan E2) — see
//! [`choisir_puits`]:
//!
//! | `MICRO` | `MICRO_MESURE` | Sink |
//! | --- | --- | --- |
//! | ≠ `0` | `1` | **Measurement** — the bench instrument TAKES PRECEDENCE |
//! | ≠ `0` | other | **Cable** — the nominal sink since block E2 |
//! | `0` | *any* | **None** |
//!
//! 🔴 **Two sinks cannot consume the same `LecteurMicro`**: each
//! drains what the other waits for, and the symptom would be a mic that hiccups
//! without any line saying so. Hence a rule **pure and tested on
//! the host**, rather than two `if`s placed one after the other.
//!
//! The MEASUREMENT sink itself (`MICRO_MESURE=1`) lives in `micro/mesure.rs`:
//! a consumer that plays the cable's role and reports to the log what it
//! heard.
//!
//! ⚠️ **BENCH INSTRUMENT, NEVER A SHIPPED CONFIGURATION.** Hence the
//! `MICRO_MESURE=1` convention that **ARMS** — and not `=0` that would disarm,
//! as `AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR` and `CAPTEUR` do. A mere
//! presence is not enough either: the value `1` is required. The repository's general
//! rule remains "we disarm on `=0` what is shipped, we arm on `=1` what
//! is not".
//!
//! **PURE: no `cfg`, no COM object, no device.** This file
//! compiles and is tested under Linux, unlike its neighbour
//! `demarrage/audio.rs`. That is what lets the part that can really
//! go wrong — the de-interleaving, the one-second window, the direction of the peak
//! — be tested without a VM.

use std::sync::{Arc, Mutex};

use crate::micro::LecteurMicro;
use crate::transport::Session;
use crate::Config;

use mesure::{consommer, PuitsDeMesure};

/// The measurement sink itself, extracted under the 500-line rule.
/// **This file only keeps the routing.**
mod mesure;

/// Which sink receives the upstream flow. See the table at the head of the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Puits {
    /// The virtual cable (`windows_micro`): the nominal sink.
    Cable,
    /// L'instrument de banc (`MICRO_MESURE=1`).
    Mesure,
    /// Aucun : `MICRO=0`. `micro_disponible()` reste faux et `ready` porte
    /// `mic: false`.
    Aucun,
}

/// **PURE, and tested on the host.** The arbitration between the two sinks.
///
/// ⚠️ **It takes two booleans and not `&Config`, contrary to the letter of
/// plan E2 (task 10, step 2).** Two reasons: it is all the rule
/// needs — a `Config` carries eighteen fields, sixteen of which are off topic —, and
/// above all there is no `Config` constructor for tests, so
/// that requiring the whole type would have made the rule testable only at the cost
/// of a set-up. A rule not tested because it costs too much to
/// set up is an untested rule.
pub(crate) fn choisir_puits(micro: bool, micro_mesure: bool) -> Puits {
    if !micro {
        return Puits::Aucun;
    }
    // ⚠️ **The bench instrument TAKES PRECEDENCE**, it is not added: we arm it
    // to observe *instead of* the cable.
    if micro_mesure {
        return Puits::Mesure;
    }
    Puits::Cable
}

/// Does `MICRO` leave the mic armed? **Anything but `0`.**
///
/// ⚠️ **REVERSE convention of that of `MICRO_MESURE` just below, on
/// purpose**: we disarm on `=0` what is SHIPPED, we arm on `=1` what is
/// not. The mic has been shipped since block E2; the measurement sink
/// never will be.
///
/// **Never test `is_ok()`**: someone writing `MICRO=0` to be
/// sure to turn it off would turn it on. A test guards this predicate.
pub(crate) fn arme_micro(value: Option<&str>) -> bool {
    value != Some("0")
}

/// Does `MICRO_MESURE` arm the sink? **`1`, and nothing else.**
///
/// ⚠️ **This predicate exists to be TESTED**, and the test exists to prevent
/// a future "simplification" into `is_ok()`. The convention is the reverse of
/// that of `AUDIO`/`SUPERVISEUR`/`PLEIN_ECRAN`/`CAPTEUR`, which disarm on
/// `=0`: here we arm on `=1`, because a bench instrument must not
/// switch on through the mere presence of a variable — someone writing
/// `MICRO_MESURE=0` to be sure to turn it off would turn it on.
pub(crate) fn arme(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Installs on `session` the sink [`choisir_puits`] designates — the cable,
/// the bench instrument, or none.
///
/// ⚠️ **This line said "the measurement sink, if `MICRO_MESURE=1`", and
/// concluded "as long as the real cable (block E2) does not exist".** The cable
/// has existed since task 9 of this same block: the sentence had become false
/// in the branch that shipped it, which is the exact class of defect
/// the cross-cutting end-of-branch review looks for. Fixed here rather than
/// left to be found.
///
/// **None of these three outcomes compromises the session**: when no sink
/// is set, `micro_disponible()` stays false, `ready` carries `mic: false`, and
/// the browser's button does not appear.
pub(super) fn brancher(config: &Config, session: &mut Session) {
    match choisir_puits(config.micro, config.micro_mesure) {
        Puits::Aucun => {
            // ⚠️ EMITTED AT WIRING, like that of the measurement sink: its
            // absence is what proves `MICRO` did not reach the
            // process.
            tracing::info!(
                session = %config.session_id,
                "micro DESARME (MICRO=0)"
            );
        }
        Puits::Mesure => {
            tracing::warn!(
                session = %config.session_id,
                "micro : l'instrument de banc PREND LE PAS sur le cable (MICRO_MESURE=1). Deux \
                 puits ne peuvent pas consommer le meme flux montant — chacun mangerait ce que \
                 l'autre attend"
            );
            brancher_mesure(config, session);
        }
        Puits::Cable => brancher_cable(config, session),
    }
}

/// The NOMINAL sink: writing to the virtual cable.
///
/// Its failure never compromises the session — `micro_disponible()` stays false,
/// `ready` carries `mic: false`, and the browser's button does not appear. It is
/// the behaviour from before block E2, and it remains reachable for all the
/// reasons `windows_micro::ouvrir` can name: cable not found or
/// ambiguous, format refused, local loop.
#[cfg(windows)]
fn brancher_cable(config: &Config, session: &mut Session) {
    match crate::windows_micro::ouvrir(config) {
        Ok(puits) => session.set_puits_micro(Box::new(puits)),
        Err(e) => tracing::warn!(
            session = %config.session_id,
            error = %e,
            "micro indisponible, la session continue sans"
        ),
    }
}

/// On the Linux host there is no cable, and nothing to say: this mode
/// only exists so that the crate compiles and the pure rules can be tested.
#[cfg(not(windows))]
fn brancher_cable(_config: &Config, _session: &mut Session) {}

/// L'instrument de banc.
fn brancher_mesure(config: &Config, session: &mut Session) {
    let lecteur = match LecteurMicro::new() {
        Ok(l) => Arc::new(Mutex::new(l)),
        Err(e) => {
            tracing::warn!(error = %e, "puits de mesure du micro indisponible, la session continue sans");
            return;
        }
    };

    session.set_puits_micro(Box::new(PuitsDeMesure {
        lecteur: Arc::clone(&lecteur),
    }));

    // ⚠️ EMITTED AT WIRING, NOT AT THE FIRST PACKET, and it is deliberate. D6
    // wrote a "did the variable arrive?" check that returned empty on all
    // seven runs because it ran before the initialisation it
    // observed: it would have masked a really missing variable. Here, the
    // line comes out as soon as the sink is set — hence before any WebRTC session —
    // and its ABSENCE proves `MICRO_MESURE` did not reach the process.
    tracing::info!(
        session = %config.session_id,
        "micro de mesure ARME (MICRO_MESURE=1) : instrument de banc, jamais une configuration livree"
    );

    let session_id = config.session_id.clone();
    std::thread::spawn(move || consommer(lecteur, session_id));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 **THE test of Decision 10, and the only one in this file that protects
    /// against a silent failure.** Two sinks cannot consume the
    /// same `LecteurMicro`: each drains what the other waits for, and the
    /// symptom would be a mic that hiccups without any line saying so.
    /// `MICRO_MESURE=1` wins — we arm a bench instrument to observe
    /// *instead of* the cable, never in addition.
    #[test]
    fn le_puits_de_mesure_prend_le_pas_sur_le_cable() {
        assert_eq!(choisir_puits(true, true), Puits::Mesure);
    }

    #[test]
    fn sans_instrument_de_banc_le_puits_est_le_cable() {
        assert_eq!(choisir_puits(true, false), Puits::Cable);
    }

    /// `MICRO=0` disarms everything, including the bench instrument: the variable
    /// says "no mic", not "no cable".
    #[test]
    fn micro_desarme_ne_pose_aucun_puits() {
        assert_eq!(choisir_puits(false, false), Puits::Aucun);
        assert_eq!(choisir_puits(false, true), Puits::Aucun);
    }

    /// ⚠️ **`MICRO` DISARMS on `=0`; mere presence does not arm.**
    /// Convention of `AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR` and `CAPTEUR`: we
    /// disarm on `=0` what is SHIPPED. This test exists to prevent a future
    /// "simplification" into `is_ok()`, which would switch the mic on for
    /// someone writing `MICRO=0` to be sure to turn it off.
    #[test]
    fn only_the_value_zero_disarms_the_mic() {
        assert!(!arme_micro(Some("0")));
        assert!(arme_micro(None));
        assert!(arme_micro(Some("")));
        assert!(arme_micro(Some("1")));
        assert!(arme_micro(Some("nimporte quoi")));
    }
}
