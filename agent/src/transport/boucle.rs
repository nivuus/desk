//! The transport loop, and the two drain points that precede it.
//!
//! **Extracted from `transport.rs`** (sub-block F1, task 17), which had crossed
//! 500 lines by gaining the `input_channel` field — the one that lets
//! `evenements::destination` route by CHANNEL rather than by the binary
//! flag alone. It is the second extraction from this file for the same
//! reason: D10 had already moved `initialisation.rs` out of it.
//!
//! The three functions belong together: `run` and `drain_quietly` are the same
//! drain in two regimes — with and without application callbacks —, and
//! `accept_offer` is one of the two mutation points that require the second.

use anyhow::{anyhow, Result};
use proto::control::ClientControl;
use proto::input::InputMessage;
use str0m::Output;

use super::tick::Tick;
use super::Session;

impl Session {
    /// Accepts the browser's offer and produces the SDP answer.
    pub fn accept_offer(&mut self, offer_sdp: &str) -> Result<String> {
        let offer = str0m::change::SdpOffer::from_sdp_string(offer_sdp)
            .map_err(|e| anyhow!("offre SDP illisible : {e}"))?;
        let answer = self
            .rtc
            .sdp_api()
            .accept_offer(offer)
            .map_err(|e| anyhow!("offer refused: {e}"))?;

        // Same reasoning as in `new()`: `accept_offer` mutates `Rtc`, we
        // drain before giving control back, without depending on what the
        // caller will do next.
        self.drain_quietly()?;

        Ok(answer.to_sdp_string())
    }

    /// Transport loop: runs until disconnection or fatal error.
    ///
    /// Blocks on purpose (synchronous UDP read) — to be called from a
    /// dedicated thread (`tokio::task::spawn_blocking`), never from a
    /// tokio async worker (see the module comment, I6 of the review).
    pub fn run(
        &mut self,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Result<()> {
        loop {
            match self
                .rtc
                .poll_output()
                .map_err(|e| anyhow!("poll_output : {e}"))?
            {
                Output::Timeout(deadline) => {
                    if let Tick::Disconnected = self.act_on_timeout(deadline)? {
                        return Ok(());
                    }
                    // 🔴 **RIGHT AFTER `act_on_timeout`, AND IT IS THE SECOND
                    // LINK OF D6's ORDER** (sub-block P2). Branch
                    // `a1octies` may have just written the VM's
                    // clipboard and armed this flag; the `Ctrl+V` injection
                    // therefore cannot precede the write.
                    //
                    // **Here and not in `act_on_timeout`**: that function
                    // receives neither `on_input` nor `on_control`, whereas `run`
                    // receives both. Adding a parameter to it for a single
                    // branch would be a permanent cost on a function that
                    // carries eleven branches and sixty lines of invariant
                    // audit, for zero gain (D-P2-1). Body in
                    // `collage`, next to the write it follows.
                    self.injecter_le_collage(on_input);
                }
                Output::Transmit(transmit) => {
                    // Routes to the direct socket or to the TURN relay depending on
                    // the source str0m indicates. Body in `relais`.
                    self.envoyer(&transmit);
                }
                Output::Event(event) => {
                    if let Tick::Disconnected = self.handle_event(event, on_input, on_control) {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Drains `poll_output` until `Output::Timeout`, without application
    /// callbacks. Used only at the mutation points prior to
    /// `run()` (`new`, `accept_offer`): no track or channel can
    /// produce application data yet at that stage.
    pub(super) fn drain_quietly(&mut self) -> Result<()> {
        loop {
            match self
                .rtc
                .poll_output()
                .map_err(|e| anyhow!("poll_output : {e}"))?
            {
                Output::Timeout(_) => return Ok(()),
                Output::Transmit(transmit) => {
                    // Same single emission point as `run`: a packet emitted
                    // during a drain must go through the relay if that is
                    // the way it must leave.
                    self.envoyer(&transmit);
                }
                Output::Event(event) => {
                    if let Tick::Disconnected = self.handle_event(event, &mut |_| {}, &mut |_| {}) {
                        return Ok(());
                    }
                }
            }
        }
    }
}
