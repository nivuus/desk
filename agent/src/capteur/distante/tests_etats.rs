//! Tests of the STATES the sensor pushes to `SourceDistante` — visibility,
//! sleep, budget share, audio order, fullscreen, clipboard.
//!
//! **Extracted from `distante/tests.rs` VERBATIM on 20 August 2026**, clipboard
//! sub-block P1, task 10: the sibling file was at **474 lines** for
//! a project cap of 500, a margin of 26 the test of
//! `Recu::PressePapier` would have eaten into — and §7.2 of the specification did
//! not list this file at all (plan divergence E1). The repository's rule
//! is to extract BEFORE adding, never to compress.
//!
//! The split follows what the tests exercise: what ARRIVES through the `Recu`
//! queue and is re-read through a `…_a_annoncer` / `…_a_appliquer` method lives
//! here; frames, re-attachment and commands stay with the sibling,
//! along with the factories (`source_with`, `source_rattachable`) and the fake
//! channel, which this file borrows rather than duplicating them.

use super::tests::{source_rattachable, source_with};
use super::*;
// `VideoSource` is imported HERE since the trait implementation was
// extracted to `distante/video_source.rs` (sub-block A1): the parent no longer
// uses it, and a trait must be in scope for its methods to be
// callable.
use crate::source::VideoSource;

/// `set_awake` relays visibility as is to the sensor: it is the one that
/// arbitrates globally (task 7). `source_with` serves here as a spy channel, through
/// its third element (`recus`), to check the SENT message.
#[test]
fn set_awake_transmet_la_visibilite_au_capteur() {
    let (mut source, _tx, recus) = source_with(4);
    source.set_awake(false, false).expect("the sensor accepts");
    assert_eq!(
        recus.lock().unwrap().as_slice(),
        &[VersCapteur::Visibilite {
            visible: false,
            focalisee: false
        }]
    );
}

/// `write_clipboard` relays the text as is to the sensor, which is
/// its sole owner (D1). `source_with` serves as a spy channel to
/// check the SENT message, not only the effect.
///
/// RED if `commander_simple` is not called, or if the text is altered on
/// the way — the child has already normalised, bounded and denormalised it, and the sensor
/// has nothing to decide about it.
#[test]
fn write_clipboard_passes_the_text_to_the_capturer() {
    let (mut source, _tx, recus) = source_with(4);
    source
        .write_clipboard("one\r\ntwo")
        .expect("the sensor accepts");
    assert_eq!(
        recus.lock().unwrap().as_slice(),
        &[VersCapteur::ClipboardWrite {
            texte: "one\r\ntwo".to_string()
        }]
    );
}

/// 🔴 **A refusal from the sensor must come back up as `Err`, and that is what prevents
/// the `Ctrl+V` injection**: without it, the key would go out on an
/// unchanged clipboard and would paste the PREVIOUS content.
///
/// RED if the implementation used bare `commander` instead of
/// `commander_simple`: it would then accept any reply,
/// including an `Error`. This test therefore does not check `commander_simple`
/// itself — it checks that IT is the one that was used.
#[test]
fn un_refus_du_capteur_empeche_le_collage() {
    let (mut source, _tx, _recus) =
        super::tests::source_with_replies(vec![Ok(DepuisCapteur::Error {
            motif: "OpenClipboard".into(),
        })]);
    let error = source
        .write_clipboard("colle")
        .expect_err("a refusal from the sensor must surface");
    assert!(
        error.to_string().contains("OpenClipboard"),
        "the sensor's reason must survive: {error}"
    );
}

/// 🔴 **THE TRAIT'S DEFAULT, AND IT IS THE TRAP D10 PAID FOR.** This repository's fake
/// sources implement their side effects as NO-OPs, and 456
/// tests stayed green on a silent product. This test therefore targets a
/// source that **DOES NOT OVERRIDE** the method — `FileSource`, the repository's
/// test source —, that is, the default itself.
///
/// RED if the default is an inert `Ok(())`, like its four neighbours in
/// `source.rs`. That would be SINGLE-WINDOW mode silently pasting the
/// PREVIOUS content at every `Ctrl+V`.
#[test]
fn the_trait_default_refuses_to_write_rather_than_pretend() {
    let source_path =
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
    let mut source = crate::source::FileSource::from_path(source_path, 1280, 720, 60)
        .expect("loading the test stream");
    let error = source
        .write_clipboard("colle")
        .expect_err("the trait's default MUST return Err, never Ok(())");
    assert!(
        error.to_string().contains("no sensor"),
        "the reason must name the cause: {error}"
    );
}

