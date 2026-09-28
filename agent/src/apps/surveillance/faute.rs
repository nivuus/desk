//! Watch fault injection — bench variable `APPS_FAUTE`,
//! **never a shipped configuration**.
//!
//! It makes reachable three code paths the machine does not produce
//! on its own: buffer overflow, the **swallowed** completion, and the
//! loss of a handle. **Verbatim** pattern of
//! `agent/src/transport/piste_audio/injection.rs`.
//!
//! 🔴 **WHAT INJECTION ESTABLISHES, AND WHAT IT DOES NOT.** It establishes
//! that the **REMEDY** works, never that a **CAUSE** exists. It is the sentence
//! sub-block D11 wrote about `AUDIO_FAUTE_RECONSTRUCTION`, and it holds
//! here word for word: a `notifications perdues` line produced by injection
//! **says nothing** about the probability that a real overflow happens on
//! this machine. Only a real burst says so, and its verdict may be NOT
//! MEASURABLE.
//!
//! ⚠️ **Convention `absent = disarmed`** — that of `AUDIO_FAUTE_LECTURE`,
//! `AUDIO_FAUTE_RECONSTRUCTION` and `INSTALLATION_FAUTE`. **Never** that
//! of `PLEIN_ECRAN`: here absence is not the disarming of a shipped
//! mechanism, it is the nominal state of an instrument that exists only for a bench.

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::OnceLock;

/// What the next completion is made to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Famille {
    /// The next *n* completions are treated as **overflows**:
    /// counted, logged, **and triggering**. Makes the path of
    /// criterion ② reachable when the real burst does not overflow.
    Debordement,
    /// 🔵 The next *n* completions are **SWALLOWED**: neither counted, nor
    /// logged, nor triggering.
    ///
    /// 🔴 IT IS THE ONLY SET-UP THAT MAKES CRITERION ③ DISCRIMINATING, and the
    /// reason fits in one sentence: in this design, **an overflow is
    /// itself a completion**, hence a trigger, and every reconciliation
    /// re-reads the whole disk — an overflow therefore fixes itself. The
    /// only failure the periodic reconciliation really buys is
    /// **a watch that stops delivering WITHOUT AN ERROR**, and that is
    /// exactly what this family fabricates.
    Muette,
    /// The next *n* completions return a fatal handle error: the
    /// root goes into failure, and its recovery becomes observable.
    Perte,
}

impl Famille {
    /// The code stored in the atomic. **`0` is reserved for "none".**
    fn code(self) -> u8 {
        match self {
            Famille::Debordement => 1,
            Famille::Muette => 2,
            Famille::Perte => 3,
        }
    }

    fn depuis_code(code: u8) -> Option<Famille> {
        match code {
            1 => Some(Famille::Debordement),
            2 => Some(Famille::Muette),
            3 => Some(Famille::Perte),
            _ => None,
        }
    }
}

/// Parses `APPS_FAUTE`. **PURE** — it reads neither the environment nor the state.
///
/// ⚠️ `"debordement:0"` RETURNS `None`, AND IT IS DECIDED HERE RATHER THAN SUFFERED: a
/// budget of zero is an injection that will never fire, and declaring it
/// "ARMED" in the log would make someone read an arming where there is none. The
/// behaviour is therefore exactly that of absence.
///
/// ⚠️ AN UNKNOWN FAMILY COMPLAINS, it does not stay silent: without the `warn!`, a
/// typo (`debordment:3`) would silently disarm the injection and the acceptance run
/// would read a zero that means nothing.
pub fn lire(value: Option<&str>) -> Option<(Famille, u32)> {
    let value = value?;
    let (nom, compte) = value.split_once(':')?;
    let famille = match nom {
        "debordement" => Famille::Debordement,
        "muette" => Famille::Muette,
        "perte" => Famille::Perte,
        _ => {
            tracing::warn!(
                value,
                "APPS_FAUTE : famille inconnue, injection DESARMEE \
                 (attendu : debordement:<n>, muette:<n> ou perte:<n>)"
            );
            return None;
        }
    };
    let compte: u32 = compte.parse().ok()?;
    if compte == 0 {
        return None;
    }
    Some((famille, compte))
}

/// The injection state, **PROCESS-GLOBAL**.
///
/// 🔴 **GLOBAL, AND THE REASON IS MEASURED ELSEWHERE.** Sub-block D10 paid for a
/// budget re-read **per thread** on `AUDIO_FAUTE_LECTURE`: every rebuilt
/// capture received a fresh budget, and **the judging figure was
/// structurally unable to leave zero, on a product that was nonetheless
/// fixed**. Here the watch **reopens** after a loss: a budget
/// re-read at reopening would re-arm identically, and the same measurement
/// failure would replay.
///
/// Two atomics rather than an `Option<Famille>`: that is what lets the
/// tests **seed the state directly** rather than the environment — an
/// already initialised `OnceLock` would not re-read `std::env` anyway, and
/// that is the note `piste_audio/injection.rs` carries.
fn etat() -> &'static (AtomicU8, AtomicU32) {
    static ETAT: OnceLock<(AtomicU8, AtomicU32)> = OnceLock::new();
    ETAT.get_or_init(|| {
        let brut = std::env::var("APPS_FAUTE").ok();
        match lire(brut.as_deref()) {
            Some((famille, compte)) => {
                tracing::warn!(
                    ?famille,
                    fautes_a_injecter = compte,
                    "faute de surveillance ARMEE (APPS_FAUTE) : banc, jamais une configuration livrée"
                );
                (AtomicU8::new(famille.code()), AtomicU32::new(compte))
            }
            None => (AtomicU8::new(0), AtomicU32::new(0)),
        }
    })
}

