//! The read window. **Pure, run on the host.**

use super::*;

fn morceaux(n: usize) -> VecDeque<Morceau> {
    (0..n)
        .map(|i| Morceau {
            position: (i as u64) * 4096,
            length: 4096,
        })
        .collect()
}

/// 🔴 **THE BOUND APPLIES TO THE TOTAL IN FLIGHT, NOT TO THE BATCH.**
///
/// Red: remove the bound. The SCTP queue would fill without end, and the channel
/// would become the latency source of everything else — which this module exists
/// precisely to prevent.
#[test]
fn the_window_never_requests_more_chunks_in_flight() {
    let mut f = Fenetre::new(morceaux(50));
    let lot = f.a_demander();
    assert_eq!(lot.len(), MORCEAUX_EN_VOL);
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
    // A second call WITHOUT receipt must add NOTHING: bounding the batch instead
    // of the total would make sixteen in flight here.
    assert!(
        f.a_demander().is_empty(),
        "no more chunks as long as nothing is received"
    );
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
}

/// 🔴 **THE RED OF THE DELIVERABLE ITSELF.**
///
/// With `MORCEAUX_EN_VOL = 1`, the mechanism is **INERT**: the bridge is
/// never ahead, and the browser's back-pressure never has anything to
/// hold back. F3 would then have delivered a flow control unable to bite —
/// that is, a check we would never see red, applied to a
/// PRODUCT mechanism.
///
/// This test fails if the constant falls back to 1, and it also fails if the window
/// stops filling after a receipt.
#[test]
#[allow(non_snake_case)]
fn the_window_really_reaches_MORCEAUX_EN_VOL_on_a_long_read() {
    const {
        assert!(
            MORCEAUX_EN_VOL > 1,
            "a window of 1 is INERT: see the module doc"
        )
    };
    let mut f = Fenetre::new(morceaux(50));
    for _ in 0..20 {
        for m in f.a_demander() {
            let _ = m;
        }
        let position = *f.en_vol.front().expect("some remain");
        f.recu(position).expect("in order");
    }
    assert_eq!(
        f.en_vol_max(),
        MORCEAUX_EN_VOL,
        "the window was never full: the mechanism is inert"
    );
}

/// 🔴 **AN OUT-OF-ORDER RESPONSE IS DENOUNCED, AND NOT APPLIED.**
///
/// Red: apply it anyway. The file would have its ranges out of
/// order, and **the SHA-256 digest would be the only criterion to catch it**
/// — the one F1 NEVER established.
#[test]
fn an_out_of_order_answer_is_denounced_and_not_applied() {
    let mut f = Fenetre::new(morceaux(10));
    f.a_demander();
    let error = f.recu(4096).expect_err("position 4096 is not the oldest");
    assert_eq!(
        error,
        HorsOrdre {
            recue: 4096,
            attendue: Some(0)
        }
    );
    // Nothing moved: the response was not consumed.
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
    // And the right response still goes through.
    assert!(f.recu(0).is_ok());
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL - 1);
}

/// An **unknown** position is denounced too, and the message says what
/// we expected.
#[test]
fn an_unknown_position_is_denounced() {
    let mut f = Fenetre::new(morceaux(2));
    f.a_demander();
    assert_eq!(
        f.recu(999_999).expect_err("position never requested"),
        HorsOrdre {
            recue: 999_999,
            attendue: Some(0)
        }
    );
}

/// A response while **nothing** is in flight is denounced, and `attendue` is
/// `None` — which distinguishes "you answered me at the wrong moment" from "you
/// answered me the wrong range".
#[test]
fn a_reply_with_nothing_in_flight_is_reported_with_expected_none() {
    let mut f = Fenetre::new(morceaux(1));
    f.a_demander();
    f.recu(0).expect("the only one");
    assert_eq!(
        f.recu(0).expect_err("nothing in flight any more"),
        HorsOrdre {
            recue: 0,
            attendue: None
        }
    );
}

/// The window empties and fills: after a receipt, one more chunk
/// is requested, and not two.
#[test]
fn a_reception_frees_exactly_one_slot() {
    let mut f = Fenetre::new(morceaux(10));
    f.a_demander();
    f.recu(0).unwrap();
    let lot = f.a_demander();
    assert_eq!(lot.len(), 1);
    assert_eq!(lot[0].position, (MORCEAUX_EN_VOL as u64) * 4096);
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
}

/// Chunks are requested in **increasing** position order: it is
/// what makes the ordering invariant true, and it is not assumed.
#[test]
fn pieces_are_requested_in_increasing_order() {
    let mut f = Fenetre::new(morceaux(12));
    let mut vues = Vec::new();
    while !f.terminee() {
        for m in f.a_demander() {
            vues.push(m.position);
        }
        let position = *f.en_vol.front().unwrap();
        f.recu(position).unwrap();
    }
    let mut triees = vues.clone();
    triees.sort_unstable();
    assert_eq!(vues, triees, "positions must be increasing");
    assert_eq!(vues.len(), 12);
}

/// An empty read is finished straight away — and requests nothing.
///
/// ⚠️ `decoupe::decouper` deliberately returns ZERO chunks for a zero
/// length. Without this case, the window would wait for a response that would never
/// come, and the ProjFS command would expire on a perfectly read file.
#[test]
fn a_read_without_chunks_is_finished_at_once() {
    let mut f = Fenetre::new(VecDeque::new());
    assert!(f.terminee());
    assert!(f.a_demander().is_empty());
    assert_eq!(f.en_vol_max(), 0);
}

/// `terminee()` is only true when **both** are empty: what remains to
/// request AND what is in flight.
///
/// Red: only test `restants`. The read would declare itself complete while
/// four chunks are still in flight, and the file would be truncated by
/// four frames — without any error being returned.
#[test]
fn finished_requires_the_flight_to_be_empty_too() {
    let mut f = Fenetre::new(morceaux(2));
    f.a_demander();
    assert!(!f.terminee(), "two chunks are in flight");
    f.recu(0).unwrap();
    assert!(!f.terminee(), "one chunk is still in flight");
    f.recu(4096).unwrap();
    assert!(f.terminee());
}