/// A `Sommeil` pushed by the sensor is kept, not ignored: it is
/// `sommeil_a_annoncer` that makes it available to the transport loop, and
/// only once — re-emitting it every round would flood the control
/// channel to the browser. `source_with` serves here as an injectable queue through
/// its second element (`tx`).
#[test]
fn un_sommeil_pousse_par_le_capteur_est_retenu_pour_le_client() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::Sommeil {
        endormie: true,
        raison: "evincee".into(),
    })
    .expect("deposit");
    // The sleep is consumed by the loop round that looks for a frame.
    assert!(source.next_frame().is_none());
    assert_eq!(
        source.sommeil_a_annoncer(),
        Some((true, "evincee".to_string()))
    );
    assert_eq!(
        source.sommeil_a_annoncer(),
        None,
        "an announcement does not repeat"
    );
}

/// `sommeil` is a CURRENT state, not a history: two `Sommeil` received
/// before any read overwrite each other, and only the last must survive — otherwise
/// the transport loop would announce to the browser an already stale
/// state, or worse, a queue of announcements would grow without ever emptying.
#[test]
fn two_consecutive_sleeps_keep_only_the_last() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::Sommeil {
        endormie: true,
        raison: "masquee".into(),
    })
    .expect("deposit");
    tx.send(Recu::Sommeil {
        endormie: true,
        raison: "evincee".into(),
    })
    .expect("deposit");
    assert!(source.next_frame().is_none());
    assert_eq!(
        source.sommeil_a_annoncer(),
        Some((true, "evincee".to_string())),
        "only the last sleep received must survive"
    );
    assert_eq!(source.sommeil_a_annoncer(), None);
}

/// A received share is kept until the transport loop
/// consumes it, and is returned only once — same pattern as
/// `un_sommeil_pousse_par_le_capteur_est_retenu_pour_le_client` above.
#[test]
fn une_part_recue_est_rendue_une_seule_fois() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::Part { bps: 4_000_000 }).expect("deposit");
    // `next_frame` is what drains the channel: without it, nothing is read.
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.part_a_appliquer(), Some(4_000_000));
    assert_eq!(source.part_a_appliquer(), None, "a share is not reapplied");
}

/// Two shares arriving between two reads overwrite each other: it is a current
/// state, not a history — same regime as `Etat` and `Sommeil`.
#[test]
fn two_shares_arriving_before_a_read_overwrite_each_other() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::Part { bps: 4_000_000 }).expect("deposit");
    tx.send(Recu::Part { bps: 2_000_000 }).expect("deposit");
    assert_eq!(source.next_frame(), None);
    assert_eq!(
        source.part_a_appliquer(),
        Some(2_000_000),
        "only the last one survives"
    );
}

/// A received audio order is kept until the transport loop
/// consumes it, and is returned only once — same pattern as
/// `une_part_recue_est_rendue_une_seule_fois` above.
#[test]
fn un_ordre_audio_recu_est_rendu_une_seule_fois() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::Audio { actif: true }).expect("deposit");
    // `next_frame` is what drains the channel: without it, nothing is read.
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.audio_a_appliquer(), Some(true));
    assert_eq!(
        source.audio_a_appliquer(),
        None,
        "an audio order is not reapplied"
    );
}

/// **Au rattachement, l'enfant REDEVIENT MUET** (conception §4.4 ; F2, revue
/// finale de branche).
///
/// What this test catches, and nothing caught before: a carrier whose
/// channel breaks kept its `emet` flag from before the break, because the
/// re-attachment only reset the PENDING order (`None`, "nothing to
/// change") and never the source's state. Two windows of the same PID
/// then played the same mix, out of sync, until the switch-off
/// order arrived — an audible echo.
#[test]
fn un_rattachement_remet_l_enfant_au_silence() {
    let (mut source, tx, _recus, _rattachements, _attempts) = source_rattachable(vec![Some(1600)]);
    // The window carries the sound, and the transport loop has consumed the order:
    // nothing is left pending, only the source's real state knows it.
    tx.send(Recu::Audio { actif: true }).expect("deposit");
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.audio_a_appliquer(), Some(true));

    // Le capteur meurt, l'enfant se rattache.
    drop(tx);
    assert_eq!(
        source.next_frame(),
        None,
        "the round of the break yields no image"
    );

    assert_eq!(
        source.audio_a_appliquer(),
        Some(false),
        "a reattachment must ORDER the silence, not keep quiet on the matter"
    );
}

