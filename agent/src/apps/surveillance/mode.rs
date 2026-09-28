//! The four states of `APPS_SURVEILLANCE`, and **none is a presence**.
//!
//! **PURE**: no `cfg`, no environment read, no input-output.
//! Reading `std::env` lives with the caller (`apps::brancher`); what is
//! DECIDED lives here, and it is the only part of the guard that can be seen turning red
//! on the host — `apps::desarme` already carries exactly this argument.
//!
//! 🔴 A SINGLE VARIABLE FOR FOUR STATES, AND NOT THREE BOOLEANS. Two reasons,
//! and the second is structural:
//!
//! 1. the `scripts/run-agent.sh` trap has been paid FIVE times by this repository
//!    (`SUPERVISEUR` in D1, `MULTIFENETRE_REPRISE` in D2, `AUDIO` in D7…): one
//!    line to pass through instead of three is a third of the risk;
//! 2. **the four states are mutually exclusive by construction.** Three
//!    booleans would allow "neither watch nor periodic reconciliation",
//!    that is an agent that NEVER reconciles — a state that makes no
//!    sense and that no acceptance run wants.

/// What `APPS_SURVEILLANCE` controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Variable absent: **the shipped product.** Watch armed, debounce
    /// armed, periodic reconciliation armed.
    Armee,
    /// `0`: watch disarmed — **exactly G1's behaviour.**
    /// It is the RED of criterion ①. ✅ **PLAYED, and it is RED**:
    /// **29,997 ms and 29,966 ms** against **960 ms and 999 ms** on the shipped arm, as
    /// `declencheur="periode"` against `"notification"`, two runs per arm.
    /// ⚠️ And **no `racine surveillée` line appears** in this state: the
    /// thread does not start. **It is that count that discriminates, never the mode's
    /// trace.**
    Desarmee,
    /// `sans-rebond`: watch armed, **debounce neutralised**. Every
    /// notification breaks the wait. It is the RED of criterion ④. ✅ **PLAYED, and
    /// it is RED**: **60 and 58** reconciliations against **4 and 4**, on the
    /// same burst of 20,000 files and in a window bounded by two
    /// timestamps.
    ///
    /// ⚠️ **IT DOES NOT MEASURE "DEBOUNCE AGAINST NOTHING"**, and that must be
    /// said next to the figure: the polling in `apps::boucle` has a granularity
    /// of 200 ms, which is **already** a weak debounce. The factor of 15 therefore pits
    /// `750 ms/4 s` against **200 ms**, never against zero.
    SansRebond,
    /// `seule`: watch armed, **periodic reconciliation DISARMED**.
    /// It was the RED of criterion ③ **as the specification writes it**.
    ///
    /// 🔴 **PLAYED, AND IT IS GREEN — two runs per arm, `cles=157` on
    /// BOTH sides.** It is not a surprise: it was written BEFORE playing it
    /// (divergence E4), and for three reasons readable in the code — a
    /// reconciliation re-reads the **whole** disk whatever its trigger,
    /// an overflow is **itself** a completion hence a trigger, and the
    /// first round is `complet` by construction. **What buys the absence of
    /// loss is therefore not the periodic reconciliation: it is that every
    /// reconciliation re-reads everything.**
    ///
    /// 🔵 **THIS MODE NONETHELESS REMAINS THE INDISPENSABLE HALF OF THE ONLY SET-UP THAT
    /// IS DISCRIMINATING**: `seule` **plus** `APPS_FAUTE=muette`. Measured,
    /// two runs per arm — the armed period catches up (`cles=157`,
    /// `declencheur="periode"`), `seule` **never** catches up (`cles=156`,
    /// for ninety seconds). **It is the measurement that justifies the
    /// specification's decision D1, and it did not exist before G4.**
    ///
    /// ⚠️ **BENCH variable, never a shipped configuration**: an agent that
    /// reconciles only on notification loses everything a missed notification
    /// carries away — that is, exactly the failure the
    /// periodic reconciliation exists to close.
    Seule,
}

