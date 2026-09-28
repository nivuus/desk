//! What the virtual display driver keeps between two calls.
//!
//! Extracted from `pilote.rs` (batch 32, task 1) to fit under the project's
//! 500-line ceiling — **before** the addition that would have made it cross, and
//! not after. Same gesture and same reason as the neighbouring module `controle.rs`,
//! and as `superviseur/boucle/creation_sortie.rs` in D10: this repository has crossed
//! this ceiling twelve times, caught up by compression only twice,
//! and `CLAUDE.md` now forbids the second form by name.
//!
//! **No decision here, no behaviour**: the structure and its
//! documentation are moved as is. The only change is one of
//! visibility — the fields become `pub(super)`, otherwise the parent
//! could no longer read them. It is the trap named by `CLAUDE.md` ("an
//! extraction is never strictly verbatim […] moves
//! visibilities"), handled here rather than discovered by the compiler.

use windows::core::GUID;

use crate::moniteurs_virtuels::numeros::Numeros;
use crate::moniteurs_virtuels::{Adaptateur, IdSortie};

/// Everything the driver must keep between two calls, under a single
/// lock — the number allocator and the two lists only serve one
/// invariant ("every created output has a known GUID as long as it is not
/// removed"), and two locks for one invariant would be a gratuitous trap.
///
/// **Two lists on the other hand, and not one**, because they are two roles and
/// two lifetimes.
///
/// An entry of `apparies` serves to **translate an identifier into a GUID** — and,
/// since batch 32, to **find the adapter** on which the output was
/// created. An entry of `a_purger` serves to **remember a due removal**. Confusing the
/// two means an output whose removal failed stays indexed by an
/// identifier the driver may reassign: a later `detruire`
/// would then pair the stale entry, send the wrong GUID, and the live
/// output would never be destroyed. An identifier that can no longer be honoured
/// must therefore leave `apparies`, without the GUID being lost for all that.
#[derive(Default)]
pub(super) struct EtatSorties {
    /// Live outputs whose identifier is RELIABLE — that is returned by
    /// an output buffer of the right size. The trait returns an `IdSortie`
    /// (`u32`) whereas the driver removes through a GUID: it is here that the
    /// translation is made, and nothing else is allowed in it.
    ///
    /// 🔴 **TWO ROLES SINCE BATCH 32, AND THE SECOND DESTROYS NOTHING.**
    /// The `Adaptateur` accompanies the GUID so that `adaptateur_de` can
    /// return the `(adapter, target identifier)` pair that DESIGNATES the
    /// output to the display system. ⚠️ **It ONLY serves to DESIGNATE,
    /// never to destroy**: the driver only removes through a GUID, and nothing
    /// else — it is the invariant all the documentation above
    /// protects, and the addition of a third member does not dent it.
    pub(super) apparies: Vec<(IdSortie, GUID, Adaptateur)>,
    /// GUIDs of outputs whose creation succeeded and whose removal is DUE,
    /// without any reliable identifier making it possible to request them again. Never
    /// consulted by `detruire`: it only serves not to lose track of
    /// what must be purged — see task 7.
    pub(super) a_purger: Vec<GUID>,
    /// Allocator of GUID numbers, with recycling on successful
    /// destruction — see `numeros` for the defect this recycling fixes (I1).
    pub(super) numeros: Numeros,
}
