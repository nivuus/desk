//! Making the cause chain of an `anyhow::Error` READABLE in a trace.
//!
//! 🔴 **The defect this module closes is an INSTRUMENT defect, and it
//! cost a whole batch.** Batch 25 measured 74 then 52 `réveil refusé, la
//! fenêtre reste endormie` in a row without anyone being able to say
//! WHY: the trace site wrote `%erreur` on an `anyhow::Error`,
//! yet `anyhow`'s PLAIN `Display` renders **only the OUTERMOST
//! layer** — here the `with_context` set by `transitions.rs::reveiller`,
//! that is "réveil de la session …:w-24", which says nothing more than
//! "it failed". The `HRESULT` of the Windows layer, the only datum that
//! answers the question, stayed in the causes, thrown away at writing time.
//!
//! **Why `{:#}` and not `{:?}`** — the choice is written here so that nobody
//! reopens it:
//!
//! - `{}` (hence `%erreur` in `tracing`): the outer layer ALONE. That is the
//!   defect being fixed.
//! - `{:#}`: the COMPLETE chain, causes separated by `: `, **on a
//!   single line**. That is what we keep.
//! - `{:?}`: the complete chain **plus** a backtrace, on
//!   SEVERAL lines, and only if `RUST_BACKTRACE` is set — which
//!   `scripts/run-agent.sh` does not set. Ruled out for two reasons: the backtrace
//!   would be empty in practice, and multi-line would break the only instrument
//!   this repository has on `agent.log`, which is line counting through
//!   `grep -c` (one `réveil refusé` would count for three).
//!
//! The repository had already decided this way, twice, without ever setting the rule
//! in the same place: `transport/piste_audio.rs` (D10's hand-over 6, an `HRESULT`
//! lost the same way) and `diagnostics/multifenetre/plafond/sonde.rs`.
//! **This module is that place**; it lives at the bare root because its name
//! is understood without reference to a parent (`CLAUDE.md` convention,
//! precedents `geometry`, `sortie_dxgi`, `survie_verdict`).

/// Returns the complete cause chain of an error, on a single line.
///
/// To be used everywhere `%erreur` was logged on an `anyhow::Error`:
/// `tracing::warn!(erreur = %crate::cause::chaine(&erreur), "…")`.
///
/// Takes a reference and **does not consume** the error: several fixed
/// sites log it then propagate it.
pub fn chaine(erreur: &anyhow::Error) -> String {
    format!("{erreur:#}")
}

#[cfg(test)]
mod tests {
    use anyhow::{anyhow, Context};

    /// The check that counts: it must TURN RED if we go back to `{}`. The two
    /// assertions are therefore asymmetric on purpose — the first says what
    /// `{:#}` adds, the second says what `{}` LOSES, and without it the test
    /// would still pass with a `format!("{erreur}")` in `chaine`.
    #[test]
    fn la_chaine_porte_la_cause_profonde_que_le_display_simple_jette() {
        let profonde = anyhow!("0x88890004");
        let erreur = Err::<(), _>(profonde)
            .context("activation de l'encodeur H.264 matériel (ActivateObject)")
            .context("réveil de la session prefixe:w-24")
            .unwrap_err();

        let rendue = super::chaine(&erreur);
        assert!(
            rendue.contains("0x88890004"),
            "la cause profonde manque : {rendue}"
        );
        assert!(
            rendue.contains("ActivateObject"),
            "la couche intermédiaire manque : {rendue}"
        );
        assert!(
            rendue.contains("réveil de la session"),
            "la couche externe manque : {rendue}"
        );

        // The negative witness, in the SAME reading: the plain `Display`, the one
        // `%erreur` used, renders ONLY the outer layer.
        let simple = format!("{erreur}");
        assert_eq!(simple, "réveil de la session prefixe:w-24");
        assert!(
            !simple.contains("0x88890004"),
            "le témoin négatif est faux : {simple}"
        );
    }

    /// An error WITHOUT context must stay readable as is: the
    /// fix must not degrade the simple case, which is the most frequent.
    #[test]
    fn une_erreur_sans_contexte_est_rendue_telle_quelle() {
        assert_eq!(
            super::chaine(&anyhow!("aucune sortie DXGI nommée 0:1")),
            "aucune sortie DXGI nommée 0:1"
        );
    }
}