/// Consumes a fault of this family, or returns `false`.
///
/// `fetch_update` with `checked_sub(1)` decrements atomically IF the global
/// budget is not already at zero, and returns `Err` without touching it otherwise: an exhausted
/// budget — the nominal case, the variable being absent — therefore lets
/// the real call through from the first round, at no measurable cost.
pub fn consommer(famille: Famille) -> bool {
    let (armee, reste) = etat();
    if Famille::depuis_code(armee.load(Ordering::Relaxed)) != Some(famille) {
        return false;
    }
    reste
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lire_reconnait_les_trois_familles_et_leur_compte() {
        assert_eq!(lire(Some("debordement:3")), Some((Famille::Debordement, 3)));
        assert_eq!(lire(Some("muette:1")), Some((Famille::Muette, 1)));
        assert_eq!(lire(Some("perte:2")), Some((Famille::Perte, 2)));
    }

    #[test]
    fn lire_desarme_sur_tout_ce_qui_n_est_pas_une_famille_suivie_d_un_compte() {
        assert_eq!(lire(None), None, "absente = DÉSARMÉE");
        assert_eq!(
            lire(Some("debordement")),
            None,
            "sans compte : rien à tirer"
        );
        assert_eq!(lire(Some("debordement:")), None);
        assert_eq!(lire(Some("debordement:x")), None);
        // ⚠️ DECIDED: a budget of zero equals absence, and does not declare itself
        // "ARMED". An arming that will never fire would be a false arming
        // in the log.
        assert_eq!(lire(Some("debordement:0")), None);
        // Unknown family: `None`, AND a `warn!` naming it.
        assert_eq!(lire(Some("debordment:3")), None);
        assert_eq!(lire(Some("muette")), None);
    }

    /// 🔴 THE TEST THAT PINS THE BUDGET AS PROCESS-GLOBAL.
    ///
    /// Its RED is a budget **re-read on every call**: the failure D10
    /// paid for on `AUDIO_FAUTE_LECTURE`. Under it, the state not being seeded
    /// from the environment, the very first `consommer` would already return
    /// `false` — and a judging figure built on it could never leave
    /// zero, on a product that is nonetheless correct.
    ///
    /// ⚠️ The state is SEEDED DIRECTLY, never through `std::env`: an already
    /// initialised `OnceLock` no longer re-reads the environment, and two tests relying
    /// on it would steal each other's budget.
    #[test]
    fn le_budget_est_global_au_processus_et_s_epuise_une_seule_fois() {
        let (armee, reste) = etat();
        armee.store(Famille::Perte.code(), Ordering::Relaxed);
        reste.store(1, Ordering::Relaxed);
        assert!(
            consommer(Famille::Perte),
            "le budget de 1 doit tirer une fois"
        );
        assert!(
            !consommer(Famille::Perte),
            "et une seule : un budget global s'épuise pour tout le processus"
        );
        armee.store(0, Ordering::Relaxed);
        reste.store(0, Ordering::Relaxed);
    }

    /// An armed family serves no other: asking for `Muette` when
    /// `Debordement` is armed consumes nothing, and does not lie.
    #[test]
    fn une_famille_ne_consomme_pas_le_budget_d_une_autre() {
        let (armee, reste) = etat();
        armee.store(Famille::Debordement.code(), Ordering::Relaxed);
        reste.store(5, Ordering::Relaxed);
        assert!(!consommer(Famille::Muette));
        assert!(!consommer(Famille::Perte));
        assert_eq!(reste.load(Ordering::Relaxed), 5, "le budget n'a pas bougé");
        assert!(consommer(Famille::Debordement));
        armee.store(0, Ordering::Relaxed);
        reste.store(0, Ordering::Relaxed);
    }

    #[test]
    fn un_etat_desarme_ne_consomme_rien() {
        let (armee, reste) = etat();
        armee.store(0, Ordering::Relaxed);
        reste.store(0, Ordering::Relaxed);
        for famille in [Famille::Debordement, Famille::Muette, Famille::Perte] {
            assert!(!consommer(famille), "{famille:?} sur un état désarmé");
        }
    }

    #[test]
    fn code_zero_is_reserved_for_no_family() {
        assert_eq!(Famille::depuis_code(0), None);
        for famille in [Famille::Debordement, Famille::Muette, Famille::Perte] {
            assert_ne!(famille.code(), 0);
            assert_eq!(Famille::depuis_code(famille.code()), Some(famille));
        }
    }
}
