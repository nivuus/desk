//! Throughput and ordering of the bridge's transport, on a real str0m peer in
//! local loopback.
//!
//! 🔴 **Both tests were RED on the previous loop**, each for its own defect:
//!
//! - the browser → bridge direction read ONE datagram per turn and slept up to
//!   `ATTENTE_MAX` before each read: ~33 KiB/s (F4 §7.2). Two MiB then take
//!   about a minute, and the scaffolding's `BUDGET` (10 s) expires;
//! - the bridge → browser direction IGNORED str0m's refusal when its SCTP
//!   buffer (128 KiB) was full: the third 64 KiB frame of a burst was lost.

use super::tests::{canal_ouvert, echanger, monter};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::*;

/// A full-size bridge frame: `MAX_FRAME_SIZE` of payload, numbered by its
/// correlation and filled with it, so that a swap or a loss shows.
fn trame_pleine(numero: u32) -> Vec<u8> {
    proto::files::encoder(
        proto::files::TYPE_DATA,
        numero,
        "{}",
        &vec![numero as u8; proto::files::MAX_FRAME_SIZE],
    )
}

#[test]
fn two_mebibytes_from_the_peer_surface_in_order_within_the_budget() {
    const TRAMES: u32 = 32; // 32 × 64 KiB = 2 MiB
    let (mut pair, _sortant, entrant) = monter(&[FILES_LABEL]);
    let canal = pair.canaux[0];
    let mut prochaine = 0u32;

    let (remontees, _) = echanger(
        &mut pair,
        &entrant,
        |rtc, remontees| {
            if !canal_ouvert(remontees) {
                return;
            }
            // As many as the peer's SCTP buffer accepts, then the next turn.
            while prochaine < TRAMES {
                let Some(mut c) = rtc.channel(canal) else {
                    return;
                };
                if !c.write(true, &trame_pleine(prochaine)).unwrap_or(false) {
                    return;
                }
                prochaine += 1;
            }
        },
        |remontees, _| {
            remontees
                .iter()
                .filter(|m| matches!(m, DuNavigateur::Reponse { .. }))
                .count()
                == TRAMES as usize
        },
        "two MiB from the peer surfacing",
    );

    let correlations: Vec<u32> = remontees
        .iter()
        .filter_map(|m| match m {
            DuNavigateur::Reponse { correlation, trame } => {
                assert_eq!(trame, &trame_pleine(*correlation), "a frame altered");
                Some(*correlation)
            }
            _ => None,
        })
        .collect();
    assert_eq!(correlations, (0..TRAMES).collect::<Vec<_>>(), "order lost");
}

#[test]
fn a_burst_beyond_the_sctp_buffer_reaches_the_peer_whole_and_in_order() {
    const TRAMES: u32 = 8; // 512 KiB, four times str0m's 128 KiB buffer
    let (mut pair, sortant, entrant) = monter(&[FILES_LABEL]);
    let mut emise = false;

    let (_, recus) = echanger(
        &mut pair,
        &entrant,
        |_, remontees| {
            if !emise && canal_ouvert(remontees) {
                emise = true;
                for numero in 0..TRAMES {
                    sortant
                        .send(VersNavigateur::Requete {
                            correlation: numero,
                            trame: trame_pleine(numero),
                            echeance: None,
                        })
                        .unwrap();
                }
            }
        },
        |_, recus| recus.len() == TRAMES as usize,
        "a 512 KiB burst reaching the peer",
    );

    for (numero, (_, binaire, octets)) in recus.iter().enumerate() {
        assert!(binaire);
        assert_eq!(
            octets,
            &trame_pleine(numero as u32),
            "frame {numero}: lost or swapped"
        );
    }
}

#[test]
fn a_waiting_request_whose_command_expired_is_dropped_never_emitted_late() {
    let maintenant = Instant::now();
    let mut en_attente: EnAttente = VecDeque::from([
        (1, vec![1], Some(maintenant - Duration::from_millis(1))),
        (2, vec![2], None),
        (3, vec![3], Some(maintenant + Duration::from_secs(5))),
        (4, vec![4], Some(maintenant)),
    ]);
    assert_eq!(retirer_expirees(&mut en_attente, maintenant), 2);
    let restantes: Vec<u32> = en_attente.iter().map(|(c, _, _)| *c).collect();
    assert_eq!(restantes, vec![2, 3], "the survivors keep their order");
}
