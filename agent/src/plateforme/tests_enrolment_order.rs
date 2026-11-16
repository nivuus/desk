//! What is queued before the enrolment completes WAITS for it.
//!
//! Declared from `session.rs`, which owns the behaviour: `tests.rs` sits at
//! its ceiling, and the parent's list of test modules is not the place a
//! session-ordering rule belongs to.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use proto::plateforme::{DepuisLaPlateforme, VersLaPlateforme};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::plateforme::ouvrir;

/// How long the fake platform takes to answer an enrolment. The real one
/// reads its database and derives a `scrypt` digest: tens of milliseconds
/// during which it refuses anything else as out of sequence.
const VERIFICATION: Duration = Duration::from_millis(300);

/// A fake `/agent` channel that answers the enrolment only after
/// `VERIFICATION`, and reports every frame it receives, tagged with whether
/// it arrived BEFORE or AFTER its own `enrole` left.
async fn slow_channel() -> (String, mpsc::UnboundedReceiver<(bool, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("local listener");
    let port = listener.local_addr().expect("local address").port();
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("WebSocket upgrade");
        let answer = tokio::time::sleep(VERIFICATION);
        tokio::pin!(answer);
        let mut enrolled = false;
        loop {
            tokio::select! {
                _ = &mut answer, if !enrolled => {
                    let enrole = DepuisLaPlateforme::enrole("PPP", "token", 1_787_136_774_000);
                    let text = serde_json::to_string(&enrole).expect("serializing the enrolment");
                    ws.send(Message::Text(text)).await.expect("sending the enrolment");
                    enrolled = true;
                }
                received = ws.next() => match received {
                    Some(Ok(Message::Text(text))) => { let _ = tx.send((enrolled, text)); }
                    Some(Ok(_)) => {}
                    _ => return,
                },
            }
        }
    });
    (format!("ws://127.0.0.1:{port}"), rx)
}

#[tokio::test]
async fn a_message_queued_before_the_enrolment_answer_waits_for_it() {
    let (url, mut received) = slow_channel().await;
    let mut canal = ouvrir(&url, "w1".into(), "secret".into());
    // Queued while no session is enrolled — the state of the queue after an
    // outage, when the agent kept emitting with no socket.
    canal
        .emetteur()
        .emettre(VersLaPlateforme::catalogue(true, Vec::new(), Vec::new()));

    canal.attendre_identite().await.expect("enrolment");
    let catalogue = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (after_enrolment, text) = received.recv().await.expect("the channel went silent");
            if text.contains(r#""type":"enroler""#) {
                continue;
            }
            return (after_enrolment, text);
        }
    })
    .await
    .expect("the queued message never reached the platform");

    // The platform refuses anything but an enrolment from a socket it has
    // not enrolled yet: a frame that arrived earlier was refused and lost.
    assert!(
        catalogue.0,
        "the queued message left BEFORE the enrolment was answered: {}",
        catalogue.1
    );
    assert!(
        catalogue.1.contains(r#""type":"catalogue""#),
        "unexpected frame: {}",
        catalogue.1
    );
}
