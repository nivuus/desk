// End-to-end tests of `connectSession`, on fake implementations
// of `RTCPeerConnection`, `WebSocket` and `MediaStream`.
//
// ⚠️ **EXTRACTED from `webrtc.test.ts` by task 10 of project E**, which had
// taken it to 517 lines — above the repository's ceiling of 500. The rule there is
// explicit: "no NEW file is born above 500 lines, and a
// file already above must not grow further", and the prescribed remedy
// is EXTRACTION, never compressing comments. The cut follows
// a real boundary and not a line count: `webrtc.test.ts` keeps what
// is exercised WITHOUT a browser (`parseSignalingMessage`, `waitForAnswer`, on
// a minimal fake socket), this file takes everything that requires simulating a
// whole browser. It is this second half that grew, and will keep growing.
//
// What these tests carry: what `connectSession` ASKS of the browser —
// the transceivers and their ORDER (which decides the `mid`, on which the agent depends),
// a single `MediaStream` carrying the received tracks, the token in the
// handshake (sub-block P2), and the real switch-off of the microphone on close
// (project E). Not the SDP generation itself, out of reach without a real WebRTC
// engine.

import { afterEach, describe, expect, it, vi } from 'vitest';

import { connectSession } from './webrtc';
import { CLE_ACCES } from './jeton';

// These two tests cover a behaviour promised by the spec (§10) but
// never written: "the offer contains an `m=audio` in `recvonly`" and "two
// received tracks end up in a single `MediaStream`". The rest of the
// file isolates `parseSignalingMessage`/`waitForAnswer` precisely to
// avoid having to run a real `RTCPeerConnection`/`WebSocket`
// under Node; here, we go all the way through fake global
// implementations rather than leaving the hole open. The tested point is what
// `connectSession` ASKS of the browser (an `audio` transceiver in
// `recvonly`, a single `MediaStream` carrying the two received tracks) — not the
// SDP generation itself, out of reach without a real WebRTC engine.

/// Fake `RTCPeerConnection` entry point. `createOffer` faithfully translates
/// the requested transceivers into SDP lines: it is this
/// translation, faithful to the request, that the tests check.
/// What an `addTransceiver` returns, reduced to what `connectSession` uses.
interface FauxTransceiver {
    kind: string;
    direction?: string;
    sender: { track: { stop(): void } | null };
}

class FakeRtcPeerConnection {
    transceivers: FauxTransceiver[] = [];
    iceGatheringState = 'complete';
    connectionState = 'new';
    localDescription: { type: string; sdp: string } | null = null;
    private listeners = new Map<string, Set<(event: any) => void>>();

    constructor(_config: unknown) {
        derniereInstancePc = this;
    }

    addTransceiver(kind: string, opts?: { direction?: string }): FauxTransceiver {
        // The fake now RETURNS a transceiver, like the real one: it is the
        // `sender` of the microphone's one that `connectSession` exposes (project E).
        // A `void` here made the five tests of `connectSession` fail on
        // "Cannot read properties of undefined (reading 'sender')".
        const transceiver: FauxTransceiver = {
            kind,
            direction: opts?.direction,
            // `track: null` is the state of a transceiver declared WITHOUT A TRACK —
            // exactly what spec §5 requires of the microphone: no capture,
            // no permission requested as long as nobody has clicked.
            sender: { track: null },
        };
        this.transceivers.push(transceiver);
        return transceiver;
    }

    createDataChannel(label: string, _opts?: unknown) {
        return {
            label,
            readyState: 'open',
            addEventListener() {},
            removeEventListener() {},
            send() {},
        };
    }

    addEventListener(type: string, cb: (event: any) => void): void {
        if (!this.listeners.has(type)) this.listeners.set(type, new Set());
        this.listeners.get(type)!.add(cb);
    }

    removeEventListener(type: string, cb: (event: any) => void): void {
        this.listeners.get(type)?.delete(cb);
    }

    emit(type: string, event: unknown): void {
        for (const cb of [...(this.listeners.get(type) ?? [])]) cb(event);
    }

    async createOffer(): Promise<{ type: 'offer'; sdp: string }> {
        const lignes = ['v=0'];
        for (const { kind, direction } of this.transceivers) {
            lignes.push(`m=${kind} 9 UDP/TLS/RTP/SAVPF 0`);
            lignes.push(`a=${direction ?? 'sendrecv'}`);
        }
        return { type: 'offer', sdp: lignes.join('\r\n') };
    }

    async setLocalDescription(desc: { type: string; sdp: string }): Promise<void> {
        this.localDescription = desc;
    }

    async setRemoteDescription(_desc: unknown): Promise<void> {}

    close(): void {}
}