/// Two audio orders arriving between two reads overwrite each other: same regime
/// as `two_shares_arriving_before_a_read_overwrite_each_other` just above.
#[test]
fn two_audio_orders_arriving_before_a_read_overwrite_each_other() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::Audio { actif: true }).expect("deposit");
    tx.send(Recu::Audio { actif: false }).expect("deposit");
    assert_eq!(source.next_frame(), None);
    assert_eq!(
        source.audio_a_appliquer(),
        Some(false),
        "only the last order survives"
    );
}

/// Same regime as `sommeil_a_annoncer`: the announcement is a CHANGE, it
/// is consumed. Otherwise the transport branch polling it at ~100 Hz
/// would flood the control channel.
#[test]
fn un_plein_ecran_pousse_est_annonce_une_seule_fois() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::PleinEcran { actif: true }).expect("deposit");
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.plein_ecran_a_annoncer(), Some(true));
    assert_eq!(
        source.plein_ecran_a_annoncer(),
        None,
        "an announcement does not repeat"
    );
}

/// Same regime as `plein_ecran_a_annoncer` just above: the announcement is a
/// CURRENT STATE, and it is CONSUMED. Otherwise the `a1septies` branch of
/// `transport/tick.rs`, which polls the source at ~100 Hz, would re-emit the same
/// `AgentControl::Clipboard` a hundred times per second and flood the control
/// channel — the text possibly weighing up to `PRESSE_PAPIER_MAX`.
#[test]
fn un_presse_papier_pousse_est_annonce_une_seule_fois() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::PressePapier {
        texte: Some("bonjour".into()),
        octets: 7,
    })
    .expect("deposit");
    assert_eq!(source.next_frame(), None);
    assert_eq!(
        source.presse_papier_a_annoncer(),
        Some((Some("bonjour".to_string()), 7))
    );
    assert_eq!(
        source.presse_papier_a_annoncer(),
        None,
        "an announcement does not repeat"
    );
}

/// The size REFUSAL (D-P1-1) travels through the same variant, `texte` as `None`
/// and `octets` carrying the refused size: that is what lets the
/// browser's banner state it. Two announcements arriving between two reads
/// overwrite each other — the clipboard IS a state, not a history.
#[test]
fn two_clipboards_arriving_before_a_read_overwrite_each_other_and_the_refusal_passes() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::PressePapier {
        texte: Some("premier".into()),
        octets: 7,
    })
    .expect("deposit");
    tx.send(Recu::PressePapier {
        texte: None,
        octets: 100_000,
    })
    .expect("deposit");
    assert_eq!(source.next_frame(), None);
    assert_eq!(
        source.presse_papier_a_annoncer(),
        Some((None, 100_000)),
        "only the last announcement survives, refusal included"
    );
}

/// Sub-block A1, same regime as `presse_papier_a_annoncer` just above:
/// the announcement is a CURRENT STATE, and it is CONSUMED.
///
/// 🔴 **THIS TEST EXISTS BECAUSE A RED STAYED GREEN.** Red T2 of
/// task 9 mutated `SourceDistante::accent_a_annoncer` into `.clone()` instead of
/// `.take()` and expected
/// `the_announced_accent_is_consumed_and_not_resent_next_round` to fail: it
/// stayed GREEN, because that test uses a FAKE source
/// (`SourceWithAccent`) whose consumption is its own. It tests the
/// WIRING of the a1nonies branch, never `SourceDistante`.
///
/// **Consumption by the REAL `SourceDistante` was therefore covered by
/// NOTHING**, and it is this gap that this test closes. The repository's rule is that a
/// red that stayed green is DIAGNOSED, not filed away.
#[test]
fn un_accent_pousse_est_annonce_une_seule_fois() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::Accent {
        couleur: "#7aa2f7".into(),
    })
    .expect("deposit");
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.accent_a_annoncer(), Some("#7aa2f7".to_string()));
    assert_eq!(
        source.accent_a_annoncer(),
        None,
        "an announcement does not repeat"
    );
}

/// Two accents arriving between two reads overwrite each other: the accent IS a state,
/// not a history, and the browser would have nothing to do with a tint the
/// icon has already replaced. Same regime as `plein_ecran` and `presse_papier`.
#[test]
fn two_accents_arriving_before_a_read_overwrite_each_other() {
    let (mut source, tx, _recus) = source_with(4);
    tx.send(Recu::Accent {
        couleur: "#7aa2f7".into(),
    })
    .expect("deposit");
    tx.send(Recu::Accent {
        couleur: "#fa8c16".into(),
    })
    .expect("deposit");
    assert_eq!(source.next_frame(), None);
    assert_eq!(
        source.accent_a_annoncer(),
        Some("#fa8c16".to_string()),
        "only the last announcement survives"
    );
}
