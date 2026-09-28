//! Tests of `SourceDistante` — all on the host, without any `#[cfg(windows)]`.
//!
//! Sibling file rather than inline module: `distante.rs` was at 487
//! lines for a project cap of 500, and the exhaustion guard from the
//! final branch review (I2) plus its test would have pushed it over.
//! Extract rather than compress — same scheme as
//! `superviseur/table/tests_retention.rs`.

use super::*;
// `VideoSource` is imported HERE since the trait implementation was
// extracted to `distante/video_source.rs` (sub-block A1): the parent no longer
// uses it, and a trait must be in scope for its methods to be
// callable.
use crate::capteur::reprise::{DUREE_FENETRE_CANAL, PAS_RATTACHEMENT};
use crate::source::VideoSource;
use std::sync::mpsc::sync_channel;

/// What the fake channel will return at the next `rattacher`. `None` = failure.
/// A queue, so that tests can chain failures then successes.
type ProchainsRattachements = std::sync::Arc<std::sync::Mutex<Vec<Option<u32>>>>;

/// Fake channel: returns prepared replies and records what was
/// asked, so that tests check the SENT message and not only
/// the effect.
struct CanalFactice {
    reponses: Vec<anyhow::Result<DepuisCapteur>>,
    recus: std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
    rattachements: ProchainsRattachements,
    attempts: std::sync::Arc<std::sync::Mutex<u32>>,
}

impl Canal for CanalFactice {
    fn commander(&mut self, message: VersCapteur) -> anyhow::Result<DepuisCapteur> {
        self.recus.lock().unwrap().push(message);
        if self.reponses.is_empty() {
            Ok(DepuisCapteur::Fait)
        } else {
            self.reponses.remove(0)
        }
    }

    fn rattacher(&mut self) -> anyhow::Result<Rattachee> {
        *self.attempts.lock().unwrap() += 1;
        let prochain = {
            let mut file = self.rattachements.lock().unwrap();
            if file.is_empty() {
                None
            } else {
                file.remove(0)
            }
        };
        match prochain {
            Some(largeur) => {
                let (tx, rx) = sync_channel(4);
                // A frame in the new queue: it is what will prove
                // that the source does read the NEW channel.
                tx.send(Recu::Image(AccessUnit {
                    data: vec![7],
                    is_keyframe: true,
                    pts_90k: 700,
                }))
                .unwrap();
                Ok(Rattachee {
                    images: rx,
                    largeur,
                    hauteur: 480,
                })
            }
            None => anyhow::bail!("no sensor"),
        }
    }
}

/// `_capacite` is kept so as not to change the signature called by
/// the existing tests, but `source_rattachable` sets its own to 4 —
/// which covers all current uses of `source_with`.
pub(super) fn source_with(
    _capacite: usize,
) -> (
    SourceDistante,
    std::sync::mpsc::SyncSender<Recu>,
    std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
) {
    let (source, tx, recus, _, _) = source_rattachable(Vec::new());
    (source, tx, recus)
}

/// Like `source_with`, but the fake channel returns the given `reponses`, in
/// order, before falling back on its default `Fait`. That is what makes it possible
/// to test a REFUSAL from the sensor.
pub(super) fn source_with_replies(
    reponses: Vec<anyhow::Result<DepuisCapteur>>,
) -> (
    SourceDistante,
    std::sync::mpsc::SyncSender<Recu>,
    std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
) {
    let (tx, rx) = sync_channel(4);
    let recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let canal = CanalFactice {
        reponses,
        recus: recus.clone(),
        rattachements: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        attempts: std::sync::Arc::new(std::sync::Mutex::new(0)),
    };
    (
        SourceDistante::new(Box::new(canal), rx, 1280, 720),
        tx,
        recus,
    )
}

