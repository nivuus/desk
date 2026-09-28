//! Tests of `attentes.rs` — next to the logic they exercise (review of
//! task 16, D10: the first extraction had left them in
//! `serveur/tests.rs`, separated from the registry they test).
//!
//! **These tests do NOT run on this host**: `serveur.rs`, which declares
//! `mod attentes;`, carries `#![cfg(windows)]` on the whole file, and
//! `capteur.rs` gates `pub mod serveur;` again with `#[cfg(windows)]` on top. On
//! the default target (Linux), `capteur::serveur` — hence
//! `capteur::serveur::attentes` — does not compile at all: `cargo test -p
//! agent capteur::serveur` returns `0 passed; ... filtered out`, never an
//! execution. Only `cargo check --target x86_64-pc-windows-gnu --tests`
//! checks this module on this host (types, borrows, arity — never
//! execution, for lack of a VM or emulator here).

use super::*;
use std::sync::mpsc::channel;

/// The F5 race (D7), on the SECOND registry. Sub-block D9 closed it on
/// the sleep registry only (`capteur/sommeil/registre.rs`) — the
/// brief of its task 10 only named `sommeil`, and no per-task review
/// could see this twin (see the comment on `oublier`).
///
/// ⚠️ **The generation is taken at ATTACH** (in `attendre_le_media`,
/// called from `ouvrir_les_commandes` in `serveur.rs`), **not at
/// process launch**: the first version of this hand-over, in D9, stamped
/// the generation at launch, and `retirer_est_perime` (its equivalent on
/// the other registry) could then structurally never return `true` in
/// production — the defect was in the design, not in the execution. This
/// test therefore exercises the real sequence: register, register again (re-attach under
/// the same name), forget the OLD generation.
///
/// ⚠️ **What this test does NOT COVER: the WIRING.** It calls
/// `attendre_le_media` and `oublier` directly, exactly as a correct caller
/// would — but also exactly as a caller would
/// that passed the WRONG generation to `oublier` (for example a
/// generation captured too early, or that of another session). The four
/// real call sites (`serveur.rs`, in `ouvrir_les_commandes` and
/// `tenir_la_fenetre`) each carry the generation they received from THEIR
/// own `attendre_le_media` — checked by READING, not by this test. Nothing
/// cheap would cover it (it would take real named pipes, hence the
/// VM): a successor must not read "this test passes" as proof
/// that the production wiring is correct.
#[test]
fn un_oubli_perime_ne_retire_pas_l_attente_neuve() {
    // Name specific to this test: `ETAT` is a GLOBAL state of the test process,
    // shared with the other tests of this module.
    let session = "test-course-f5-second-registre";
    etat().attentes.remove(session);

    let (media1, _recepteur1) = channel::<std::fs::File>();
    let g1 = attendre_le_media(session, media1);
    let (media2, _recepteur2) = channel::<std::fs::File>();
    let g2 = attendre_le_media(session, media2); // re-attach
    assert!(g2 > g1);

    oublier(session, g1); // the old thread wakes up too late
    assert!(
        etat().attentes.contains_key(session),
        "forgetting generation 1 must not take generation 2 with it"
    );

    oublier(session, g2);
    assert!(!etat().attentes.contains_key(session));
}
