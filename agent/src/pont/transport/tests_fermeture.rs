//! Bridge transport tests: channel closing, and the label that decides which
//! channel is kept, apart from `tests.rs` to stay under 500 lines.

use super::tests::{canal_ouvert, echanger, monter};
use super::*;
use std::sync::mpsc::channel;
use std::time::Instant;
use str0m::channel::ChannelId;
use str0m::{Event, Rtc};

#[test]
fn closing_the_channel_alone_surfaces_channel_closed_and_only_for_the_right_id() {
    // The case where the browser closes ITS data channel without closing the
    // connection — distinct from the next test's close_notify, and served by
    // another arm of `traiter`. Born from a SURVIVING MUTATION: removing the
    // `ChannelClose` arm left all the other tests green, none going through
    // this path.
    //
    // ⚠️ **It is written on SYNTHETIC events, and it is not a
    // convenience shortcut — it is the only way.** Checked in the sources of
    // str0m 0.21 rather than assumed: `DirectApi::close_data_channel` calls
    // `RtcSctp::close_stream`, which only sets `do_close = true` on
    // the LOCAL entry (`sctp/mod.rs:516-520`); the resulting `SctpEvent::Close`
    // event (`:838-841`) is returned to the closing peer, and **nothing is
    // emitted on the wire**. A str0m peer therefore cannot cause an
    // `Event::ChannelClose` on our side. A real browser, for its part, emits an
    // SCTP stream reset that str0m does handle (`:740`, `:790`, `:808`, `:818`
    // set `do_close` from the entry) — the arm is therefore useful in
    // production, and only unreachable from this bench.
    //
    // This repository's precedent for the form: `transport/evenements/tests.rs`
    // hands synthetic `MediaAdded`s to a bare session, to exercise a
    // discrimination without negotiation.
    let mut faux = Rtc::builder().clear_codecs().build(Instant::now());
    let mut api = faux.sdp_api();
    let bon = api.add_channel(FILES_LABEL.to_string());
    let autre = api.add_channel("autre-canal".to_string());

    let (tx, rx) = channel();
    let mut canal: Option<ChannelId> = None;

    assert!(traiter(
        Event::ChannelOpen(bon, FILES_LABEL.to_string()),
        &mut canal,
        &tx
    )
    .is_none());
    assert_eq!(rx.try_recv(), Ok(DuNavigateur::CanalOuvert));
    assert_eq!(canal, Some(bon));

    // A closing that does NOT concern our channel must report NOTHING:
    // without this half, the test would pass on an arm that sends
    // `CanalFerme` at each closing, whatever it is — and the bridge
    // would declare dead a perfectly alive channel.
    assert!(traiter(Event::ChannelClose(autre), &mut canal, &tx).is_none());
    assert!(
        rx.try_recv().is_err(),
        "la fermeture d'un autre canal ne remonte rien"
    );
    assert_eq!(canal, Some(bon), "et elle ne doit pas oublier le nôtre");

    assert!(traiter(Event::ChannelClose(bon), &mut canal, &tx).is_none());
    assert_eq!(rx.try_recv(), Ok(DuNavigateur::CanalFerme));
    assert_eq!(canal, None, "le canal fermé doit être oublié");
}

#[test]
fn un_canal_dont_le_label_n_est_pas_le_notre_n_est_jamais_retenu() {
    // The synthetic counterpart of `un_seul_canal_est_retenu_parmi_deux…`, and it
    // exercises what that one cannot: the REVERSE opening order, where the
    // foreign channel arrives LAST. Without a label filter, it is the one that
    // would overwrite ours.
    let mut faux = Rtc::builder().clear_codecs().build(Instant::now());
    let mut api = faux.sdp_api();
    let bon = api.add_channel(FILES_LABEL.to_string());
    let autre = api.add_channel("autre-canal".to_string());

    let (tx, rx) = channel();
    let mut canal: Option<ChannelId> = None;
    traiter(
        Event::ChannelOpen(bon, FILES_LABEL.to_string()),
        &mut canal,
        &tx,
    );
    let _ = rx.try_recv();
    traiter(
        Event::ChannelOpen(autre, "autre-canal".to_string()),
        &mut canal,
        &tx,
    );

    assert_eq!(
        canal,
        Some(bon),
        "un canal étranger ne doit jamais écraser le nôtre"
    );
    assert!(
        rx.try_recv().is_err(),
        "et il ne doit annoncer aucune ouverture"
    );
}

#[test]
fn la_fermeture_du_canal_remonte_canal_ferme() {
    let (mut pair, _sortant, entrant) = monter(&[FILES_LABEL]);

    // The peer leaves — the tab closes. The loop must SAY so, not
    // end silently: without this message, commands in flight would wait
    // their delay rather than return ERROR_IO_DEVICE right away.
    let mut parti = false;
    let (remontees, _) = echanger(
        &mut pair,
        &entrant,
        |rtc, remontees| {
            if !parti && canal_ouvert(remontees) {
                parti = true;
                // ⚠️ `close()` and NOT `disconnect()` — the difference is the whole
                // test. `disconnect()` only sets `alive = false`
                // LOCALLY, without emitting anything: the bridge would only learn it
                // at ICE expiry, tens of seconds later.
                // `close()` sends the DTLS close_notify, that is, what
                // a browser whose tab is closed does. A first
                // draft used `disconnect()` and failed at the budget —
                // it did not measure what it believed.
                rtc.close().expect("close_notify émis");
            }
        },
        |remontees, _| remontees.contains(&DuNavigateur::CanalFerme),
        "que la fermeture ne remonte",
    );
    assert!(
        remontees.contains(&DuNavigateur::CanalOuvert),
        "le canal doit d'abord s'être ouvert, sinon le test ne mesure rien"
    );
    assert!(remontees.contains(&DuNavigateur::CanalFerme));
}
