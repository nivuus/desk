//! TURN client: state machine and encapsulation, without I/O.
//!
//! This module consumes bytes and returns bytes. It owns no
//! socket, does not know `Rtc`, and depends on no Windows API — that is
//! what makes the whole state machine testable on Linux, without coturn.
//!
//! Division of labour with `is::stun`: we READ responses with its parser
//! (which covers all the TURN vocabulary we need), we WRITE the
//! requests ourselves. Its builder does not know `REQUESTED-TRANSPORT`
//! (0x0019), without which a compliant server refuses any allocation with 400.
//!
//! Split into submodules following the protocol's boundaries: `messages`
//! (request serialisation and key derivation), `allocation` (the state
//! machine, from the 401 refusal to lease refresh), `canaux` (STUN/data
//! demultiplexing and ChannelData encapsulation), `fixtures` (the scaffolding
//! the tests of the previous two share). Same reason for being as the split
//! of `congestion` and of `transport`: `CLAUDE.md`'s 500-lines-per-file
//! limit, which a single-piece module would have doubled.
//!
//! Visibility: `messages` is a private module, so what it declares `pub`
//! stays bounded to the `turn` subtree — reachable by its siblings
//! (`allocation`, `canaux`) through `super::messages`, invisible outside. The
//! transport only sees the two re-exports below.

mod allocation;
mod canaux;
#[cfg(test)]
mod fixtures;
mod messages;

pub use allocation::TurnClient;
pub use canaux::est_channel_data;
