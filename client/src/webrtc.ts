// Establishing the WebRTC session. The browser is the offerer: it declares
// the video track as receive-only, the downstream audio track as receive-only,
// the mic's UPSTREAM track as send-only and WITHOUT A TRACK (workstream
// E), and the two data channels; then it waits for the agent's answer
// relayed by signaling.

import { parseAgentControl, type AgentControl } from '../../proto/ts/control';
import { jetonAcces } from './jeton';

export interface SessionOptions {
    signalingUrl: string;
    sessionId: string;
    video: HTMLVideoElement;
    onControl?: (message: AgentControl) => void;
    onStatus?: (message: string) => void;
    /// The access token carried in the handshake (sub-block P2).
    ///
    /// ⚠️ OPTIONAL ON PURPOSE: a required field would force modifying
    /// `main.ts`, the only caller, which another workstream holds. Absent, the
    /// token is read from the browser's vault (`jeton.ts`) — which leaves
    /// ONE SINGLE reader of storage in the whole client, which is better in
    /// itself. The cost is named: this module gains a dependency on a browser
    /// global, whereas it already handles `WebSocket` and
    /// `RTCPeerConnection`; `jeton.ts`, for its part, stays pure.
    jeton?: string;
}

export interface SessionHandle {
    pc: RTCPeerConnection;
    inputChannel: RTCDataChannel;
    controlChannel: RTCDataChannel;
    /// The sender of the UPSTREAM track (workstream E), declared WITHOUT A TRACK.
    ///
    /// It is `client/src/micro.ts` that fills it through `replaceTrack`, on click,
    /// and empties it on switch-off. Exposed here because `connectSession` is the
    /// only place that builds the `RTCPeerConnection`: the sender does not exist
    /// before it, and nothing else can find it without digging through
    /// `pc.getTransceivers()` by position — which would be a positional index,
    /// that is, exactly what this repository has already paid for on DXGI outputs.
    micSender: RTCRtpSender;
    close(): void;
}

// Maximum delay waiting for the agent's answer, after sending the
// offer. If no agent is connected to the requested session, the signaling
// server relays the offer to a non-existent peer and never sends anything
// back to the client: without this delay, the waiting promise would never
// resolve and the user would stay stuck indefinitely.
const ANSWER_TIMEOUT_MS = 15_000;

/// Subset of the signaling messages expected in answer to the offer,
/// discriminated by `type`.
type SignalingMessage =
    | { type: 'answer'; sdp: string }
    | { type: 'error'; reason?: string }
    | { type: 'peer-gone' }
    | { type: 'ice-config'; iceServers: RTCIceServer[] };

/// Parses and validates a raw signaling message.
///
/// `JSON.parse` succeeds on payloads that are not objects:
/// `"null"` gives `null`, but also `"42"` gives a number, `'"x"'` a
/// string, `"[1,2]"` an array. Accessing `.type` on one of these values
/// does not always throw (an array or a string do have an `undefined`
/// `.type`, no exception), but `null.type` throws an uncaught
/// `TypeError` — that is the defect fixed here. We therefore explicitly validate
/// that the result is a non-null, non-array object before any property
/// read, whatever the shape of the payload.
///
/// Returns `undefined` if the message is unreadable or of an unexpected shape:
/// the caller then silently ignores it, without crashing the wait
/// (the awaited message — the SDP answer — may still arrive afterwards).
// Exported only to be unit tested without having to instantiate
// a real WebSocket (see webrtc.test.ts): the rest of the module only uses it
// through `waitForAnswer`, internally.
export function parseSignalingMessage(raw: string): SignalingMessage | undefined {
    let parsed: unknown;
    try {
        parsed = JSON.parse(raw);
    } catch {
        return undefined;
    }
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
        return undefined;
    }
    const { type } = parsed as Record<string, unknown>;
    if (type === 'answer' || type === 'error' || type === 'peer-gone' || type === 'ice-config') {
        return parsed as SignalingMessage;
    }
    return undefined;
}

