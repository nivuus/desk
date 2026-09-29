//! What the platform asks of the applications loop.
//!
//! 🔴 EXTRACTED FROM `agent/src/plateforme.rs` VERBATIM (sub-block G2), BECAUSE
//! THE 500-LINE CEILING WAS CROSSED — 504 — AND BECAUSE THE REPOSITORY'S DOCTRINE
//! IS TO CATCH UP THROUGH AN EXTRACTION, NEVER THROUGH A COMPRESSION.
//!
//! ⚠️ **THE EXTRACTION SHOULD HAVE PRECEDED THE ADDITION.** G2's plan announced
//! this file at 453 lines for "+1 downstream branch"; the branch and its
//! documentation brought it to 504. **The crossing is DECLARED**, as
//! this repository requires of its three crossings in D10 and its two in D9.
//!
//! ⚠️ This is NOT the "Child module convention" of `docs/claude/module-conventions.md`, which targets
//! modules extracted from a `#[cfg(windows)]` parent: this parent has no
//! `cfg`, and it is the same mechanism used for the other reason — the
//! 500-line rule.

/// What the platform asks of the applications loop.
///
/// ❌ **"THE TWO DOWNSTREAM MESSAGES" BECAME FALSE IN SUB-BLOCK G3: THERE
/// ARE THREE.** `DepuisLaPlateforme::Installer` is added, and it does NOT go
/// to the same consumer — hence a SECOND queue, `Canal::installations()`.
///
/// ⚠️ **THE ARGUMENT BELOW IS NOT REFUTED FOR ALL THAT, and that is what
/// deserves to be read.** The risk it names — "a future addition forgets one"
/// — is real, and it holds for a single consumer. There is no longer one:
/// discovery drains `Ordre` on its COM thread, installation drains its queue on
/// `tokio`. Putting `Installer` here would make the COM THREAD DOWNLOAD SEVERAL HUNDRED
/// MEGABYTES, and it would stop reconciling — that is, the
/// catalogue would freeze during exactly the installation it is expected
/// to report on. The risk of forgetting is handled otherwise: the queue opposite
/// has **only one message type**, hence nothing to forget, and `Ordre` stays
/// exhaustive for ITS consumer — exactly as these lines ask.
/// See `agent/src/plateforme/installation.rs`.
///
/// 🔴 AN ENUM RATHER THAN A SECOND QUEUE, AND IT IS A DECISION. The two
/// downstream messages go to the SAME consumer — the apps loop, on its
/// dedicated COM thread —, and two queues would force it to query both at
/// each wait round, with the risk that a future addition forgets one. An
/// enum makes exhaustiveness checkable by the compiler where two queues would
/// leave it to vigilance.
///
/// ⚠️ IT CARRIES NO IMAGE BYTE. `IconesManquantes` only carries an
/// inventory of fingerprints; the images go up through `PUT /icone/:sha256`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ordre {
    /// Launch an application, by its key. `demande` pairs the response.
    Lancer { demande: String, cle: String },
    /// Upload the icons the platform does not have.
    IconesManquantes { empreintes: Vec<String> },
}
