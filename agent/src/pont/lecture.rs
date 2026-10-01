//! **The read window**: how many chunks we request in advance, and
//! the ordering invariant that makes this advance safe. **PURE** — no `cfg`,
//! no I/O, no clock.
//!
//! # 🔴 WHY THIS MODULE EXISTS, AND WHY IT COULD NOT BE DELIVERED
//! ALONE
//!
//! Spec §7.3 states: "the bridge does not request chunk *n+1* as long as the
//! channel has more than `SEUIL_TAMPON` bytes pending". **Read in the code of
//! F1 and F2: there is only ONE chunk in flight at a time** — the next one is
//! requested only on receipt of the previous one, and three comments in the repository
//! announced the window as a deliverable of F3.
//!
//! **With a single chunk in flight, the spec's rule can NEVER bite:
//! the bridge is never ahead.** Delivering `SEUIL_TAMPON` without the window
//! would be delivering a mechanism unable to trigger — that is, a
//! check we will never see red, applied this time to a
//! PRODUCT mechanism. F3 therefore delivers **both, or neither**.
//!
//! ⚠️ **Back-pressure, for its part, is on the BROWSER SIDE**
//! (`client/src/files/flux.ts`), because it is the browser that emits the large messages and
//! `bufferedAmount` is a property of ITS channel. The bridge does not see it and
//! cannot see it. The two halves are inseparable: the window without
//! back-pressure would fill the SCTP queue, back-pressure without the
//! window would have nothing to hold back.
//!
//! # THE INVARIANT THAT MAKES THE WINDOW SAFE, AND IT IS CHECKED RATHER THAN BELIEVED
//!
//! The channel is `ordered` (`client/src/files/canal.ts`), chunks are
//! requested in increasing position order, so responses arrive
//! in that order, so `PrjWriteFileData` is called in that order.
//!
//! 🔴 **This module does NOT trust SCTP for all that.** It checks that the
//! received response is indeed the expected one, and **denounces** otherwise instead of
//! applying it. Writing a range at the wrong place would produce a file that
//! **only a SHA-256 digest would show to be wrong** — and the end-to-end
//! digest is precisely what F1 NEVER established (its legacy no. 6).
//!
//! ⚠️ **F3 CLAIMS NO THROUGHPUT GAIN.** The repository's only throughput
//! measurement is inconsistent by a factor of ~120 (6.5 MiB/s versus 52–55 KiB/s, F1
//! §11), without explanation.
//!
//! ✅ **F4 JUDGED, AND DID MORE THAN JUDGE: IT EXPLAINS.** The bridge's channel
//! sustains **~30 to 33 KiB/s**, and it is LINEAR — 4 KiB in 190 ms, 64 KiB in
//! 2,012 ms, 128 KiB in 4,044 ms, two runs each. The ~120 factor
//! dissolves into two measured facts: **a REREAD does not cross the bridge**
//! (0.6 ms, zero traversal — ProjFS hydration *is* the data cache),
//! and the ceiling comes from `pont::transport`'s loop, which reads **one
//! datagram per turn** while blocking up to `ATTENTE_MAX` before each
//! read. Mutated to 1 ms, it DOUBLES the throughput. F1's upper bound
//! (6.5 MiB/s) is therefore **unreachable through the bridge** on this setup.
//!
//! 🔴 **AND `MORCEAUX_EN_VOL = 4` IS NEVER REACHED ON THE SHIPPED BINARY.**
//! `en_vol_max` does rise to 2 on a two-chunk read — the window
//! opens —, but the only read that would have four (256 KiB) **FAILS**:
//! four concurrent chunks share 33 KiB/s, each then exceeds
//! `DELAI_LIRE`, and they expire. **The flow control F3 delivers has therefore
//! never had the chance to serve in operation**; it serves on the
//! diagnostic binary, at `ATTENTE_MAX = 1 ms`, where the 256 KiB rank completes with
//! `lire=n:4`. See `docs/…/2026-08-21-pont-fichiers-f4-resultats.md`. (policy: allow-fr, real file path)
//!
//! ✅ **THE CEILING IS LIFTED (October 1st, 2026)**: the loop no longer reads one
//! datagram per turn — it is woken by each datagram as it lands
//! (`pont/transport/reveil.rs`), and `transport/tests_debit.rs` moves 2 MiB
//! within a 10 s budget the old loop exceeded. And the browser now sends the
//! answers of one path in request order (`client/src/files/ordre.ts`), which
//! is what [`Fenetre::recu`] checks. ⚠️ **No throughput is claimed on the VM**:
//! it remains to be measured there, as F4 did.

