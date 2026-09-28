//! Making the cause chain of an `anyhow::Error` READABLE in a trace.
//!
//! 🔴 **The defect this module closes is an INSTRUMENT defect, and it
//! cost a whole batch.** Batch 25 measured 74 then 52 `wake-up refused, the
//! window stays asleep` in a row without anyone being able to say
//! WHY: the trace site wrote `%error` on an `anyhow::Error`,
//! yet `anyhow`'s PLAIN `Display` renders **only the OUTERMOST
//! layer** — here the `with_context` set by `transitions.rs::reveiller`,
//! that is "waking session …:w-24", which says nothing more than
//! "it failed". The `HRESULT` of the Windows layer, the only datum that
//! answers the question, stayed in the causes, thrown away at writing time.
//!
//! **Why `{:#}` and not `{:?}`** — the choice is written here so that nobody
//! reopens it:
//!
//! - `{}` (hence `%error` in `tracing`): the outer layer ALONE. That is the
//!   defect being fixed.
//! - `{:#}`: the COMPLETE chain, causes separated by `: `, **on a
//!   single line**. That is what we keep.
//! - `{:?}`: the complete chain **plus** a backtrace, on
//!   SEVERAL lines, and only if `RUST_BACKTRACE` is set — which
//!   `scripts/run-agent.sh` does not set. Ruled out for two reasons: the backtrace
//!   would be empty in practice, and multi-line would break the only instrument
//!   this repository has on `agent.log`, which is line counting through
//!   `grep -c` (one `wake-up refused` would count for three).
//!
//! The repository had already decided this way, twice, without ever setting the rule
//! in the same place: `transport/piste_audio.rs` (D10's hand-over 6, an `HRESULT`
//! lost the same way) and `diagnostics/multifenetre/plafond/sonde.rs`.
//! **This module is that place**; it lives at the bare root because its name
//! is understood without reference to a parent (`CLAUDE.md` convention,
//! precedents `geometry`, `sortie_dxgi`, `survie_verdict`).

/// Returns the complete cause chain of an error, on a single line.
///
/// To be used everywhere `%error` was logged on an `anyhow::Error`:
/// `tracing::warn!(error = %crate::cause::chain(&error), "…")`.
///
/// Takes a reference and **does not consume** the error: several fixed
/// sites log it then propagate it.
pub fn chain(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

#[cfg(test)]
mod tests {
    use anyhow::{anyhow, Context};

    /// The check that counts: it must TURN RED if we go back to `{}`. The two
    /// assertions are therefore asymmetric on purpose — the first says what
    /// `{:#}` adds, the second says what `{}` LOSES, and without it the test
    /// would still pass with a `format!("{error}")` in `chain`.
    #[test]
    fn the_chain_carries_the_root_cause_that_plain_display_drops() {
        let profonde = anyhow!("0x88890004");
        let error = Err::<(), _>(profonde)
            .context("activating the hardware H.264 encoder (ActivateObject)")
            .context("waking session prefixe:w-24")
            .unwrap_err();

        let rendue = super::chain(&error);
        assert!(
            rendue.contains("0x88890004"),
            "the root cause is missing: {rendue}"
        );
        assert!(
            rendue.contains("ActivateObject"),
            "the middle layer is missing: {rendue}"
        );
        assert!(
            rendue.contains("waking session"),
            "the outer layer is missing: {rendue}"
        );

        // The negative witness, in the SAME reading: the plain `Display`, the one
        // `%error` used, renders ONLY the outer layer.
        let simple = format!("{error}");
        assert_eq!(simple, "waking session prefixe:w-24");
        assert!(
            !simple.contains("0x88890004"),
            "the negative control is wrong: {simple}"
        );
    }

    /// An error WITHOUT context must stay readable as is: the
    /// fix must not degrade the simple case, which is the most frequent.
    #[test]
    fn an_error_without_context_is_rendered_as_is() {
        assert_eq!(
            super::chain(&anyhow!("no DXGI output named 0:1")),
            "no DXGI output named 0:1"
        );
    }
}
