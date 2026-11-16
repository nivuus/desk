// Liveness of the WebSocket peers: a protocol-level ping, and a deadline.
//
// A TCP connection whose peer vanished — a cut link, a machine put to sleep, a
// network card reset — stays ESTABLISHED on our side for as long as nothing is
// written to it: no `close` event ever fires. On `/signal` such a socket kept
// its role, so the peer's own reconnection was answered "role already taken";
// on `/agent` it kept its VM "reachable". Nothing in the service could tell a
// silent peer from a dead one.
//
// This is the pattern the `ws` library documents for exactly this case
// ("How to detect and close broken connections?", README of ws 8.21): ping
// every peer on an interval, and `terminate()` the ones that did not answer
// by the next round. `terminate()` and not `close()`: a close handshake
// needs the peer's answer, which is what is missing. The socket's ordinary
// `close` handler then runs, and the caller's cleanup with it — this module
// knows neither roles nor VMs.
//
// A browser answers pings by itself, as RFC 6455 §5.5.2 requires, and so does
// `tokio-tungstenite` on the agent side: no peer has anything to implement.

import type { WebSocket, WebSocketServer } from 'ws';
import { ligne } from '../obs/journal';

/// The interval between two sweeps, which is also the answer deadline: a dead
/// peer is detected after one period at best, two at worst.
///
/// 30 s at worst is the same order as the agent's reconnection backoff
/// ceiling (`agent/src/plateforme/repli.rs::REPLI_MAX_MS`): a role held by a
/// dead socket is released within about one retry of its owner.
export const LIVENESS_PERIOD_MS = 15_000;

/// Starts watching every socket of `wss`, until the server closes.
///
/// `route` only names the server in the log line.
export function watchLiveness(wss: WebSocketServer, route: string): void {
    /// The sockets pinged at the last sweep that have not answered since.
    const unanswered = new WeakSet<WebSocket>();

    wss.on('connection', (socket: WebSocket) => {
        socket.on('pong', () => unanswered.delete(socket));
    });

    const sweep = setInterval(() => {
        for (const socket of wss.clients) {
            if (unanswered.has(socket)) {
                // One line per dead peer, never one per ping: its volume is
                // bounded by the connections, which are themselves braked.
                console.warn(ligne('dead-peer', { route, deadline_ms: LIVENESS_PERIOD_MS }));
                socket.terminate();
                continue;
            }
            unanswered.add(socket);
            // The callback takes the send error: a socket already closing
            // reports it there, and the next sweep terminates it.
            socket.ping(() => {});
        }
    }, LIVENESS_PERIOD_MS);
    // The sweep must never be what keeps the process alive.
    sweep.unref();
    wss.on('close', () => clearInterval(sweep));
}
