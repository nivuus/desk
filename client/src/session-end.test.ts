// A session window that fails or loses its peer is torn down: every module
// detached, the microphone stopped, the session closed, the user told.

import { afterEach, describe, expect, it, vi } from 'vitest';

import { attachGamepad } from './gamepad';
import { createSessionEnd } from './session-end';
import { connectSession } from './webrtc';
import {
    armerLesFactices,
    fakes,
    FakeSignalingSocket,
    fauxVideo,
    type FauxTransceiver,
} from './webrtc.fixtures';

afterEach(() => {
    fakes.derniereInstancePc = undefined;
    fakes.messagesEnvoyes = [];
    fakes.closedSockets = 0;
    fakes.lastSocket = undefined;
    vi.unstubAllGlobals();
});

/// A session established on the fake browser, its microphone ON, with the
/// gamepad module attached on a fake timer — the 4 ms timer `main.ts` runs.
async function sessionWithMicAndGamepad() {
    armerLesFactices();
    const session = await connectSession({
        signalingUrl: 'ws://signaling.invalid',
        sessionId: 'test',
        video: fauxVideo(),
        jetonFrais: async () => 'j',
    });
    const micro = { arrete: false };
    (session.micSender as unknown as FauxTransceiver['sender']).track = {
        stop() {
            micro.arrete = true;
        },
    };
    const minuteur = { poses: 0, annules: [] as number[] };
    const manette = attachGamepad({
        manettes: { get: () => [] },
        minuteur: {
            poser: () => {
                minuteur.poses += 1;
                return 42;
            },
            annuler: (id: number) => void minuteur.annules.push(id),
        },
        horloge: () => 0,
        envoyer: () => {},
    });
    const affiches: [string, string][] = [];
    const sessionEnd = createSessionEnd((texte, ton) => void affiches.push([texte, ton]));
    sessionEnd.add(() => manette.detacher());
    sessionEnd.watch(session);
    return { session, micro, minuteur, affiches, pc: fakes.derniereInstancePc! };
}

describe('the end of a session window', () => {
    it('a connection moving to the failed state tears everything down', async () => {
        const { micro, minuteur, affiches, pc } = await sessionWithMicAndGamepad();

        pc.connectionState = 'failed';
        pc.emit('connectionstatechange', {});

        expect(micro.arrete).toBe(true);
        expect(minuteur.annules).toEqual([42]);
        expect(pc.closed).toBe(true);
        expect(fakes.closedSockets).toBe(1);
        expect(affiches).toHaveLength(1);
        expect(affiches[0][1]).toBe('danger');
    });

    it('the peer leaving tears everything down', async () => {
        const { micro, minuteur, affiches, pc } = await sessionWithMicAndGamepad();
        const socket = fakes.lastSocket;
        expect(socket).toBeDefined();

        socket!.emit('message', { data: JSON.stringify({ type: 'peer-gone' }) });

        expect(micro.arrete).toBe(true);
        expect(minuteur.annules).toEqual([42]);
        expect(pc.closed).toBe(true);
        expect(affiches).toHaveLength(1);
    });

    it('a transient disconnection does not end the session', async () => {
        const { micro, affiches, pc } = await sessionWithMicAndGamepad();

        pc.connectionState = 'disconnected';
        pc.emit('connectionstatechange', {});

        expect(micro.arrete).toBe(false);
        expect(pc.closed).toBe(false);
        expect(affiches).toEqual([]);
    });

    it('ends once, whatever ends it first', async () => {
        const { minuteur, affiches, pc } = await sessionWithMicAndGamepad();

        pc.connectionState = 'failed';
        pc.emit('connectionstatechange', {});
        // Closing the connection moves it to `closed`: the end must not run again.
        pc.connectionState = 'closed';
        pc.emit('connectionstatechange', {});

        expect(minuteur.annules).toEqual([42]);
        expect(affiches).toHaveLength(1);
    });
});

describe('a handshake that fails after the connection is created', () => {
    it('closes the connection and the socket', async () => {
        armerLesFactices();
        // A relay that refuses the OFFER: the connection exists by then.
        class OfferRefusingRelay extends FakeSignalingSocket {
            override send(data: string): void {
                const message = JSON.parse(data) as { type?: string; role?: string };
                if (message.role === 'client') {
                    queueMicrotask(() =>
                        this.emit('message', { data: JSON.stringify({ type: 'ice-config', iceServers: [] }) }),
                    );
                }
                if (message.type === 'offer') {
                    queueMicrotask(() =>
                        this.emit('message', { data: JSON.stringify({ type: 'error', reason: 'no agent' }) }),
                    );
                }
            }
        }
        vi.stubGlobal('WebSocket', OfferRefusingRelay);

        await expect(
            connectSession({
                signalingUrl: 'ws://signaling.invalid',
                sessionId: 'test',
                video: fauxVideo(),
                jetonFrais: async () => 'j',
            }),
        ).rejects.toThrow(/no agent/);

        expect(fakes.derniereInstancePc!.closed).toBe(true);
        expect(fakes.closedSockets).toBe(1);
    });
});