/// Fake `MediaStream`: just enough to prove that received tracks
/// accumulate in the same object rather than chasing each other out.
class FakeMediaStream {
    private tracks: unknown[] = [];
    addTrack(track: unknown): void {
        this.tracks.push(track);
    }
    getTracks(): unknown[] {
        return this.tracks;
    }
}

/// Fake `WebSocket` that opens right away and automatically answers
/// "answer" as soon as it sees an offer go by, so that `connectSession`
/// can go all the way without ever touching a real network.
class FakeSignalingSocket {
    private listeners = new Map<string, Set<(event: any) => void>>();

    constructor(_url: string) {
        queueMicrotask(() => this.emit('open', {}));
    }

    addEventListener(type: string, cb: (event: any) => void): void {
        if (!this.listeners.has(type)) this.listeners.set(type, new Set());
        this.listeners.get(type)!.add(cb);
    }

    removeEventListener(type: string, cb: (event: any) => void): void {
        this.listeners.get(type)?.delete(cb);
    }

    emit(type: string, event: unknown): void {
        for (const cb of [...(this.listeners.get(type) ?? [])]) cb(event);
    }

    send(data: string): void {
        messagesEnvoyes.push(data);
        const parsed = JSON.parse(data) as { type?: string; role?: string };
        // Empty ICE configuration, emitted as soon as the role is declared, as
        // the signaling server does when no relay is deployed.
        // Without it, `connectSession` would wait 2 s (the delay
        // of `attendreConfigIce`) before building the connection.
        if (parsed.role === 'client') {
            queueMicrotask(() => {
                this.emit('message', {
                    data: JSON.stringify({ type: 'ice-config', iceServers: [] }),
                });
            });
        }
        if (parsed.type === 'offer') {
            queueMicrotask(() => {
                this.emit('message', {
                    data: JSON.stringify({ type: 'answer', sdp: 'v=0\r\n' }),
                });
            });
        }
    }

    close(): void {}
}

let derniereInstancePc: FakeRtcPeerConnection | undefined;
/// Everything the client pushed on the signaling socket. It is the only
/// instrument that can say what the HANDSHAKE carries.
let messagesEnvoyes: string[] = [];

/// The role declaration, that is, the first message sent.
function poigneeDeMain(): Record<string, unknown> {
    return JSON.parse(messagesEnvoyes[0]) as Record<string, unknown>;
}

function fauxVideo(): HTMLVideoElement {
    return { srcObject: null } as unknown as HTMLVideoElement;
}

