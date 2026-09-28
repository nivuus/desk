// The "your peer has arrived" announcement, and the rule that decides whom it goes to.
//
// 🔴 THE SYMMETRIC TWIN OF `peer-gone`, AND THAT IS ITS WHOLE JUSTIFICATION.
// The relay already knew how to say "your peer has left" (`relais.ts`,
// `close` handler) and did not know how to say "your peer has arrived". This
// module adds the missing half of an EXISTING mechanism — it does not
// introduce a second one, which the repository doctrine explicitly forbids.
//
// 🔴 WHAT IT REPAIRS, MEASURED IN PRODUCTION ON 30 AUGUST 2026 (network capture
// on `vnet30`, `tcpdump` + `tshark`, the WebSocket being in clear text on the VM side):
// at t = 12.0 s after its startup, the supervisor announces its windows
// (`fenetre-ouverte` × 3) in a session where NO `client` peer is
// connected yet. `send(peer, …)` is then a silent no-op: the
// announcements are LOST, without a trace. At t = 43.1 s, for lack of a `viewport`
// in return, the agent refuses its own windows
// (`superviseur/table/orphelines.rs`, `DELAI_ATTENTE_VIEWPORT_MAX`). Yet
// the user spends precisely those seconds in the proxy
// authentication: they therefore land on an empty desktop, on a VM full of
// very much alive windows. The only known workaround before this batch was to
// restart the agent WHILE the user is looking at the page.
//
// 🔴 WHAT THIS MESSAGE IS NOT: a BUFFER. The platform memorises
// NO window announcement, on purpose. A window list buffered here
// would be a COPY of a truth that lives in the agent — it would go stale as soon
// as a window closes, and nothing in this service would know when
// to expire it. So yesterday's truth is not replayed: the one holding it
// is told that it is being asked for now. (The only thing this relay
// memorises remains the SDP offer — `Appariement::retenirOffre` —, and it has a
// natural expiry: it is DELIVERED ONCE then forgotten.)

/// The type carried on the wire. Read by `agent/src/superviseur/protocole.rs`
/// (`DepuisLaShell::PairPresent`): changing it is a protocol
/// change, on both sides at once.
///
/// ⚠️ **IT MUST NEVER ENTER `TYPES_RELAYES`** (`relais.ts`): it
/// is EMITTED BY the relay, like `peer-gone`, `ice-config` and `error`. Putting it
/// there would allow a peer to FORGE it towards the other, hence to make
/// the agent re-announce at will — an amplifier offered to anyone with a token.
export const TYPE_PAIR_PRESENT = 'pair-present';

/// Le message complet, tel qu'il part.
export function messagePairPresent(): { type: string } {
    return { type: TYPE_PAIR_PRESENT };
}

/// True if the peer ALREADY IN PLACE must be told of this arrival.
///
/// `roleArrivant` is the role of the peer that has just declared itself; the peer
/// told is therefore the one OPPOSITE.
///
/// 🔴 ONLY THE `agent` ROLE IS TOLD, AND THE ASYMMETRY IS DELIBERATE — it
/// is not an oversight of the reverse direction.
///
/// ① The agent, and it alone, holds a REPLAYABLE state: its window table.
///    The client learns the presence of the agent by receiving its SDP
///    answer; this news would teach it nothing.
/// ② The reverse direction would be MEASURABLE noise. On each window
///    session `w-N`, it is the browser page that connects first and
///    the child that arrives next: telling "the peer opposite" without
///    looking at its role would send this message to ALL session pages,
///    where `client/src/webrtc.ts::parseSignalingMessage` does not recognise it
///    and logs "unreadable or unexpectedly shaped signaling message,
///    ignored". The client therefore IGNORES it without breaking — checking that was the
///    condition for not touching `client/`, outside the scope of this batch —
///    but one trace per window opening is a cost with no counterpart.
///
/// ⚠️ The day a client needs this news, it is this
/// comment that will have to be CONTRADICTED, not silently completed.
export function prevenirLePairEnPlace(roleArrivant: 'agent' | 'client'): boolean {
    return roleArrivant === 'client';
}

/// True if the peer THAT HAS JUST ARRIVED must be told that a peer was already
/// waiting for it.
///
/// 🔴 **THE THIRD CASE, AND IT WAS MISSING — IT IS THE OTHER HALF OF THE SAME
/// MECHANISM, NOT ONE MORE MECHANISM.** `prevenirLePairEnPlace` above
/// answers "the client arrives, the agent is there". This one answers the
/// SYMMETRIC question, which nothing asked: **"the agent arrives, the client is
/// already there"**. Together they state a single, complete rule:
/// *the agent is told as soon as the pairing is complete, whichever
/// side arrived last.*
///
/// 🔴 **WHY THIS CASE EXISTS NOW AND NOT BEFORE.** Until this batch,
/// the agent only opened its control session ONCE, at startup: it
/// was therefore always the first to arrive, and the case could not
/// happen. Since `agent/src/superviseur/signalisation.rs` REOPENS it
/// after a drop, the order flips as soon as the shell page comes back before it
/// — which is the ORDINARY case after a service restart: the
/// user's browser is reloaded by hand within a few seconds,
/// the agent follows an exponential backoff that can reach thirty
/// seconds. Without this rule, the agent would reconnect **without ever
/// learning that someone is waiting for it**, and would re-announce nothing: a
/// perfectly successful reconnection, and an empty desktop nonetheless — yesterday's
/// silent failure, moved one notch.
///
/// ⚠️ **AND THE PEER OPPOSITE IS NECESSARILY A `client`**: the relay
/// only admits one `agent` and one `client` per session
/// (`appariement.ts::declarer`), so when the arriving one is the `agent`, the one
/// waiting for it can only be the `client`. That is what allows this
/// rule to look ONLY at the role of the arriving one, like its neighbour.
///
/// ⚠️ **WHAT THIS MESSAGE COSTS ELSEWHERE, MEASURED BY READING THE CODE RATHER THAN
/// ASSUMED.** It now also goes to the agents of the `w-N` and
/// `files` sessions, where the page usually connects first: these processes
/// read their signaling in `agent/src/signaling.rs`, whose dispatch
/// ends with `other => tracing::debug!(?other, "signaling message
/// ignored")` — a catch-all that logs and **continues**, never a
/// `break`. They therefore ignore it without breaking, at the price of a `debug!` line
/// that is not even emitted under the operations `RUST_LOG=info`.
/// ⚠️ The day this dispatch stops tolerating an unknown type, it is
/// this paragraph that will have to be CONTRADICTED, not discovered.
export function prevenirLArrivant(roleArrivant: 'agent' | 'client'): boolean {
    return roleArrivant === 'agent';
}
