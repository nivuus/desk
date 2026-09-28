// The listener that keeps an oversized frame from bringing the service down.
//
// 🔴 EXTRACTED FROM `serveur.ts` ON 25 AUGUST 2026 (fix round 1, the
// « legacy without a VM » work), TO MAKE ROOM IN IT — without changing a line of
// behaviour. `serveur.ts` was at 494/500; wiring the background cleanup
// of the stores (`apps/nettoyage.ts`) would have pushed it past. The repository
// rule is EXTRACT, never compress.

import type { WebSocketServer } from 'ws';

/// 🔴 WITHOUT THIS FUNCTION, `TRAME_MAX_OCTETS` (`serveur.ts`) GIVES A DENIAL OF
/// SERVICE WORSE THAN THE ONE IT CLOSES, and this is no guess:
/// MEASURED on 20 August 2026 on the real entry point, `connect ECONNREFUSED`
/// — THE PROCESS WAS DEAD, killed by ONE SINGLE ANONYMOUS FRAME.
///
/// THE CHAIN, in three links each of them mundane: `ws` refuses a frame
/// beyond `maxPayload` and EMITS `error` on the server socket; no
/// server socket of this service had an `error` listener (checked:
/// `grep -n "on('error'" relais.ts canal.ts serveur.ts` only returned the
/// `http.once('error', reject)` of the startup); and an `EventEmitter` that emits
/// `error` without a listener THROWS. The exception then crosses a Node
/// event handler, which has nobody to catch it — the exact failure mode
/// that `signaling/relais.ts` and `signaling/trace.ts` both
/// document, reached here through a new door.
///
/// ⚠️ NO TEST « INSIDE » VITEST COULD SEE IT: vitest installs its
/// own handler for uncaught exceptions, so that the tests of
/// `http/serveur.test.ts` stayed GREEN while the real service was dying
/// (they merely reported « Vitest caught N unhandled errors »). The proof
/// therefore lives in `signaling/resilience.test.ts`, which launches `index.ts` as a
/// real child process — that is precisely the reason that file exists,
/// and its header said so before P5.
///
/// ⚠️ IT LOGS NOTHING, AND THAT IS A REASONED CHOICE, NOT NEGLIGENCE.
/// `CLAUDE.md` has carried the rule since the TURN work: « never trace per
/// packet in the transport loop — count or sample, never
/// trace per packet », after a trace per `Transmit` wrote 18 619
/// lines in a few seconds and destroyed the measurement it served. One line
/// per faulty socket would make the service an amplifier again: an
/// attacker opening N sockets would get N lines written, on the very path that
/// `TRAME_MAX_OCTETS` has just closed.
///
/// ⚠️ THE COST IS NAMED: a socket error is therefore INVISIBLE to
/// the operator. What stays observable is the CLOSE, which the peer sees
/// (code 1009), and the fact that the service keeps serving. The day they
/// need to be counted, a counter is what will be needed — not a trace.
export function absorbSocketErrors(wss: WebSocketServer): void {
    // Registered BEFORE `createSignalingServer` and `servirLeCanalAgent`, which
    // set their own `connection` handlers: the listeners run
    // in their order of registration, and this one must be attached to the
    // socket before anything else talks to it.
    wss.on('connection', (socket) => {
        socket.on('error', () => {
            // Deliberately empty — see above. The only thing that matters
            // is that a listener EXISTS: it is that, and that alone, which keeps
            // `EventEmitter` from throwing.
        });
    });
}
