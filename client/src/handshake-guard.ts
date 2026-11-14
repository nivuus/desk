// The guard of a signalling handshake, shared by the session window
// (`webrtc.ts::connectSession`) and the file bridge
// (`files/canal.ts::connectFilesChannel`).
//
// 🔴 WHY ONE LISTENER FOR THE WHOLE HANDSHAKE. Each wait used to watch only
// what it was waiting for: `attendreConfigIce` ignored an `error` message,
// and `waitForAnswer` attached its `close` listener only after candidate
// gathering. A relay that refused the hello (expired token, occupied role)
// and closed was therefore not heard: the page waited out the ICE
// configuration delay, sent its offer on a closed socket, then reported
// "the agent did not answer" fifteen seconds later — never the server's
// reason. The guard below is attached right after the socket opens, records
// the FIRST failure (an `error` message, `peer-gone`, the socket's `error`
// or `close`), and every wait races against it.

import { parseSignalingMessage } from './signaling-message';

/// What the guard needs from a `WebSocket`, and nothing more.
export interface GuardedSocket {
    readonly readyState: number;
    addEventListener(type: 'message', cb: (event: MessageEvent) => void): void;
    addEventListener(type: 'close', cb: (event: CloseEvent) => void): void;
    addEventListener(type: 'error', cb: (event: Event) => void): void;
    removeEventListener(type: 'message', cb: (event: MessageEvent) => void): void;
    removeEventListener(type: 'close', cb: (event: CloseEvent) => void): void;
    removeEventListener(type: 'error', cb: (event: Event) => void): void;
}

/// `WebSocket.OPEN`, as the WHATWG specification numbers it. Written as a
/// value rather than read from the global, which tests replace.
const OPEN_STATE = 1;

export interface HandshakeGuard {
    /// Settles like `pending`, unless the handshake fails first: then rejects
    /// with that failure.
    race<T>(pending: Promise<T>): Promise<T>;
    /// Throws the recorded failure, or a closed-socket failure, before a send.
    beforeSend(): void;
    /// Detaches the guard's listeners. Called once the handshake is over,
    /// whatever its outcome.
    release(): void;
}

export function guardHandshake(socket: GuardedSocket): HandshakeGuard {
    let failure: Error | undefined;
    let rejectFailure: (e: Error) => void = () => {};
    const failed = new Promise<never>((_resolve, reject) => {
        rejectFailure = reject;
    });
    // The shared rejection is always observed through `race`; this
    // handler only keeps an unawaited guard from reporting it as unhandled.
    failed.catch(() => {});

    const record = (e: Error) => {
        if (failure !== undefined) return;
        failure = e;
        rejectFailure(e);
    };

    const onMessage = (event: MessageEvent) => {
        const message = parseSignalingMessage(String(event.data));
        if (message?.type === 'error') {
            record(new Error(message.reason ?? message.motif ?? 'signaling error'));
        } else if (message?.type === 'peer-gone') {
            record(new Error('agent disconnected'));
        }
    };
    const onClose = (event: CloseEvent) => {
        const detail = event.reason ? ` (${event.reason})` : '';
        record(new Error(`connection to the signaling server lost during the handshake${detail}`));
    };
    const onError = () => record(new Error('signaling connection error'));

    socket.addEventListener('message', onMessage);
    socket.addEventListener('close', onClose);
    socket.addEventListener('error', onError);

    return {
        race: (pending) => Promise.race([pending, failed]),
        beforeSend() {
            if (failure !== undefined) throw failure;
            if (socket.readyState !== OPEN_STATE) {
                throw new Error('connection to the signaling server lost during the handshake');
            }
        },
        release() {
            socket.removeEventListener('message', onMessage);
            socket.removeEventListener('close', onClose);
            socket.removeEventListener('error', onError);
        },
    };
}