/// Waits for the agent's answer SDP, relayed by the signaling socket.
/// Rejects if: an error or "peer-gone" message is received, the socket
/// closes before the answer, or the maximum delay is exceeded. An
/// unreadable or unexpectedly shaped message (invalid JSON, non-object value like
/// `null`/a number/an array, or an object without a recognised `type`) is logged
/// and ignored rather than crashing the wait: other valid
/// messages may still arrive, notably the answer itself.
///
/// Exported only to be tested without going through `connectSession`
/// (which requires a complete DOM — `RTCPeerConnection`, a real `WebSocket`, etc.,
/// unavailable under the test's Node runtime): `waitForAnswer` only depends
/// on `addEventListener`/`removeEventListener`, which a minimal fake socket
/// is enough to provide (see webrtc.test.ts).
export function waitForAnswer(socket: WebSocket): Promise<string> {
    return new Promise((resolve, reject) => {
        let settled = false;

        // Whatever the outcome (success, error, closing, timeout), the
        // listeners and the timer must be removed exactly once: no
        // leak, no double resolution/rejection.
        const finish = (action: () => void) => {
            if (settled) return;
            settled = true;
            socket.removeEventListener('message', onMessage);
            socket.removeEventListener('close', onClose);
            clearTimeout(timer);
            action();
        };

        const onMessage = (event: MessageEvent) => {
            const message = parseSignalingMessage(String(event.data));
            if (!message) {
                console.warn('message de signaling illisible ou de forme inattendue, ignoré');
                return;
            }
            if (message.type === 'answer') {
                finish(() => resolve(message.sdp));
            } else if (message.type === 'error') {
                finish(() => reject(new Error(message.reason ?? 'erreur de signaling')));
            } else if (message.type === 'peer-gone') {
                finish(() => reject(new Error('agent déconnecté')));
            }
        };

        const onClose = () => {
            finish(() =>
                reject(new Error("connexion au serveur de signaling perdue avant la réponse de l'agent")),
            );
        };

        const timer = setTimeout(() => {
            // User-oriented message: no internal detail (no
            // mention of the signaling server or of the protocol), just enough
            // to diagnose without reloading the page blindly.
            finish(() =>
                reject(
                    new Error(
                        "l'agent n'a pas répondu — vérifiez qu'il est bien lancé et connecté à cette session",
                    ),
                ),
            );
        }, ANSWER_TIMEOUT_MS);

        socket.addEventListener('message', onMessage);
        socket.addEventListener('close', onClose);
    });
}

/// Waits for ICE gathering to be complete: without trickle, the SDP must already
/// contain all the candidates.
///
/// ⚠️ EXPORTED BY SUB-BLOCK F1, AND IT IS A DECLARED CHANGE.
/// `client/src/fichiers/canal.ts` opens a DEDICATED `RTCPeerConnection`, without
/// media (decision D4 of F1's plan): `connectSession` does not suit it —
/// it requires an `HTMLVideoElement` and unconditionally adds three
/// transceivers. The plan forbids refactoring `connectSession` to make
/// video optional: that would touch the critical path of all
/// windows for a need that has its own function. It does prescribe
/// REUSING the signaling functions "without copying them" — hence this export
/// and that of `attendreConfigIce`. No behaviour is changed.
export function waitForIceGathering(pc: RTCPeerConnection): Promise<void> {
    if (pc.iceGatheringState === 'complete') return Promise.resolve();
    return new Promise((resolve) => {
        const check = () => {
            if (pc.iceGatheringState === 'complete') {
                pc.removeEventListener('icegatheringstatechange', check);
                resolve();
            }
        };
        pc.addEventListener('icegatheringstatechange', check);
        // Safety net: never block indefinitely on a slow network.
        setTimeout(() => {
            pc.removeEventListener('icegatheringstatechange', check);
            resolve();
        }, 3000);
    });
}

