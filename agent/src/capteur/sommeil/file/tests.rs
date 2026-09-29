//! Tests of `capteur::sommeil::file` — sibling file rather than inline
//! module.
//!
//! **Extraction done in a DEDICATED task, BEFORE the one that adds** (fix
//! round 1, 25 August 2026): `file.rs` was at 445 lines for a
//! project cap of 500, and the fixes that follow add code
//! AND doc to it. The repository's rule is "extract, never compress", and
//! "the regained margin is lost again if treated as settled" — paid for six
//! times.
//!
//! ⚠️ **`#[path]` in the parent, and it is NOT the
//! `<parent>_<child>` convention.** That one only targets modules extracted from a
//! `#[cfg(windows)]` parent to compile on the host; here the Rust mechanism
//! is the same but the reason is different — splitting a TEST module that is too
//! long inside an otherwise portable file. `docs/claude/module-conventions.md` puts this
//! case explicitly OUTSIDE the scope of that convention, and names the
//! precedent: `superviseur/table.rs`, which likewise declares
//! `#[path = "table/tests.rs"] mod tests;` and
//! `#[path = "table/tests_relance.rs"] mod tests_relance;`.
//!
//! **The module path stays `file::tests`**: only the physical location
//! of the file changes, no visibility is touched.

use super::*;
use crate::capteur::sommeil::{Message, Ordre};
use std::collections::VecDeque;

/// 🔴 THE HEART OF THE DECISION: two `Part` without a read leave only
/// ONE, and it is the LAST value that survives.
#[test]
fn deux_parts_se_coalescent_en_une_seule() {
    let mut f = VecDeque::new();
    assert!(matches!(
        deposer(&mut f, Message::Part { bps: 1 }),
        Depot::Empilee
    ));
    assert!(matches!(
        deposer(&mut f, Message::Part { bps: 2 }),
        Depot::Coalescee
    ));
    assert_eq!(f.len(), 1);
    assert!(matches!(f[0], Message::Part { bps: 2 }));
}

/// 🔴 THE CRITERION THAT DISTINGUISHES IN-PLACE COALESCING FROM TAIL COALESCING,
/// and it is the invariant `sommeil.rs` writes: within a session, the
/// channel guarantees the DELIVERY ORDER between variants. Coalescing at the
/// tail would make the share cross a sleep order dropped in the meantime.
#[test]
fn la_coalescence_conserve_la_position() {
    let mut f = VecDeque::new();
    let _ = deposer(&mut f, Message::Part { bps: 1 });
    let _ = deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
    let _ = deposer(&mut f, Message::Part { bps: 2 });
    assert_eq!(f.len(), 2);
    assert!(
        matches!(f[0], Message::Part { bps: 2 }),
        "the share keeps its PLACE"
    );
    assert!(matches!(f[1], Message::Sommeil(Ordre::Reveiller)));
}

/// A lost order leaves a window wrongly asleep or awake.
#[test]
fn un_ordre_de_sommeil_n_est_jamais_coalesce() {
    let mut f = VecDeque::new();
    let _ = deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
    assert!(matches!(
        deposer(&mut f, Message::Sommeil(Ordre::Reveiller)),
        Depot::Empilee
    ));
    assert_eq!(f.len(), 2);
}

/// A lost clipboard is the user's data.
#[test]
fn un_presse_papier_n_est_jamais_coalesce() {
    let mut f = VecDeque::new();
    let _ = deposer(
        &mut f,
        Message::PressePapier {
            texte: Some("a".into()),
            octets: 1,
        },
    );
    let d = deposer(
        &mut f,
        Message::PressePapier {
            texte: Some("b".into()),
            octets: 1,
        },
    );
    assert!(matches!(d, Depot::Empilee));
    assert_eq!(f.len(), 2);
}

/// The hard bound REFUSES, it does not silently truncate.
#[test]
fn au_dela_de_la_borne_le_depot_est_refuse() {
    let mut f = VecDeque::new();
    for _ in 0..PROFONDEUR_MAX {
        let _ = deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
    }
    assert!(matches!(
        deposer(&mut f, Message::Sommeil(Ordre::Reveiller)),
        Depot::Refusee
    ));
    assert_eq!(f.len(), PROFONDEUR_MAX, "the queue did not grow");
}

/// 🔴 THE NEGATIVE WITNESS, AND ITS EXACT GUARANTEE: **once an
/// occurrence of the variant is ALREADY queued**, a drop of that
/// variant never hits the bound, whatever the rate — it
/// coalesces, so it does not even test the bound. Without it, "refused"
/// above would not say that coalescing really bounds.
///
/// ⚠️ **THE GUARANTEE IS NO BROADER THAN THAT, and the original name
/// (`une_variante_coalescable_ne_bute_jamais_sur_la_borne`) OVER-CLAIMED.**
/// The FIRST drop of a coalescable variant, on the other hand, stacks like the
/// others and hits the bound if the queue is full of non-coalescables:
/// that is the case `un_premier_depot_coalescable_bute_bien_sur_la_borne`
/// measures just below. It is not a defect — the refusal is explicit,
/// never a truncation — but an over-claim is the class of
/// defect this repository fights first.
#[test]
fn une_variante_deja_en_file_ne_bute_jamais_sur_la_borne() {
    let mut f = VecDeque::new();
    for i in 0..(PROFONDEUR_MAX * 10) {
        let d = deposer(&mut f, Message::Part { bps: i as u32 });
        assert!(!matches!(d, Depot::Refusee));
    }
    assert_eq!(f.len(), 1);
}

