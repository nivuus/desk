//! The session's safety net for held keys and buttons: what the peer has
//! pressed is tracked as it is dispatched, and released when the browser
//! reports that the window lost focus.
//!
//! The web client sends releases on blur itself; this is the agent-side net
//! for the ones the unordered, unreliable input channel loses. Releases go
//! through `on_input`, the same path as every injected input. When the
//! session ends, the injector's own `Drop` releases what it injected (see
//! `crate::input`), so no release is needed here on that path.

use proto::control::ClientControl;
use proto::input::InputMessage;

use super::Session;

impl Session {
    /// Hands one input message to `on_input`, after noting what it holds.
    pub(super) fn receive_input(
        &mut self,
        message: InputMessage,
        on_input: &mut impl FnMut(InputMessage),
    ) {
        self.held.observer(&message);
        on_input(message);
    }

    /// Releases everything held when `message` says the window lost focus.
    pub(super) fn release_on_focus_loss(
        &mut self,
        message: &ClientControl,
        on_input: &mut impl FnMut(InputMessage),
    ) {
        if !matches!(message, ClientControl::Visibility { focused: false, .. }) {
            return;
        }
        let releases = self.held.release_all();
        if releases.is_empty() {
            return;
        }
        // One line per loss of focus that found something held, never per
        // key: it is the trace that says the net caught a lost release.
        tracing::info!(
            session = %self.session_id,
            releases = releases.len(),
            "focus lost with keys or buttons still held: releasing them"
        );
        for release in releases {
            on_input(release);
        }
    }
}

#[cfg(test)]
#[path = "held_release/tests.rs"]
mod tests;
