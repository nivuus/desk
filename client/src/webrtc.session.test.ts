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
// a single `MediaStream` carrying the received tracks, and the real switch-off
// of the microphone on close (project E). The handshake itself — the token it
// carries and a refusal during it — is in `webrtc.handshake.test.ts`; the fakes
// are in `webrtc.fixtures.ts`. Not the SDP generation itself, out of reach without a real WebRTC
// engine.

import { afterEach, describe, expect, it, vi } from 'vitest';

import { connectSession } from './webrtc';
import {
    fakes,
    FakeMediaStream,
    FakeRtcPeerConnection,
    FakeSignalingSocket,
    fauxVideo,
    type FauxTransceiver,
} from './webrtc.fixtures';

describe('connectSession — negotiation promised by spec §10', () => {
    afterEach(() => {
        fakes.derniereInstancePc = undefined;
        fakes.messagesEnvoyes = [];
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
            jetonFrais: async () => 'j',
        });

        const sdp = fakes.derniereInstancePc!.localDescription!.sdp;
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
            jetonFrais: async () => 'j',
        });

        // `receiver` is always present on a real `RTCTrackEvent` — a
        // bare object here, without `playoutDelayHint`, imitates a browser that does not
        // support the property (see the defensive access in webrtc.ts).
        const pisteVideo = { kind: 'video' } as unknown as MediaStreamTrack;
        const pisteAudio = { kind: 'audio' } as unknown as MediaStreamTrack;
        fakes.derniereInstancePc!.emit('track', { track: pisteVideo, receiver: {} });
        fakes.derniereInstancePc!.emit('track', { track: pisteAudio, receiver: {} });

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
            jetonFrais: async () => 'j',
        });

        // The ORDER is the crux of the test, not a detail of form: it is what
        // decides the `mid`, and the agent files the audio track under `audio_mid` or
        // under `mic_mid` depending on its direction (`transport/evenements.rs`).
        // Swapping the two audio transceivers would send the DOWNSTREAM sound
        // onto a track that cannot emit, without a single error.
        expect(fakes.derniereInstancePc!.transceivers.map((t) => [t.kind, t.direction])).toEqual([
            ['video', 'recvonly'],
            ['audio', 'recvonly'],
            ['audio', 'sendonly'],
        ]);

        const lignes = fakes.derniereInstancePc!.localDescription!.sdp.split('\r\n');
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
            jetonFrais: async () => 'j',
        });

        // It is the sender of the THIRD transceiver, not of another: without this
        // identity equality, `micro.ts` would fill the downstream track.
        expect(session.micSender).toBe(fakes.derniereInstancePc!.transceivers[2].sender);
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
            jetonFrais: async () => 'j',
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
