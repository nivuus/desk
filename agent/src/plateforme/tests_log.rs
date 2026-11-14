//! What the `/agent` channel writes to the log about a frame it cannot
//! read: never the frame itself, which can carry the agent's bearer token.

use std::io::Write;
use std::sync::{Arc, Mutex};

use super::*;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// A `MakeWriter` that keeps everything the subscriber writes.
#[derive(Clone, Default)]
struct CapturedLog(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedLog {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedLog {
    type Writer = CapturedLog;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl CapturedLog {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

/// A fake `/agent` channel that answers the enrolment with `frame`, raw.
async fn answers_raw(frame: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("local listener");
    let port = listener.local_addr().expect("local address").port();
    tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("WebSocket upgrade");
        let _ = ws.next().await;
        let _ = ws.send(Message::Text(frame)).await;
        std::future::pending::<()>().await;
    });
    format!("ws://127.0.0.1:{port}")
}

const TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.SECRET-AGENT-TOKEN.signature";

#[tokio::test]
async fn an_unreadable_enrolment_never_logs_its_token() {
    // An `enrole` from a platform one version ahead fails `check_version`
    // and lands in the "unreadable message" arm, token included.
    let frame = format!(
        r#"{{"type":"enrole","v":{},"prefixe":"P","jeton":"{TOKEN}","expire_a":1}}"#,
        proto::plateforme::PLATEFORME_VERSION + 1
    );
    let url = answers_raw(frame).await;
    let captured = CapturedLog::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(captured.clone())
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let channel = ouvrir(&url, "vm-1".into(), "chut".into());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !captured
        .text()
        .contains("unreadable message from the platform")
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the unreadable frame was never reported, the test would prove nothing \
             (log: {})",
            captured.text()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    channel.tache.abort();

    let text = captured.text();
    assert!(
        !text.contains("SECRET-AGENT-TOKEN"),
        "the agent token must never reach the log (log: {text})"
    );
    assert!(
        text.contains(r#"frame_type=Some("enrole")"#),
        "the frame's type must still be logged, to diagnose it (log: {text})"
    );
}
