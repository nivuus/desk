//! The effects `Table` returns, and that the supervisor executes.
//!
//! Extracted from `table.rs` (task 9 of sub-block D10, at review, August 6th, 2026):
//! the parent file was at 499 lines, margin 1, once the
//! `size` field of `LancerEnfant` was added — the plan's constraint ("extraction first,
//! without exception", not "extraction once 500 is crossed") applies from
//! that very margin, before even exceeding it. Purely declarative: no
//! logic here, only the enumeration and its documentation, already heavy —
//! same reason as `attribution.rs`, its neighbour in this same directory.

use super::{IdFenetre, IdSession};

/// What the table asks the outside world to do. The supervisor
/// executes them in the order returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effet {
    AnnoncerOuverture {
        session: IdSession,
        titre: String,
    },
    /// `titre` comes with the request because the refusal that may follow
    /// is shown to a human. Without it, the caller only has the session
    /// identifier at hand and the shell page announces "*"w-3" could not
    /// open*" — a message that designates nothing for the user.
    CreateOutput {
        session: IdSession,
        titre: String,
        largeur: u32,
        hauteur: u32,
    },
    LancerEnfant {
        session: IdSession,
        fenetre: IdFenetre,
        /// DXGI name of the output (`\\.\DISPLAYn`), **and not a pair
        /// of indices**: those are positional, the child resolves them at its
        /// startup — hence later — and an output appearing or disappearing
        /// meanwhile makes it capture something else, or fail.
        nom_sortie: String,
        /// The RETAINED size (`placement::retained_size`), not the output's:
        /// the output can be much larger (polluted registry,
        /// see `creation_sortie::create_output`). It is this size the
        /// supervisor puts on the child (`TAILLE_FENETRE`), so that it
        /// tells it again to the sensor at attach time (task 9 of sub-block D10) — the
        /// sensor needs it to crop (task 8).
        size: (u32, u32),
    },
    TuerEnfant {
        session: IdSession,
    },
    /// `sortie_pilote` is **the DRIVER's identifier**, not the DXGI name: the
    /// driver can only remove an output through what it itself returned at
    /// creation; presenting it a DXGI name would destroy nothing, or
    /// would destroy someone else's output. Both identifiers designate the same
    /// output and have no computable relation — hence the two fields.
    ///
    /// `nom_sortie` comes with the destruction because the entry has already
    /// left the table when this effect is returned: without it, the caller
    /// could no longer know which DXGI slot becomes free again.
    DetruireSortie {
        sortie_pilote: u32,
        nom_sortie: String,
    },
    /// The browser resized its window, and the session is ALREADY live:
    /// there is neither an output to create nor a child to launch, only a retained
    /// size to correct and a window to put back.
    ///
    /// 🔴 **WHY AN EFFECT, AND NOT A DECISION OF THE TABLE.** The new
    /// size is bounded by the output's WORK AREA, which only a fresh Win32
    /// read gives (`window::zones_du_moniteur_au_point`) — and the table
    /// is PURE, tested on the Linux host. Besides, it only retains the
    /// RETAINED size, never the output's: it therefore could not
    /// make a window GROW that a smaller viewport had shrunk. The
    /// table decides THAT IT MUST FOLLOW, the loop measures and applies.
    ///
    /// `largeur`/`hauteur` are already bounded by `clamp_to_max_size`,
    /// as on the two other entry paths of the viewport.
    SuivreLeViewport {
        session: IdSession,
        largeur: u32,
        hauteur: u32,
    },
    AnnoncerFermeture {
        session: IdSession,
    },
    AnnoncerRefus {
        titre: String,
        motif: String,
    },
}
