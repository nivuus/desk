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

import { parseSignalingMessage } from './signalisation';

/// What the guard needs from a `WebSocket`, and nothing more.
export interface SocketGarde {
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
const OUVERTE = 1;

export interface GardePoigneeDeMain {
    /// Settles like `attente`, unless the handshake fails first: then rejects
    /// with that failure.
    course<T>(attente: Promise<T>): Promise<T>;
    /// Throws the recorded failure, or a closed-socket failure, before a send.
    avantEnvoi(): void;
    /// Detaches the guard's listeners. Called once the handshake is over,
    /// whatever its outcome.
    liberer(): void;
}

export function garderLaPoigneeDeMain(socket: SocketGarde): GardePoigneeDeMain {
    let echec: Error | undefined;
    let rejeter: (e: Error) => void = () => {};
    const echoue = new Promise<never>((_resolve, reject) => {
        rejeter = reject;
    });
    // The shared rejection is always observed through `course`; this
    // handler only keeps an unawaited guard from reporting it as unhandled.
    echoue.catch(() => {});

    const noter = (e: Error) => {
        if (echec !== undefined) return;
        echec = e;
        rejeter(e);
    };

    const surMessage = (event: MessageEvent) => {
        const message = parseSignalingMessage(String(event.data));
        if (message?.type === 'error') {
            noter(new Error(message.reason ?? message.motif ?? 'signaling error'));
        } else if (message?.type === 'peer-gone') {
            noter(new Error('agent disconnected'));
        }
    };
    const surFermeture = (event: CloseEvent) => {
        const detail = event.reason ? ` (${event.reason})` : '';
        noter(new Error(`connection to the signaling server lost during the handshake${detail}`));
    };
    const surErreur = () => noter(new Error('signaling connection error'));

    socket.addEventListener('message', surMessage);
    socket.addEventListener('close', surFermeture);
    socket.addEventListener('error', surErreur);

    return {
        course: (attente) => Promise.race([attente, echoue]),
        avantEnvoi() {
            if (echec !== undefined) throw echec;
            if (socket.readyState !== OUVERTE) {
                throw new Error('connection to the signaling server lost during the handshake');
            }
        },
        liberer() {
            socket.removeEventListener('message', surMessage);
            socket.removeEventListener('close', surFermeture);
            socket.removeEventListener('error', surErreur);
        },
    };
}