impl Mode {
    /// Returns the mode **and**, where relevant, the unknown value to log.
    ///
    /// 🔴 EACH STATE IS AN EXACT EQUALITY, NEVER AN `is_some()`. Testing
    /// presence would ENABLE the mechanism when writing `APPS_SURVEILLANCE=0` TO
    /// TURN IT OFF, and would turn it off when writing `=1` to enable it. The two
    /// errors compensate to the point that nobody would see them without the test
    /// that pins them. It is the convention of `PLEIN_ECRAN`, `AUDIO`, `APPS`,
    /// `ICONES`, `PART_SONDAGE` and `PRESSE_PAPIER`, and `crate::apps::desarme`
    /// is **REUSED, not copied** — G2 set that precedent for `ICONES`,
    /// and the reuse is what preserves the `"0 "` case for free.
    ///
    /// 🔴 AN UNKNOWN VALUE KEEPS THE SHIPPED BEHAVIOUR, AND NAMES IT. Without
    /// it, a typo in a red — `seul` for `seule` — would run
    /// the GREEN behaviour under the RED's name, and the acceptance run would read a
    /// wrong verdict. It is the "a check that cannot fail" pattern
    /// in a new form: the set-up would yield `156` where it believes it reads
    /// `156`, for a reason that is not the one it measures.
    pub fn lire(value: Option<&str>) -> (Mode, Option<String>) {
        if crate::apps::desarme(value) {
            return (Mode::Desarmee, None);
        }
        match value {
            None => (Mode::Armee, None),
            Some("sans-rebond") => (Mode::SansRebond, None),
            Some("seule") => (Mode::Seule, None),
            Some(autre) => (Mode::Armee, Some(autre.to_string())),
        }
    }

    /// Should the watch thread start at all?
    pub fn surveille(&self) -> bool {
        !matches!(self, Mode::Desarmee)
    }

    /// Should the debounce defer the reconciliation?
    pub fn rebond(&self) -> bool {
        !matches!(self, Mode::SansRebond)
    }

    /// Should the periodic reconciliation run?
    ///
    /// ⚠️ **`Desarmee` returns `true`**, and it is not an oversight: `0` disarms the
    /// WATCH, it does not cut off the source of truth. That is precisely what
    /// makes `APPS_SURVEILLANCE=0` identical to G1's behaviour,
    /// hence a better RED for ① than the one the specification proposes —
    /// same binary, same corpus, same machine, a single variable of
    /// difference.
    pub fn periodique(&self) -> bool {
        !matches!(self, Mode::Seule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_quatre_etats_sont_des_egalites_exactes() {
        assert_eq!(Mode::lire(None), (Mode::Armee, None));
        assert_eq!(Mode::lire(Some("0")), (Mode::Desarmee, None));
        assert_eq!(Mode::lire(Some("sans-rebond")), (Mode::SansRebond, None));
        assert_eq!(Mode::lire(Some("seule")), (Mode::Seule, None));
    }

    /// 🔴 THE CONVENTION'S RED: replacing `crate::apps::desarme(value)`
    /// with `value.is_some()` makes this test fail on its FIRST assertion —
    /// `"sans-rebond"` would become `Desarmee`, that is, asking for the
    /// RED of criterion ④ would yield that of criterion ①, and the acceptance run would read a
    /// wrong verdict without any line saying so.
    #[test]
    fn une_presence_n_est_pas_un_desarmement() {
        assert_eq!(Mode::lire(Some("sans-rebond")).0, Mode::SansRebond);
        assert_eq!(Mode::lire(Some("seule")).0, Mode::Seule);
        assert_eq!(Mode::lire(Some("1")).0, Mode::Armee);
    }

    /// ⚠️ `"0 "` — one space too many in a generated `.ps1` — DOES NOT DISARM.
    /// The test in `apps.rs` already pins it for `APPS`; the reuse of `desarme`
    /// must preserve it, and that is what this assertion checks.
    #[test]
    fn an_unknown_value_keeps_the_shipped_behaviour_and_names_itself() {
        for value in ["seul", "SEULE", "", "00", "0 ", "sans_rebond", "false"] {
            let (mode, inconnue) = Mode::lire(Some(value));
            assert_eq!(mode, Mode::Armee, "{value:?} doit retenir le mode livré");
            assert_eq!(
                inconnue.as_deref(),
                Some(value),
                "{value:?} doit être NOMMÉE au journal"
            );
        }
    }

    /// The three predicates, one per consumer — and `Desarmee` lets the
    /// periodic reconciliation run, which is what makes `=0` equal to G1.
    #[test]
    fn each_predicate_only_cuts_what_concerns_it() {
        assert!(Mode::Armee.surveille() && Mode::Armee.rebond() && Mode::Armee.periodique());
        assert!(!Mode::Desarmee.surveille());
        assert!(
            Mode::Desarmee.periodique(),
            "`0` ne coupe PAS la source de vérité"
        );
        assert!(Mode::SansRebond.surveille() && !Mode::SansRebond.rebond());
        assert!(Mode::SansRebond.periodique());
        assert!(Mode::Seule.surveille() && Mode::Seule.rebond());
        assert!(!Mode::Seule.periodique());
    }
}