#[allow(clippy::type_complexity)]
pub(super) fn source_rattachable(
    rattachements: Vec<Option<u32>>,
) -> (
    SourceDistante,
    std::sync::mpsc::SyncSender<Recu>,
    std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
    ProchainsRattachements,
    std::sync::Arc<std::sync::Mutex<u32>>,
) {
    let (tx, rx) = sync_channel(4);
    let recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let file = std::sync::Arc::new(std::sync::Mutex::new(rattachements));
    let attempts = std::sync::Arc::new(std::sync::Mutex::new(0));
    let canal = CanalFactice {
        reponses: Vec::new(),
        recus: recus.clone(),
        rattachements: file.clone(),
        attempts: attempts.clone(),
    };
    (
        SourceDistante::new(Box::new(canal), rx, 1280, 720),
        tx,
        recus,
        file,
        attempts,
    )
}

#[test]
fn une_image_poussee_est_rendue_par_next_frame() {
    let (mut source, tx, _) = source_with(4);
    tx.send(Recu::Image(AccessUnit {
        data: vec![1, 2],
        is_keyframe: true,
        pts_90k: 42,
    }))
    .unwrap();
    let unite = source.next_frame().expect("an image was queued");
    assert_eq!(unite.pts_90k, 42);
    assert!(unite.is_keyframe);
}

/// The COMMON case: nothing new. It must be free and above all not
/// pass for an exhaustion — the transport loop polls at 100 Hz.
#[test]
fn une_file_vide_rend_none_sans_epuiser_la_source() {
    let (mut source, _tx, _) = source_with(4);
    assert!(source.next_frame().is_none());
    assert!(!source.is_exhausted());
    assert!(source.is_alive());
}

#[test]
fn frames_come_out_in_arrival_order() {
    let (mut source, tx, _) = source_with(4);
    for pts in [1, 2, 3] {
        tx.send(Recu::Image(AccessUnit {
            data: vec![],
            is_keyframe: false,
            pts_90k: pts,
        }))
        .unwrap();
    }
    let rendus: Vec<u64> = (0..3)
        .map(|_| source.next_frame().unwrap().pts_90k)
        .collect();
    assert_eq!(rendus, vec![1, 2, 3]);
}

/// `Etat` is not a frame: it updates the cache and reading
/// continues, without consuming the round.
#[test]
fn un_etat_intercale_met_a_jour_le_cache_sans_masquer_l_image_suivante() {
    let (mut source, tx, _) = source_with(4);
    tx.send(Recu::Etat {
        vivante: true,
        epuisee: false,
        largeur: 800,
        hauteur: 600,
    })
    .unwrap();
    tx.send(Recu::Image(AccessUnit {
        data: vec![],
        is_keyframe: false,
        pts_90k: 5,
    }))
    .unwrap();
    assert_eq!(source.next_frame().unwrap().pts_90k, 5);
    assert_eq!(source.dimensions(), (800, 600));
}

#[test]
fn une_fenetre_disparue_rend_la_source_non_vivante_et_epuisee() {
    let (mut source, tx, _) = source_with(4);
    tx.send(Recu::Etat {
        vivante: false,
        epuisee: true,
        largeur: 1280,
        hauteur: 720,
    })
    .unwrap();
    assert!(source.next_frame().is_none());
    assert!(!source.is_alive());
    assert!(source.is_exhausted());
}

/// **I2 from the final branch review of sub-block D4.** The test above
/// keeps `tx` alive, hence NEVER reaches `Disconnected`: it is this one
/// that tests the real sequence of events on the normal close of a
/// window — the sensor pushes `Etat { epuisee: true }`, THEN closes the pipe.
///
/// The child must then conclude, and above all not re-attach: a re-attachment
/// would make the sensor reopen a DXGI duplication and an encoder on an
/// output the supervisor is destroying at the same instant, and if it succeeded it
/// would reset `epuisee` to false — the session that was meant to close would not
/// close.
///
/// The re-attachment queue carries a success ON PURPOSE: without the guard, the
/// re-attachment would not merely be attempted, it would SUCCEED.
#[test]
fn un_epuisement_autoritaire_interdit_tout_rattachement() {
    let (mut source, tx, _, _, attempts) = source_rattachable(vec![Some(1600)]);
    tx.send(Recu::Etat {
        vivante: false,
        epuisee: true,
        largeur: 1280,
        hauteur: 720,
    })
    .unwrap();
    assert!(source.next_frame().is_none());
    assert!(
        source.is_exhausted(),
        "the authoritative state exhausts the source"
    );

    // The sensor closes the pipe: the next round sees `Disconnected`.
    drop(tx);
    assert!(source.next_frame().is_none());
    assert_eq!(
        *attempts.lock().unwrap(),
        0,
        "no reattachment must be attempted after an authoritative exhaustion"
    );
    assert!(source.is_exhausted(), "the exhaustion stays acquired");
    assert!(!source.is_alive());
}