/// Waits for the ICE configuration from signaling, at most `delaiMs`.
///
/// Returns an EMPTY array when absent: it is the normal case of a
/// deployment without a relay, not an error. The message is removed from the stream
/// so as not to be mistaken later for an SDP answer.
///
/// ⚠️ EXPORTED BY SUB-BLOCK F1 — see the note on `waitForIceGathering`.
export function attendreConfigIce(socket: WebSocket, delaiMs: number): Promise<RTCIceServer[]> {
    return new Promise((resolve) => {
        const finir = (serveurs: RTCIceServer[]) => {
            socket.removeEventListener('message', onMessage);
            clearTimeout(timer);
            resolve(serveurs);
        };
        const onMessage = (event: MessageEvent) => {
            const message = parseSignalingMessage(String(event.data));
            if (message?.type === 'ice-config') finir(message.iceServers);
        };
        const timer = setTimeout(() => finir([]), delaiMs);
        socket.addEventListener('message', onMessage);
    });
}

export async function connectSession(options: SessionOptions): Promise<SessionHandle> {
    const status = options.onStatus ?? (() => {});

    // The socket opens BEFORE the `RTCPeerConnection`, unlike milestone 1:
    // the ICE servers are only known once the configuration is received from
    // signaling, and `RTCPeerConnection` wants them at construction.
    const socket = new WebSocket(options.signalingUrl);
    await new Promise<void>((resolve, reject) => {
        socket.addEventListener('open', () => resolve(), { once: true });
        socket.addEventListener('error', () => reject(new Error('signaling injoignable')), {
            once: true,
        });
    });
    // 🔴 THE `jeton` FIELD IS ADDED, NONE IS REMOVED — spec §10.2. A
    // service from sub-block P1 only reads `role` and `session` (its relay ignores
    // everything else): this client therefore stays compatible with a service
    // predating the guard. COMPATIBILITY ONLY GOES THIS WAY — a
    // pre-P2 client, for its part, will be refused by a P2 service, and that is
    // precisely the point of the sub-block.
    //
    // ⚠️ NO REDIRECT HERE, and it is not an oversight. Without a token, the
    // session is refused and the refusal is displayed; it is `hub/page.ts`
    // (`shell-page.ts` before the hub became the only surface,
    // August 31st, 2026) that sends back to the sign-in screen, because it is
    // the user's real entry point. A session page is ALWAYS opened by the shell, on
    // the same origin, so the token is already there. Redirecting from a
    // library would give it power over its callers' navigation.
    const jeton = options.jeton ?? jetonAcces();
    socket.send(JSON.stringify({ role: 'client', session: options.sessionId, jeton }));

    // The ICE configuration arrives right after the role declaration, or
    // never if no relay is deployed. We wait for it briefly rather than
    // blocking: a session on a local network must keep being established
    // without a relay, exactly as before this workstream.
    const iceServers = await attendreConfigIce(socket, 2000);
    const pc = new RTCPeerConnection({ iceServers });

    pc.addTransceiver('video', { direction: 'recvonly' });
    // The browser is the offerer: it is the one that must declare the audio
    // track. The agent only answers, provided it has enabled Opus on
    // its `Rtc` builder — otherwise it would answer without an audio track.
    pc.addTransceiver('audio', { direction: 'recvonly' });

    // The mic (workstream E). Declared WITHOUT A TRACK: nothing is captured, no
    // permission is requested, no byte is emitted as long as
    // `client/src/micro.ts` has not called `replaceTrack`. That is what makes
    // "on demand" feasible without renegotiation — `connectSession` makes
    // a SINGLE round trip (offer, then answer) and has NO path for
    // a second offer. `replaceTrack` on an existing sender changes neither
    // the codec nor the m-lines, hence requires no renegotiation.
    //
    // ⚠️ THE ORDER OF THE THREE `addTransceiver` DECIDES THE `mid`s, and the agent
    // depends on it. This transceiver must come AFTER the downstream audio: it then takes
    // `mid:2`, and the agent sees it as `RecvOnly` (str0m inverts the
    // remote direction when accepting the offer) — that is what files its
    // `mid` under `mic_mid` and not under `audio_mid`
    // (`agent/src/transport/evenements.rs`). Swapping the two lines
    // would send the DOWNSTREAM sound onto a track the agent cannot
    // emit, without a single error: it is the latent defect task 7
    // of workstream E exposed then fixed.
    const micTransceiver = pc.addTransceiver('audio', { direction: 'sendonly' });

    // Inputs: unreliable and unordered — a stale mouse position has
    // no value, better to lose it than to delay the following ones.
    const inputChannel = pc.createDataChannel('input', {
        ordered: false,
        maxRetransmits: 0,
    });
    const controlChannel = pc.createDataChannel('control', { ordered: true });

    controlChannel.addEventListener('message', (event) => {
        try {
            options.onControl?.(parseAgentControl(String(event.data)));
        } catch (error) {
            console.warn('message de contrôle invalide', error);
        }
    });

    // A single MediaStream carries both tracks. Reassigning `srcObject` at
    // each received track would make the second chase away the first: the order
    // of arrival is not guaranteed, and the result would be a session sometimes
    // mute, sometimes without an image.
    const flux = new MediaStream();
    pc.addEventListener('track', (event) => {
        flux.addTrack(event.track);
        // Playback latency: ask the browser not to build up
        // a jitter buffer beyond what is strictly necessary.
        //
        // It is not free — on a jittery link, this buffer is what
        // smooths playback, and trimming it trades latency for
        // stutter. Measured under each netem profile in the acceptance run.
        //
        // Chromium only: elsewhere the property does not exist and
        // the assignment has no effect. Hence the defensive access rather than a
        // direct `receiver.playoutDelayHint = 0`, which would throw in strict mode
        // on a sealed object.
        if (event.track.kind === 'video' && 'playoutDelayHint' in event.receiver) {
            (event.receiver as RTCRtpReceiver & { playoutDelayHint: number }).playoutDelayHint = 0;
        }
        if (options.video.srcObject !== flux) {
            options.video.srcObject = flux;
        }
        status(`flux reçu (${flux.getTracks().length} piste(s))`);
    });

    pc.addEventListener('connectionstatechange', () => {
        status(`connexion : ${pc.connectionState}`);
    });

    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    await waitForIceGathering(pc);

    status('offre envoyée, attente de l\'agent…');
    socket.send(JSON.stringify({ type: 'offer', sdp: pc.localDescription!.sdp }));

    const answerSdp = await waitForAnswer(socket);
    await pc.setRemoteDescription({ type: 'answer', sdp: answerSdp });
    status('réponse reçue');

    return {
        pc,
        inputChannel,
        controlChannel,
        micSender: micTransceiver.sender,
        close() {
            // ⚠️ SWITCHING OFF MUST BE REAL (spec §9). `pc.close()` DOES NOT STOP
            // the local tracks: Chrome's mic indicator would stay
            // lit after the end of the session, and the device would stay taken.
            // It is precisely the "visual lie" the spec calls
            // unacceptable on this function. `stop()` is idempotent: that
            // `micro.ts` has already called it costs nothing.
            //
            // ⚠️ DELIBERATE DIVERGENCE FROM THE PLAN, which writes "`close()` calls
            // the mic's `detacher()`". That would make `webrtc.ts` depend on
            // `micro.ts`, which already depends on `SessionHandle.micSender`: a
            // cycle, and a cycle task 10 could not have written anyway,
            // `micro.ts` being born at task 11. We therefore stop the sender's
            // track directly — which is enough for the requirement, `detacher()`
            // doing nothing more than `replaceTrack(null)` and this `stop()`.
            // `main.ts` besides calls its own detachment at the end of the
            // session, so that the button's STATE follows too.
            micTransceiver.sender.track?.stop();
            socket.close();
            pc.close();
        },
    };
}
