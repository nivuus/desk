// The signalling handshake of `connectSession`: the FRESH token the hello
// carries (sub-block P2), and what a refusal during the handshake produces.
//
// Moved out of `webrtc.session.test.ts`, which had crossed 500 lines; the
// fakes live in `webrtc.fixtures.ts`.

import { afterEach, describe, expect, it, vi } from 'vitest';

import { connectSession } from './webrtc';
import { CLE_ACCES, CLE_RAFRAICHISSEMENT, sansVerrou } from './jeton';
import { fournisseurDeJetonFrais } from './jeton-frais';
import {
    armerLesFactices,
    etat,
    FakeMediaStream,
    FakeRtcPeerConnection,
    FakeSignalingSocket,
    fauxVideo,
    poigneeDeMain,
} from './webrtc.fixtures';

describe('the handshake carries a FRESH token (sub-block P2)', () => {
    afterEach(() => {
        etat.messagesEnvoyes = [];
        vi.unstubAllGlobals();
    });

    /// A token whose `exp` is `expMs` (the browser checks no signature).
    function jetonExpirantA(expMs: number): string {
        return `x.${btoa(JSON.stringify({ exp: expMs })).replace(/=+$/, '')}.y`;
    }

    it('sends the token the provider returns', async () => {
        armerLesFactices();
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
            jetonFrais: async () => 'jeton-de-l-appelant',
        });
        expect(poigneeDeMain()).toEqual({
            role: 'client',
            session: 'test',
            jeton: 'jeton-de-l-appelant',
        });
    });

    it('with an expired stored token and a valid refresh token, the hello carries a fresh token', async () => {
        armerLesFactices();
        const maintenant = 1_000_000;
        const perime = jetonExpirantA(maintenant - 1);
        const neuf = jetonExpirantA(maintenant + 600_000);
        const coffre = new Map<string, string>([
            [CLE_ACCES, perime],
            [CLE_RAFRAICHISSEMENT, 'rafraichissement-valide'],
        ]);
        const vault = {
            getItem: (c: string) => coffre.get(c) ?? null,
            setItem: (c: string, v: string) => void coffre.set(c, v),
            removeItem: (c: string) => void coffre.delete(c),
        };
        // The raw store is the global one too: a client that read it would
        // send the expired token.
        vi.stubGlobal('localStorage', vault);
        const urls: string[] = [];
        const fetch = async (url: string) => {
            urls.push(url);
            return {
                ok: true,
                status: 200,
                json: async () => ({ acces: neuf, rafraichissement: 'rafraichissement-suivant' }),
            };
        };

        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
            jetonFrais: fournisseurDeJetonFrais({
                coffre: vault,
                base: 'https://h',
                fetch,
                maintenant: () => maintenant,
                verrou: sansVerrou,
            }),
        });

        expect(urls).toEqual(['https://h/auth/rafraichir']);
        expect(poigneeDeMain().jeton).toBe(neuf);
    });

    it('without a fresh token, no socket is opened and the failure says so', async () => {
        armerLesFactices();
        let sockets = 0;
        vi.stubGlobal(
            'WebSocket',
            class extends FakeSignalingSocket {
                constructor(url: string) {
                    super(url);
                    sockets += 1;
                }
            },
        );
        await expect(
            connectSession({
                signalingUrl: 'ws://signaling.invalid',
                sessionId: 'test',
                video: fauxVideo(),
                jetonFrais: async () => undefined,
            }),
        ).rejects.toThrow(/session has expired/);
        expect(sockets).toBe(0);
    });

    it("adds NO field other than `jeton`, and removes none", async () => {
        // 🔴 Spec §10.2: the field is ADDED, none is removed. A service
        // of sub-block P1 only reads `role` and `session` and ignores `jeton`,
        // so this client stays compatible with it.
        armerLesFactices();
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
            jetonFrais: async () => 'j',
        });
        expect(Object.keys(poigneeDeMain()).sort()).toEqual(['jeton', 'role', 'session']);
    });
});

describe('a refusal during the handshake', () => {
    afterEach(() => {
        etat.messagesEnvoyes = [];
        vi.useRealTimers();
        vi.unstubAllGlobals();
    });

    /// A relay that refuses the hello: it sends an `error` carrying its
    /// reason, then closes, and never sends the ICE configuration.
    class RelaisQuiRefuse extends FakeSignalingSocket {
        override send(data: string): void {
            etat.messagesEnvoyes.push(data);
            if ((JSON.parse(data) as { role?: string }).role !== 'client') return;
            queueMicrotask(() => {
                this.emit('message', {
                    data: JSON.stringify({ type: 'error', reason: 'token expired', motif: 'jeton-expire' }),
                });
                this.readyState = 3;
                this.emit('close', { code: 1008, reason: 'jeton-expire' });
            });
        }
    }

    it("rejects at once with the server's reason, before any configuration wait elapses", async () => {
        vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', RelaisQuiRefuse);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        const issue = connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
            jetonFrais: async () => 'j',
        }).then(
            () => 'resolved',
            (e: unknown) => (e instanceof Error ? e.message : String(e)),
        );
        let observee = 'pending';
        void issue.then((v) => (observee = v));
        // Ten milliseconds: far less than the 2 s ICE configuration wait and
        // the 15 s answer wait the refusal used to sit behind.
        await vi.advanceTimersByTimeAsync(10);

        expect(observee).toMatch(/token expired/);
        // The offer is never sent on a socket the server has closed.
        expect(etat.messagesEnvoyes.some((m) => m.includes('"offer"'))).toBe(false);
    });
});