describe('connectSession — negotiation promised by spec §10', () => {
    afterEach(() => {
        derniereInstancePc = undefined;
        messagesEnvoyes = [];
        vi.unstubAllGlobals();
    });

    it("the sent offer contains an `m=audio` as `recvonly`", async () => {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });

        const sdp = derniereInstancePc!.localDescription!.sdp;
        const lignes = sdp.split('\r\n');
        const indexAudio = lignes.indexOf('m=audio 9 UDP/TLS/RTP/SAVPF 0');
        expect(indexAudio).toBeGreaterThanOrEqual(0);
        expect(lignes[indexAudio + 1]).toBe('a=recvonly');
    });

    it('two received tracks end up in a single MediaStream', async () => {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        const video = fauxVideo();
        // The session is awaited BEFORE emitting the tracks: since the
        // ICE configuration must be received to build the connection, the
        // `RTCPeerConnection` is born after the first `await` of
        // `connectSession`, and the fake instance therefore does not exist yet when
        // the call returns. The `track` listener is wired right after its
        // construction, well before resolution — emitting here reaches it
        // as surely as before.
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video,
        });

        // `receiver` is always present on a real `RTCTrackEvent` — a
        // bare object here, without `playoutDelayHint`, imitates a browser that does not
        // support the property (see the defensive access in webrtc.ts).
        const pisteVideo = { kind: 'video' } as unknown as MediaStreamTrack;
        const pisteAudio = { kind: 'audio' } as unknown as MediaStreamTrack;
        derniereInstancePc!.emit('track', { track: pisteVideo, receiver: {} });
        derniereInstancePc!.emit('track', { track: pisteAudio, receiver: {} });

        const flux = video.srcObject as unknown as FakeMediaStream;
        expect(flux).toBeInstanceOf(FakeMediaStream);
        expect(flux.getTracks()).toEqual([pisteVideo, pisteAudio]);

        // The second track must not have chased the first out by
        // reassigning `srcObject`: same `flux` object before and after.
        expect(video.srcObject).toBe(flux);
    });

    // ── The microphone (project E, task 10) ─────────────────────────────────
    //
    // ⚠️ THE PLAN DECLARES THESE THREE TESTS IMPOSSIBLE, AND IT IS WRONG. It writes
    // that "`connectSession` requires a real `RTCPeerConnection` and is therefore
    // not tested here — it is already the choice of `webrtc.test.ts`, which only exercises
    // `parseSignalingMessage` and `waitForAnswer`". That is no longer true since
    // the two negotiation tests above, and since the three token
    // tests (sub-block P2): five tests go through `connectSession` end to
    // end on a fake `pc` whose `createOffer` FAITHFULLY TRANSLATES the
    // requested transceivers into SDP lines.
    //
    // The plan then ruled out "a test that checks that `addTransceiver` was
    // called", on the grounds that it could only fail by deleting the
    // line. The criticism holds for a test that counted calls; it does not
    // hold for these, which are about the ORDER of the m-lines and about
    // switching off — two properties that can be broken without deleting anything, and
    // each of which was seen falling under mutation (see the task report).
    it("the offer declares a THIRD m-line, `audio` as `sendonly`, AFTER the downstream audio", async () => {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });

        // The ORDER is the crux of the test, not a detail of form: it is what
        // decides the `mid`, and the agent files the audio track under `audio_mid` or
        // under `mic_mid` depending on its direction (`transport/evenements.rs`).
        // Swapping the two audio transceivers would send the DOWNSTREAM sound
        // onto a track that cannot emit, without a single error.
        expect(derniereInstancePc!.transceivers.map((t) => [t.kind, t.direction])).toEqual([
            ['video', 'recvonly'],
            ['audio', 'recvonly'],
            ['audio', 'sendonly'],
        ]);

        const lignes = derniereInstancePc!.localDescription!.sdp.split('\r\n');
        const audios = lignes.flatMap((l, i) => (l.startsWith('m=audio ') ? [i] : []));
        expect(audios).toHaveLength(2);
        expect(lignes[audios[0] + 1]).toBe('a=recvonly');
        expect(lignes[audios[1] + 1]).toBe('a=sendonly');
    });

    it('the mic sender is exposed, and it is born WITHOUT A TRACK', async () => {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        const session = await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });

        // It is the sender of the THIRD transceiver, not of another: without this
        // identity equality, `micro.ts` would fill the downstream track.
        expect(session.micSender).toBe(derniereInstancePc!.transceivers[2].sender);
        expect(session.micSender.track).toBeNull();
    });

    it("`close()` STOPS the mic track, and not only the connection", async () => {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        const session = await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });

        // What `micro.ts` will have done on click: `replaceTrack` sets the track
        // on the sender.
        let arretee = false;
        (session.micSender as unknown as FauxTransceiver['sender']).track = {
            stop() {
                arretee = true;
            },
        };

        session.close();

        // ⚠️ `pc.close()` DOES NOT STOP local tracks: without the explicit
        // `stop()`, Chrome's microphone indicator would stay on after the
        // end of the session and the device would stay taken. It is the "visual
        // lie" spec §9 calls unacceptable on this feature.
        expect(arretee).toBe(true);
    });
});

describe('the handshake carries the token (sub-block P2)', () => {
    afterEach(() => {
        messagesEnvoyes = [];
        vi.unstubAllGlobals();
    });

    function armerLesFactices(): void {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);
    }

    it('sends the token passed as an option', async () => {
        armerLesFactices();
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
            jeton: 'jeton-de-l-appelant',
        });
        expect(poigneeDeMain()).toEqual({
            role: 'client',
            session: 'test',
            jeton: 'jeton-de-l-appelant',
        });
    });

    it('falls back to the browser store when no token is passed', async () => {
        // E7: the field is OPTIONAL so that `main.ts` — modified by another
        // project — does not have to move. The fallback is therefore the
        // NOMINAL path, not an emergency case, and it must be exercised as such.
        armerLesFactices();
        vi.stubGlobal('localStorage', {
            getItem: (c: string) => (c === CLE_ACCES ? 'jeton-du-coffre' : null),
            setItem: () => {},
            removeItem: () => {},
        });
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });
        expect(poigneeDeMain().jeton).toBe('jeton-du-coffre');
    });

    it("adds NO field other than `jeton`, and removes none", async () => {
        // 🔴 Spec §10.2: the field is ADDED, none is removed. A service
        // of sub-block P1 only reads `role` and `session` and ignores `jeton`,
        // so this client stays compatible with it. Compatibility only goes
        // in that direction, and it is this test that guards the first half.
        armerLesFactices();
        vi.stubGlobal('localStorage', {
            getItem: () => null,
            setItem: () => {},
            removeItem: () => {},
        });
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });
        // Without a token: the handshake from before P2, identical.
        expect(Object.keys(poigneeDeMain()).sort()).toEqual(['role', 'session']);

        messagesEnvoyes = [];
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
            jeton: 'j',
        });
        // With a token: EXACTLY one more field.
        expect(Object.keys(poigneeDeMain()).sort()).toEqual(['jeton', 'role', 'session']);
    });
});