#[test]
fn commands_leave_in_the_expected_shape() {
    let (mut source, _tx, recus) = source_with(4);
    source.set_bitrate(3_000_000).unwrap();
    source.set_encode_size(640, 360).unwrap();
    source.request_keyframe().unwrap();
    let recus = recus.lock().unwrap();
    assert_eq!(
        *recus,
        vec![
            VersCapteur::Debit { bps: 3_000_000 },
            VersCapteur::EncodeSize {
                largeur: 640,
                hauteur: 360
            },
            VersCapteur::ImageCle,
        ]
    );
}

/// `resize` must keep the size ACTUALLY obtained, not the one requested
/// — same rule as in single-window mode (`transport/redimensionnement.rs`).
/// The test uses initial dimensions DIFFERENT from the reply
/// to check that the reply is actually adopted (and not ignored).
#[test]
fn a_resize_keeps_the_obtained_size() {
    let (tx_img, rx) = sync_channel(4);
    let recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let canal = CanalFactice {
        reponses: vec![Ok(DepuisCapteur::Size {
            largeur: 1280,
            hauteur: 720,
        })],
        recus: recus.clone(),
        rattachements: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        attempts: std::sync::Arc::new(std::sync::Mutex::new(0)),
    };
    let mut source = SourceDistante::new(Box::new(canal), rx, 640, 480);
    drop(tx_img);
    source.resize(1281, 713).unwrap();
    assert_eq!(source.dimensions(), (1280, 720));
}

#[test]
fn a_capturer_error_surfaces_as_an_error() {
    let (_tx, rx) = sync_channel(4);
    let canal = CanalFactice {
        reponses: vec![Ok(DepuisCapteur::Error {
            motif: "encoder lost".into(),
        })],
        recus: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        rattachements: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        attempts: std::sync::Arc::new(std::sync::Mutex::new(0)),
    };
    let mut source = SourceDistante::new(Box::new(canal), rx, 1280, 720);
    let error = source.set_bitrate(1).unwrap_err().to_string();
    assert!(
        error.contains("encoder lost"),
        "unexpected message: {error}"
    );
}

/// The heart of criterion 2: killing the sensor closes the pipe, hence breaks the
/// channel — and that must NOT close the session, otherwise
/// `brancher_video` calls `begin_ending("video source exhausted")`.
#[test]
fn a_broken_channel_does_not_exhaust_the_source_within_the_window() {
    let (mut source, tx, _) = source_with(4);
    drop(tx);
    assert!(source.next_frame().is_none());
    assert!(
        !source.is_exhausted(),
        "a channel break is not an exhaustion"
    );
}

/// But a lasting break does: without it, a dead session
/// would stay open indefinitely on a frozen frame.
#[test]
fn un_canal_rompu_au_dela_de_la_fenetre_epuise_la_source() {
    let (mut source, tx, _) = source_with(4);
    drop(tx);
    assert!(source.next_frame().is_none());
    source.vieillir_pour_test(
        crate::capteur::reprise::DUREE_FENETRE_CANAL + std::time::Duration::from_millis(1),
    );
    assert!(source.next_frame().is_none());
    assert!(source.is_exhausted());
}

