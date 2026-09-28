//! What the platform asks to INSTALL, and why it does not go through
//! [`super::Ordre`].
//!
//! 🔴 A SECOND QUEUE, AND NOT A THIRD VARIANT OF `Ordre`: IT IS THE
//! CONSUMER THAT DECIDES, NOT TASTE.
//!
//! `Ordre` is drained by the discovery loop, which runs on **a real
//! dedicated COM thread** (`apps.rs`: "the COM apartment belongs to ITS thread;
//! `IShellLinkW` and `ShellExecuteExW` must both run on the one that
//! called `CoInitializeEx`"). An installation, for its part, DOWNLOADS several
//! hundred megabytes in `tokio` then waits for a process for
//! minutes. Putting it in `Ordre` would make the COM thread do this work, which
//! would then stop reconciling — that is, the catalogue would freeze
//! **during exactly the installation it is expected to report on**.
//!
//! ⚠️ **THE HEADER OF `ordre.rs` ARGUES AGAINST TWO QUEUES, and it is right
//! about the risk it names**: "two queues would force querying both at
//! each round, with the risk that a future addition forgets one". That is true
//! of a single consumer — and there is not one here, there are two. The
//! risk is handled otherwise: this queue has **only one message type**,
//! hence nothing to forget; and `Ordre` stays exhaustive for its own
//! consumer, exactly as G2 wrote it.
//!
//! ⚠️ **Its sentence "the TWO downstream messages" became false** — there
//! are three since sub-block G3. Annotated in its place.
//!
//! ⚠️ Decision D11 of G3's plan argued against "a widened tuple":
//! `ordres()` then returned a `(String, String)`. G2 replaced it with an enum
//! in the meantime. **The premise changed; the conclusion holds for another
//! reason**, the one written above.

/// The order to install an uploaded piece of software.
///
/// ⚠️ IT CARRIES NO BYTE, only a **URL**: the channel is in JSON,
/// it carries the heartbeat, and an 8 MiB slice would cost +33 % in
/// base64 there while blocking this heartbeat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    pub id: String,
    pub url: String,
    pub nom: String,
    pub size: u64,
    pub sha256: String,
}
