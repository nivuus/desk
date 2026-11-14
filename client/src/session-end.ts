// The single end of a session window.
//
// 🔴 WHY ONE ROUTINE. The end-of-session message detached the modules but
// never closed the session, and nothing at all reacted to the connection
// failing or to the peer leaving after setup: the microphone kept capturing,
// the gamepad's 4 ms timer kept running, the page kept a dead connection, and
// nothing told the user. Every way a session can end now goes through
// `end`, which runs once: it calls every registered clean-up (the
// modules `main.ts` attached), closes the session — which stops the
// microphone's track, `webrtc.ts::close` — and shows a terminal status.
//
// ⚠️ NO AUTOMATIC RECONNECTION, and that is not an oversight: whether and how a
// window reconnects is the owner's decision. The terminal status says what
// happened; reopening the window from the hub is the user's gesture.
//
// PURE OF THE DOM: what it needs from the session and the status banner is a
// parameter, so the whole routine is testable in Node.

import type { TonTerminal } from './ecran-terminal';

/// What the routine needs from an established session (`SessionHandle`).
export interface WatchedSession {
    pc: {
        readonly connectionState: RTCPeerConnectionState;
        addEventListener(type: 'connectionstatechange', cb: () => void): void;
    };
    /// Registers a callback for the relay saying the agent left.
    onPeerGone(callback: () => void): void;
    close(): void;
}

export interface SessionEnd {
    /// Registers a clean-up to run at the end, in registration order.
    add(cleanup: () => void): void;
    /// Ends the session when its connection fails or closes, or its peer leaves.
    watch(session: WatchedSession): void;
    /// Ends the session now. Runs once; later calls do nothing.
    end(text: string, ton: TonTerminal): void;
}

/// `failed` and `closed` are final states of a peer connection; `disconnected`
/// is transient — ICE may recover from it on its own — and does not end the
/// session.
const FINAL_STATES: readonly RTCPeerConnectionState[] = ['failed', 'closed'];

export function createSessionEnd(show: (text: string, ton: TonTerminal) => void): SessionEnd {
    const cleanups: (() => void)[] = [];
    let session: WatchedSession | undefined;
    let ended = false;

    const sessionEnd: SessionEnd = {
        add(cleanup) {
            cleanups.push(cleanup);
        },
        watch(watched) {
            session = watched;
            // A session that ended before it was known (an end-of-session
            // message raced the setup) is closed as soon as it is.
            if (ended) {
                watched.close();
                return;
            }
            watched.pc.addEventListener('connectionstatechange', () => {
                const state = watched.pc.connectionState;
                if (FINAL_STATES.includes(state)) sessionEnd.end(`connection ${state}`, 'danger');
            });
            watched.onPeerGone(() =>
                sessionEnd.end('the remote machine left the session', 'danger'),
            );
        },
        end(text, ton) {
            if (ended) return;
            ended = true;
            for (const cleanup of cleanups) cleanup();
            session?.close();
            show(text, ton);
        },
    };
    return sessionEnd;
}
