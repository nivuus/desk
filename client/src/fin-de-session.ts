// The single end of a session window.
//
// 🔴 WHY ONE ROUTINE. The end-of-session message detached the modules but
// never closed the session, and nothing at all reacted to the connection
// failing or to the peer leaving after setup: the microphone kept capturing,
// the gamepad's 4 ms timer kept running, the page kept a dead connection, and
// nothing told the user. Every way a session can end now goes through
// `terminer`, which runs once: it calls every registered clean-up (the
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
export interface SessionSurveillee {
    pc: {
        readonly connectionState: RTCPeerConnectionState;
        addEventListener(type: 'connectionstatechange', cb: () => void): void;
    };
    /// Registers a callback for the relay saying the agent left.
    surPairParti(rappel: () => void): void;
    close(): void;
}

export interface FinDeSession {
    /// Registers a clean-up to run at the end, in registration order.
    ajouter(nettoyage: () => void): void;
    /// Ends the session when its connection fails or closes, or its peer leaves.
    surveiller(session: SessionSurveillee): void;
    /// Ends the session now. Runs once; later calls do nothing.
    terminer(texte: string, ton: TonTerminal): void;
}

/// `failed` and `closed` are final states of a peer connection; `disconnected`
/// is transient — ICE may recover from it on its own — and does not end the
/// session.
const ETATS_FINAUX: readonly RTCPeerConnectionState[] = ['failed', 'closed'];

export function creerFinDeSession(afficher: (texte: string, ton: TonTerminal) => void): FinDeSession {
    const nettoyages: (() => void)[] = [];
    let session: SessionSurveillee | undefined;
    let terminee = false;

    const fin: FinDeSession = {
        ajouter(nettoyage) {
            nettoyages.push(nettoyage);
        },
        surveiller(surveillee) {
            session = surveillee;
            // A session that ended before it was known (an end-of-session
            // message raced the setup) is closed as soon as it is.
            if (terminee) {
                surveillee.close();
                return;
            }
            surveillee.pc.addEventListener('connectionstatechange', () => {
                const etat = surveillee.pc.connectionState;
                if (ETATS_FINAUX.includes(etat)) fin.terminer(`connection ${etat}`, 'danger');
            });
            surveillee.surPairParti(() =>
                fin.terminer('the remote machine left the session', 'danger'),
            );
        },
        terminer(texte, ton) {
            if (terminee) return;
            terminee = true;
            for (const nettoyage of nettoyages) nettoyage();
            session?.close();
            afficher(texte, ton);
        },
    };
    return fin;
}