use std::collections::VecDeque;

use crate::pont::decoupe::Morceau;

/// How many chunks at most are requested in advance.
///
/// ⚠️ **NOT CALIBRATED.** It joins `SEUIL_TAMPON`, `DELAI_MUTATION`,
/// `PERIODE_RECENSEMENT`, F1's four, F2's and the eight of
/// work item D in the list of constants no measurement has judged.
///
/// 🔴 **A VALUE OF 1 WOULD MAKE THE WINDOW INERT**, that is, would deliver a
/// flow control unable to bite. It is what
/// `the_window_really_reaches_MORCEAUX_EN_VOL_on_a_long_read`
/// denounces, and it is **the red of the deliverable itself**.
pub const MORCEAUX_EN_VOL: usize = 4;

/// A response that is not the one we expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HorsOrdre {
    /// The position whose response we received.
    pub recue: u64,
    /// The one we expected, if there was one.
    pub attendue: Option<u64>,
}

/// The chunks of a read: what remains to request, what is in flight.
#[derive(Debug)]
pub struct Fenetre {
    /// What has not been requested yet.
    restants: VecDeque<Morceau>,
    /// The positions requested and not yet received, **in emission
    /// order**.
    en_vol: VecDeque<u64>,
    /// The maximum of `en_vol.len()` reached. **Read at the census**: it is
    /// what says whether the window served any purpose.
    en_vol_max: usize,
}

impl Fenetre {
    pub fn new(restants: VecDeque<Morceau>) -> Self {
        Self {
            restants,
            en_vol: VecDeque::new(),
            en_vol_max: 0,
        }
    }

    /// The chunks to request NOW — **at most [`MORCEAUX_EN_VOL`] in
    /// flight in total**, never per call.
    ///
    /// ⚠️ **The bound applies to the TOTAL in flight, not to the returned batch.** Bounding
    /// the batch would let the queue grow without end: four per call, called
    /// four times, would make sixteen in flight.
    pub fn a_demander(&mut self) -> Vec<Morceau> {
        let mut lot = Vec::new();
        while self.en_vol.len() < MORCEAUX_EN_VOL {
            let Some(morceau) = self.restants.pop_front() else {
                break;
            };
            self.en_vol.push_back(morceau.position);
            lot.push(morceau);
        }
        self.en_vol_max = self.en_vol_max.max(self.en_vol.len());
        lot
    }

    /// A response arrived for `position`.
    ///
    /// 🔴 **IT MUST BE THE OLDEST IN FLIGHT**, and the rest is denounced.
    /// Applying an out-of-order response would write a range at the wrong rank of the
    /// file, and **only a SHA-256 digest would catch it**.
    ///
    /// ⚠️ **An UNKNOWN position is denounced too** — not only an in-flight
    /// position arriving too early. A late response to an already resolved
    /// correlation is thrown away upstream by `pont::table`; receiving one here
    /// would mean the two ends have diverged, and guessing would write
    /// anything into ProjFS's buffer.
    pub fn recu(&mut self, position: u64) -> Result<(), HorsOrdre> {
        let attendue = self.en_vol.front().copied();
        if attendue != Some(position) {
            return Err(HorsOrdre {
                recue: position,
                attendue,
            });
        }
        self.en_vol.pop_front();
        Ok(())
    }

    /// The maximum of chunks in flight reached since the start of the read.
    pub fn en_vol_max(&self) -> usize {
        self.en_vol_max
    }

    /// How many are in flight at this instant.
    #[cfg(test)]
    pub fn en_vol(&self) -> usize {
        self.en_vol.len()
    }

    /// Nothing left to request, nothing in flight.
    pub fn terminee(&self) -> bool {
        self.restants.is_empty() && self.en_vol.is_empty()
    }
}

#[cfg(test)]
mod tests;