/// 🔴 WHAT THE REVIEW OF THE PREVIOUS TASK MEASURED, and the witness
/// above does not say: the FIRST `Part` dropped on a queue full of
/// NON-COALESCABLE variants has nothing to replace, so it stacks — so
/// it is REFUSED. **It is not a defect**: the refusal is explicit and
/// counted, never a silent truncation. It is the bound of the
/// guarantee, and it is now tested rather than assumed.
#[test]
fn un_premier_depot_coalescable_bute_bien_sur_la_borne() {
    let mut f = VecDeque::new();
    for _ in 0..PROFONDEUR_MAX {
        let _ = deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
    }
    assert!(matches!(
        deposer(&mut f, Message::Part { bps: 1 }),
        Depot::Refusee
    ));
    assert_eq!(f.len(), PROFONDEUR_MAX, "the queue did not grow");
}

/// The pair behaves like the channel it replaces: what is dropped
/// is received, in order.
#[test]
fn what_is_deposited_is_received_in_order() {
    let (e, r) = canal_de_session("test");
    assert!(matches!(
        e.envoyer(Message::Sommeil(Ordre::Reveiller)),
        Envoi::Depose(_)
    ));
    assert!(matches!(
        e.envoyer(Message::PressePapier {
            texte: Some("a".into()),
            octets: 1
        }),
        Envoi::Depose(_)
    ));
    assert!(matches!(r.essayer_recevoir(), Ok(Message::Sommeil(_))));
    assert!(matches!(
        r.essayer_recevoir(),
        Ok(Message::PressePapier { .. })
    ));
    assert_eq!(r.essayer_recevoir(), Err(VideOuFerme::Vide));
}

/// 🔴 THE REFUSAL IS COUNTED. A refusal that is not counted is a refusal
/// no operations will ever see.
#[test]
fn les_refus_se_comptent() {
    let (e, _r) = canal_de_session("test");
    for _ in 0..PROFONDEUR_MAX {
        assert!(matches!(
            e.envoyer(Message::Sommeil(Ordre::Reveiller)),
            Envoi::Depose(_)
        ));
    }
    assert_eq!(
        e.refuses(),
        0,
        "no refusal as long as the bound is not reached"
    );
    // 🔴 AND THE OUTCOME IS `Refuse`, NOT `Depose`: it is the distinction no
    // caller made before fix round 1, and which the type
    // now imposes on each of them.
    assert_eq!(e.envoyer(Message::Sommeil(Ordre::Reveiller)), Envoi::Refuse);
    assert_eq!(e.refuses(), 1);
}

/// The sender knows nobody reads any more.
///
/// 🔴 IT IS THE TEST THAT HOLDS THE PURGE OF DEAD SESSIONS in the registry:
/// `distribuer` removes a session on `Envoi::Rompu`, and nothing else does
/// on this path.
#[test]
fn un_receveur_tombe_ferme_l_emetteur() {
    let (e, r) = canal_de_session("test");
    drop(r);
    assert_eq!(e.envoyer(Message::Sommeil(Ordre::Reveiller)), Envoi::Rompu);
}

/// 🔴 THE THREE OUTCOMES ARE PAIRWISE DISTINCT, AND THAT IS WHAT HAD TO BE
/// ESTABLISHED: `Refuse` is neither `Depose` nor `Rompu`.
///
/// Under `mpsc`, `send(...).is_ok()` meant "delivered"; here it would have crushed
/// `Depose` and `Refuse` into a single value, and that is exactly the confusion
/// that cost two faulty memorisations (`dernieres_parts`,
/// `derniers_audio`). Without this test, nothing would forbid a future `envoyer` from
/// returning `Refuse` on a dropped receiver, or the reverse.
#[test]
fn the_refusal_is_confused_neither_with_delivery_nor_with_breakage() {
    let (e, r) = canal_de_session("test");
    for _ in 0..PROFONDEUR_MAX {
        assert!(matches!(
            e.envoyer(Message::Sommeil(Ordre::Reveiller)),
            Envoi::Depose(_)
        ));
    }
    // File pleine, receveur VIVANT.
    assert_eq!(e.envoyer(Message::Sommeil(Ordre::Reveiller)), Envoi::Refuse);
    // The same send, receiver DROPPED: the outcome changes, and the queue has nothing
    // to do with it — that is what distinguishes the two causes.
    drop(r);
    assert_eq!(e.envoyer(Message::Sommeil(Ordre::Reveiller)), Envoi::Rompu);
}

/// The receiver distinguishes "nothing to read" from "nobody writes any more".
#[test]
fn un_emetteur_tombe_se_distingue_d_une_file_vide() {
    let (e, r) = canal_de_session("test");
    assert_eq!(r.essayer_recevoir(), Err(VideOuFerme::Vide));
    assert!(matches!(
        e.envoyer(Message::Sommeil(Ordre::Reveiller)),
        Envoi::Depose(_)
    ));
    drop(e);
    // What remains queued can STILL be read: closing throws nothing away.
    assert!(matches!(r.essayer_recevoir(), Ok(Message::Sommeil(_))));
    assert_eq!(r.essayer_recevoir(), Err(VideOuFerme::Ferme));
}

/// `drain` returns what is waiting, in order, and leaves the queue empty.
#[test]
fn drain_returns_everything_waiting_in_order() {
    let (e, r) = canal_de_session("test");
    assert!(matches!(
        e.envoyer(Message::Part { bps: 7 }),
        Envoi::Depose(_)
    ));
    assert!(matches!(
        e.envoyer(Message::Sommeil(Ordre::Reveiller)),
        Envoi::Depose(_)
    ));
    let recus = r.drain();
    assert_eq!(recus.len(), 2);
    assert!(matches!(recus[0], Message::Part { bps: 7 }));
    assert!(matches!(recus[1], Message::Sommeil(Ordre::Reveiller)));
    assert!(r.drain().is_empty());
}
