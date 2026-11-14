// The fake browser `connectSession` runs on in the tests: `RTCPeerConnection`,
// `WebSocket` and `MediaStream`, plus what they record.
//
// Extracted from `webrtc.session.test.ts` when the handshake tests moved to
// `webrtc.poignee.test.ts`: both files drive the same fakes. A fixture file, like `micro.fixtures.ts`: no test of its own.

import { vi } from 'vitest';

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
export interface FauxTransceiver {
    kind: string;
    direction?: string;
    sender: { track: { stop(): void } | null };
}

export class FakeRtcPeerConnection {
    transceivers: FauxTransceiver[] = [];
    iceGatheringState = 'complete';
    connectionState = 'new';
    localDescription: { type: string; sdp: string } | null = null;
    private listeners = new Map<string, Set<(event: any) => void>>();

    constructor(_config: unknown) {
        etat.derniereInstancePc = this;
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

    /// Set by `close()`: what `fin-de-session.test.ts` checks.
    fermee = false;

    close(): void {
        this.fermee = true;
    }
}

/// Fake `MediaStream`: just enough to prove that received tracks
/// accumulate in the same object rather than chasing each other out.
export class FakeMediaStream {
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
export class FakeSignalingSocket {
    private listeners = new Map<string, Set<(event: any) => void>>();
    /// `WebSocket.readyState`: CONNECTING (0), then OPEN (1), CLOSED (3).
    readyState = 0;

    constructor(_url: string) {
        etat.dernierSocket = this;
        queueMicrotask(() => {
            this.readyState = 1;
            this.emit('open', {});
        });
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
        etat.messagesEnvoyes.push(data);
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

    close(): void {
        this.readyState = 3;
        etat.socketsFermees += 1;
    }
}

/// What the fakes record, shared with the test files that use them.
export const etat: {
    derniereInstancePc: FakeRtcPeerConnection | undefined;
    /// Everything the client pushed on the signaling socket. It is the only
    /// instrument that can say what the HANDSHAKE carries.
    messagesEnvoyes: string[];
    /// How many fake signalling sockets were closed.
    socketsFermees: number;
    /// The last fake signalling socket built.
    dernierSocket: FakeSignalingSocket | undefined;
} = { derniereInstancePc: undefined, messagesEnvoyes: [], socketsFermees: 0, dernierSocket: undefined };

/// The role declaration, that is, the first message sent.
export function poigneeDeMain(): Record<string, unknown> {
    return JSON.parse(etat.messagesEnvoyes[0]) as Record<string, unknown>;
}

export function fauxVideo(): HTMLVideoElement {
    return { srcObject: null } as unknown as HTMLVideoElement;
}

/// Installs the three fakes as browser globals.
export function armerLesFactices(): void {
    vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
    vi.stubGlobal('WebSocket', FakeSignalingSocket);
    vi.stubGlobal('MediaStream', FakeMediaStream);
}
