//! The bounds of the download: an idle budget on every network wait, and the
//! announced length checked against the order's size before anything is
//! written. Against a real local TCP server, like `tests.rs`, whose helpers
//! these tests share.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::tests::{bac, corps_de, empreinte};
use super::*;

/// Idle budget used by these tests: short enough to keep them fast, long
/// enough for a loopback connection to answer.
const BUDGET: Duration = Duration::from_millis(300);

/// Outer bound of every test: a download that ignores the budget fails the
/// test instead of hanging it.
const GUARD: Duration = Duration::from_secs(10);

/// Reads a request up to the end of its header, and returns it as text.
async fn read_request(socket: &mut TcpStream) -> String {
    let mut raw = Vec::new();
    let mut buffer = [0u8; 4096];
    while !raw.windows(4).any(|f| f == b"\r\n\r\n") {
        let n = socket.read(&mut buffer).await.unwrap_or(0);
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&buffer[..n]);
    }
    String::from_utf8_lossy(&raw).to_string()
}

/// A server that answers each connection with `responses[i]` and, when
/// `hold` is set for it, keeps that connection open and silent afterwards.
async fn server(responses: Vec<(Vec<u8>, bool)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        for (response, hold) in responses {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            read_request(&mut socket).await;
            let _ = socket.write_all(&response).await;
            if hold {
                held.push(socket);
            } else {
                let _ = socket.shutdown().await;
            }
        }
        // Held connections stay open, without a byte, until the test ends.
        tokio::time::sleep(Duration::from_secs(60)).await;
        drop(held);
    });
    format!("http://127.0.0.1:{port}/t/c")
}

fn header(status: u16, length: usize) -> Vec<u8> {
    format!("HTTP/1.1 {status} OK\r\nContent-Length: {length}\r\n\r\n").into_bytes()
}

#[tokio::test]
async fn a_server_that_accepts_and_never_answers_is_refused_within_the_budget() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!(
        "http://127.0.0.1:{}/t/c",
        listener.local_addr().unwrap().port()
    );
    tokio::spawn(async move {
        // Accepts, then never writes nor closes.
        let held_socket = listener.accept().await;
        tokio::time::sleep(Duration::from_secs(60)).await;
        drop(held_socket);
    });
    let b = bac("muet");
    let body = corps_de(100);
    let outcome = tokio::time::timeout(
        GUARD,
        download_with_idle_budget(
            Demande {
                url: &url,
                jeton: "j",
                destination: &b.file,
                expected_size: body.len() as u64,
                sha256_attendu: &empreinte(&body),
            },
            |_, _| {},
            BUDGET,
        ),
    )
    .await
    .expect("the download must give up on a silent server, not hang");
    assert!(matches!(outcome, Err(Refus::Reseau(_))), "{outcome:?}");
}

#[tokio::test]
async fn a_body_that_stalls_is_resumed_like_a_cut() {
    let body = corps_de(10_000);
    let mut first = header(200, body.len());
    first.extend_from_slice(&body[..4_000]);
    let mut resumed = header(206, body.len() - 4_000);
    resumed.extend_from_slice(&body[4_000..]);
    let url = server(vec![(first, true), (resumed, false)]).await;
    let b = bac("fige");
    let written = tokio::time::timeout(
        GUARD,
        download_with_idle_budget(
            Demande {
                url: &url,
                jeton: "j",
                destination: &b.file,
                expected_size: body.len() as u64,
                sha256_attendu: &empreinte(&body),
            },
            |_, _| {},
            BUDGET,
        ),
    )
    .await
    .expect("a stalled body must not hang the download")
    .expect("the resumption must complete the file");
    assert_eq!(written, body.len() as u64);
    assert_eq!(std::fs::read(&b.file).expect("written"), body);
}

#[tokio::test]
async fn an_announced_length_larger_than_the_order_is_refused_before_writing() {
    let body = corps_de(5_000);
    let mut response = header(200, body.len());
    response.extend_from_slice(&body);
    let url = server(vec![(response, false)]).await;
    let b = bac("trop-long");
    let mut most_written = 0u64;
    let refusal = telecharger(
        Demande {
            url: &url,
            jeton: "j",
            destination: &b.file,
            expected_size: 1_000,
            sha256_attendu: &empreinte(&body[..1_000]),
        },
        |faits, _| most_written = most_written.max(faits),
    )
    .await
    .expect_err("must refuse");
    assert_eq!(
        refusal,
        Refus::Size {
            attendue: 1_000,
            obtenue: 5_000
        }
    );
    assert_eq!(
        most_written, 0,
        "nothing may be written past the order's size"
    );
    assert!(!b.file.exists(), "no file must be created");
}

#[tokio::test]
async fn bytes_after_the_announced_length_in_the_first_read_are_not_body() {
    // The header and more than the announced body arrive in one segment.
    let mut response = header(200, 5);
    response.extend_from_slice(b"helloEXTRA");
    let url = server(vec![(response, false)]).await;
    let b = bac("debut");
    let written = telecharger(
        Demande {
            url: &url,
            jeton: "j",
            destination: &b.file,
            expected_size: 5,
            sha256_attendu: &empreinte(b"hello"),
        },
        |_, _| {},
    )
    .await
    .expect("the announced body is complete");
    assert_eq!(written, 5);
    assert_eq!(std::fs::read(&b.file).expect("written"), b"hello");
}
