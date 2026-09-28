//! The wheel-round thread: the re-arbitration cadence, and polling the
//! clipboard OUTSIDE the global lock.
//!
//! **Extracted from `registre.rs` in fix round 1 (25 August 2026),
//! because its fixes had taken the file to 508 lines for a
//! project cap of 500.** Extract, never compress — same reason and same
//! set-up as `parts.rs`, `porteurs.rs` and `presse_papier.rs`, the three
//! neighbours already extracted from `sommeil.rs` for that reason.
//!
//! **What guided the CUT, and not arithmetic alone**: `registre.rs`
//! keeps *the registry itself* — the state, its access point, and the four
//! operations touching it (`distribuer`, `oublier`, `inscrire`,
//! `retirer`). This file is *the THREAD that calls them on a cadence*, which
//! is not the same responsibility: it carries a clock and Win32 I/O,
//! the registry has none.
//!
//! **Transposition, not rewrite**: the block is moved identically,
//! no value, no order of operations, no signature changed — only
//! the visibility of `start_the_round` moves to `pub(super)` to
//! stay reachable from `registre.rs`, which calls it.

use std::time::Instant;

use super::{
    distribuer, etat, parts, porteurs, presse_papier, purger_les_inaptitudes, PERIODE_REARBITRAGE,
};

/// **A single thread for the whole process**, started at the first registration.
///
/// **Safety does not rest on the `sleep` below.** This thread is launched FROM the
/// initialisation closure of `ETAT.get_or_init`; it is
/// `OnceLock::get_or_init` itself that guarantees that a second thread calling
/// `etat()` while this closure is still running **blocks** until
/// it finishes — the reentrancy that would panic would be that of the *same*
/// thread, which does not happen here. The `sleep` is only a cadence, not a guard.
pub(super) fn start_the_round() {
    std::thread::spawn(|| {
        let mut sondeur = crate::presse_papier::Sondeur::new();
        loop {
            std::thread::sleep(PERIODE_REARBITRAGE);

            // 🔴 **OUTSIDE THE LOCK, AND THAT IS THE WHOLE POINT OF THIS LINE.**
            // `sondeur.tour()` does Win32 I/O — `GetClipboardSequenceNumber`,
            // then `OpenClipboard`/`GetClipboardData` when the counter has moved.
            // `OpenClipboard` is a CONTENDED resource of the window
            // station: it fails, or waits, as soon as another application
            // holds it. Placed under `etat()` — the registry's GLOBAL lock, a
            // single `Mutex<Etat>` for the whole process —, it would block
            // all that time `inscrire`, `retirer`, `signaler` and
            // `echec_de_reveil`, that is the attach and removal of ALL
            // windows, and the return of a refused wake-up.
            //
            // The specification places polling "on the wheel round" without
            // saying on which side of the lock; it is the plan (D-P1-3, divergence
            // E3) that decided, and it is a defect fixed before it existed.
            //
            // Only the RESULT — an `Annonce` already normalised, bounded and
            // deduplicated — enters under the lock, below.
            //
            // The `presse_papier::actif()` arming guard lives inside
            // `tour()`, BEFORE any read: `PRESSE_PAPIER=0` therefore prevents
            // even reading the counter, not only the sending. Duplicating
            // it here would duplicate a decision already taken at the right place.
            //
            // ⚠️ **The REVERSE direction tests it a second time, and it is NOT
            // the duplicate the sentence above forbids**: `actif()` there guards
            // another decision point — the WRITE, served from a window
            // thread (`sommeil::presse_papier::write_with`). Without it,
            // `PRESSE_PAPIER=0` would cut reading and leave writing,
            // and "the whole mechanism is disarmed" would be a half-truth.
            // 🔴 **BEFORE `tour()`, and the order IS the mechanism** (sub-block
            // P2). Consumes the write the WINDOW thread put into
            // `Etat` while serving a paste, and arms D5's guards no. 1 and
            // no. 2 on it. Placed after `tour()`, it would arrive too late: the
            // round would already have re-read our own text and sent it back to the
            // windows.
            presse_papier::armer_les_gardes(&mut sondeur);

            let annonce = sondeur.tour();

            // 🔴 **THE SECOND TAKE (D-P3-6), AFTER `tour()` AND BEFORE
            // `distribuer`.** `armer_les_gardes` above consumed
            // the write that EXISTED before the round; this one consumes the one
            // that ARRIVED DURING it. Without it, a second paste happening
            // between arming and reading passes BOTH of D5's guards — no. 1
            // because the counter moved again, no. 2 because the memorised text
            // is that of the PREVIOUS paste — and its own text
            // goes back out to the N windows.
            //
            // The race was MEASURED before being closed, by a red test
            // on the intact tree and without any mutation; its demonstration and
            // the remaining residue live next to
            // `Sondeur::ecarter_notre_ecriture`.
            let annonce = presse_papier::filtrer_nos_ecritures_tardives(&mut sondeur, annonce);

            let mut garde = etat();
            let maintenant = Instant::now();
            let ordres = garde.vivier.rearbitrer(maintenant);
            distribuer(&mut garde, ordres);
            parts::distribuer_les_parts(&mut garde);
            purger_les_inaptitudes(&mut garde.inaptes, Instant::now());
            porteurs::distribuer_l_audio(&mut garde);
            if let Some(annonce) = annonce {
                presse_papier::distribuer(&mut garde, annonce);
            }
        }
    });
}
