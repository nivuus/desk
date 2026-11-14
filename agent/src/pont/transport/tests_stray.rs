//! Bridge transport tests: datagrams that are not STUN, DTLS or RTP must not
//! end the loop. Apart from `tests.rs` to stay under 500 lines.

use std::net::UdpSocket;

use super::tests::{canal_ouvert, echanger, monter};
use super::*;

#[test]
fn stray_datagrams_do_not_end_the_bridge_loop() {
    // The bridge's socket has no input filter: anything that can route to it
    // can send it a datagram. An empty one and one whose first byte matches
    // no protocol str0m multiplexes are both rejected by
    // `DatagramRecv::try_from`; the loop must drop them and keep serving.
    let (mut pair, _sortant, entrant) = monter(&[FILES_LABEL]);
    let stray = UdpSocket::bind("127.0.0.1:0").expect("stray socket");
    let answer = proto::files::encoder(proto::files::TYPE_DATA, 0x5EED_0001, "{}", &[9, 9]);

    let channel = pair.canaux[0];
    let target = pair.bridge_address;
    let to_send = answer.clone();
    let (mut strays_sent, mut sent) = (false, false);
    let (surfaced, _) = echanger(
        &mut pair,
        &entrant,
        |rtc, surfaced| {
            if !canal_ouvert(surfaced) {
                return;
            }
            if !strays_sent {
                strays_sent = true;
                stray.send_to(&[], target).expect("empty datagram sent");
                stray
                    .send_to(&[0xFF; 32], target)
                    .expect("unknown datagram sent");
                // Let the bridge's reader take both before the legitimate
                // frame, so that the answer below proves the loop SURVIVED
                // them rather than overtook them.
                std::thread::sleep(std::time::Duration::from_millis(50));
                return;
            }
            if !sent {
                if let Some(mut c) = rtc.channel(channel) {
                    sent = c.write(true, &to_send).is_ok();
                }
            }
        },
        |surfaced, _| {
            surfaced
                .iter()
                .any(|m| matches!(m, DuNavigateur::Reponse { .. }))
        },
        "the answer sent after two stray datagrams",
    );
    assert!(
        surfaced.contains(&DuNavigateur::Reponse {
            correlation: 0x5EED_0001,
            trame: answer,
        }),
        "the bridge must still deliver answers after stray datagrams"
    );
}