/// The heart of criterion 2: the sensor dies, it is restarted, and the session
/// resumes — same new queue, same dimensions announced by the sensor.
#[test]
fn un_rattachement_reussi_fait_revivre_la_source_et_reprend_ses_dimensions() {
    let (mut source, tx, _, rattachements, attempts) = source_rattachable(vec![Some(1600)]);
    drop(tx);
    // First round: break observed, re-attachment attempted and successful.
    assert!(
        source.next_frame().is_none(),
        "the round of the break yields no image"
    );
    assert_eq!(*attempts.lock().unwrap(), 1);
    assert!(rattachements.lock().unwrap().is_empty());
    // Next round: the frame comes from the NEW queue.
    let unite = source
        .next_frame()
        .expect("the fresh queue carries an image");
    assert_eq!(unite.pts_90k, 700);
    assert_eq!(
        source.dimensions(),
        (1600, 480),
        "the dimensions of the relaunched sensor"
    );
    assert!(!source.is_exhausted());
    assert!(source.is_alive());
}

/// A failed re-attachment concludes nothing: the window is still running.
#[test]
fn un_rattachement_qui_echoue_laisse_la_source_en_attente_sans_l_epuiser() {
    let (mut source, tx, _, _, attempts) = source_rattachable(vec![None]);
    drop(tx);
    assert!(source.next_frame().is_none());
    assert_eq!(*attempts.lock().unwrap(), 1);
    assert!(
        !source.is_exhausted(),
        "a reattachment failure does not exhaust"
    );
}

/// But a failure lasting beyond the window does: without it a
/// dead session would stay open indefinitely on a frozen frame.
///
/// ⚠️ This test does NOT exercise a second re-attachment attempt: once
/// `DUREE_FENETRE_CANAL` is exceeded, `rupture()` short-circuits and returns
/// `true` before even reaching `can_retry` (`Disconnected` branch
/// of `next_frame`) — whether `vieillir_pour_test` ages
/// `last_attempt` or not therefore has no effect HERE. That band
/// (ageing beyond the spacing step alone, while staying within the
/// window) is the one tested by `ageing_of_the_step_alone_relaunches_an_attempt`.
#[test]
fn un_rattachement_qui_echoue_jusqu_a_expiration_epuise_la_source() {
    let (mut source, tx, _, _, _) = source_rattachable(vec![None]);
    drop(tx);
    assert!(source.next_frame().is_none());
    source.vieillir_pour_test(DUREE_FENETRE_CANAL + std::time::Duration::from_millis(1));
    assert!(source.next_frame().is_none());
    assert!(source.is_exhausted());
}

/// Ageing WITHIN the window, beyond the spacing step alone: a second
/// attempt must go out. This test is the only one to check that
/// `vieillir_pour_test` does age `last_attempt` — the expiry one
/// never reaches it, `rupture` short-circuiting before.
#[test]
fn ageing_of_the_step_alone_relaunches_an_attempt() {
    let (mut source, tx, _, _, attempts) = source_rattachable(vec![None, None]);
    drop(tx);
    assert!(source.next_frame().is_none());
    assert_eq!(*attempts.lock().unwrap(), 1);
    source.vieillir_pour_test(PAS_RATTACHEMENT + std::time::Duration::from_millis(1));
    assert!(source.next_frame().is_none());
    assert_eq!(
        *attempts.lock().unwrap(),
        2,
        "the elapsed step allows a second attempt"
    );
    assert!(!source.is_exhausted(), "we are still inside the window");
}

/// Without spacing, a break would cause ~100 attempts per second.
#[test]
fn a_burst_of_polls_produces_a_single_attempt() {
    let (mut source, tx, _, _, attempts) = source_rattachable(vec![None, None, None, None]);
    drop(tx);
    for _ in 0..10 {
        assert!(source.next_frame().is_none());
    }
    assert_eq!(
        *attempts.lock().unwrap(),
        1,
        "a single attempt in the burst"
    );
}
