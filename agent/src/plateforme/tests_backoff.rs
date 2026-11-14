//! The reconnection backoff of the `/agent` channel across a success then a
//! run of failures — what `tests.rs` does not follow.

use super::*;
use futures_util::{SinkExt, StreamExt};
use proto::plateforme::{DepuisLaPlateforme, MotifCanal};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// A fake `/agent` channel whose FIRST connection enrols and holds for
/// `hold`, and whose every later one refuses the enrolment. Returns its URL
/// and the instant each connection arrived, in the runtime's clock.
async fn enrols_then_refuses(
    hold: Duration,
) -> (String, mpsc::UnboundedReceiver<tokio::time::Instant>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("local listener");
    let port = listener.local_addr().expect("local address").port();
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut first = true;
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let _ = tx.send(tokio::time::Instant::now());
            let enrol = std::mem::replace(&mut first, false);
            tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("WebSocket upgrade");
                let _ = ws.next().await;
                let reply = if enrol {
                    DepuisLaPlateforme::enrole("PREFIXE", "jeton-jwt", 1_787_136_774_000)
                } else {
                    DepuisLaPlateforme::refus(MotifCanal::Enrolement)
                };
                let text = serde_json::to_string(&reply).expect("serializing");
                let _ = ws.send(Message::Text(text)).await;
                tokio::time::sleep(if enrol {
                    hold
                } else {
                    Duration::from_millis(50)
                })
                .await;
                drop(ws);
            });
        }
    });
    (format!("ws://127.0.0.1:{port}"), rx)
}

/// The gaps between the first `n` connections.
async fn gaps(arrivals: &mut mpsc::UnboundedReceiver<tokio::time::Instant>, n: usize) -> Vec<u64> {
    let mut instants = Vec::new();
    for _ in 0..n {
        instants.push(arrivals.recv().await.expect("one more connection"));
    }
    instants
        .windows(2)
        .map(|w| (w[1] - w[0]).as_millis() as u64)
        .collect()
}

#[tokio::test(start_paused = true)]
async fn the_backoff_keeps_growing_after_a_past_enrolment() {
    // One enrolment that lives a few seconds, then nothing but refusals:
    // the platform went away, or the secret was rotated. The refusals must
    // space out up to the ceiling, never hammer at the floor because the
    // agent was once enrolled.
    let (url, mut arrivals) = enrols_then_refuses(Duration::from_secs(2)).await;
    let channel = ouvrir(&url, "vm-1".into(), "chut".into());

    let gaps = gaps(&mut arrivals, 9).await;
    let after_refusals = &gaps[1..];
    assert!(
        after_refusals
            .windows(2)
            .all(|w| w[1] > w[0] || w[1] >= repli::REPLI_MAX_MS),
        "the gaps between refused connections must grow (gaps in ms: {gaps:?})"
    );
    assert!(
        *after_refusals.last().unwrap() >= repli::REPLI_MAX_MS,
        "the ceiling must be reached (gaps in ms: {gaps:?})"
    );
    channel.tache.abort();
}

#[tokio::test(start_paused = true)]
async fn a_session_that_served_rearms_the_backoff() {
    // The negative control: a session that lived well past the useful
    // threshold DID serve, so the next reconnection starts again from the
    // floor — an agent connected for days that loses its network for a
    // second must come back in half a second.
    let (url, mut arrivals) = enrols_then_refuses(Duration::from_secs(120)).await;
    let channel = ouvrir(&url, "vm-1".into(), "chut".into());

    let gaps = gaps(&mut arrivals, 3).await;
    // First gap: the 120 s session plus the floor delay.
    assert!(
        gaps[0] < 120_000 + 2 * repli::REPLI_MIN_MS,
        "after a session that served, the floor delay must apply (gaps in ms: {gaps:?})"
    );
    channel.tache.abort();
}
