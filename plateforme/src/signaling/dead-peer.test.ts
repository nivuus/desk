import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { WebSocket } from 'ws';
import { LIVENESS_PERIOD_MS } from '../http/liveness';
import type { Garde } from '../identite/garde';
import { Frein } from '../securite/frein';
import { createSignalingServer } from './relais';
import { poserTurnAmbiant } from './turn-harnais';

/// The relay facing a peer that DIED WITHOUT CLOSING — a cut link, a laptop put
/// to sleep. Real sockets; only the liveness sweep's `setInterval` is faked,
/// so the test decides when a sweep runs and nothing else.
///
/// An open guard, as in `server.test.ts`: authentication has its own file.
const OPEN_GUARD: Garde = {
    verify: () => ({ ok: true }),
    revendiquer: () => {},
    liberer: () => {},
};

let restoreTurn: () => void;
beforeAll(() => {
    restoreTurn = poserTurnAmbiant();
});
afterAll(() => restoreTurn());

let server: ReturnType<typeof createSignalingServer>;
const peers: WebSocket[] = [];

beforeEach(() => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    server = createSignalingServer(0, OPEN_GUARD, new Frein(), new Set());
});

afterEach(async () => {
    for (const peer of peers.splice(0)) peer.terminate();
    await server.close();
    vi.useRealTimers();
    vi.restoreAllMocks();
});

interface Peer {
    socket: WebSocket;
    messages: Record<string, unknown>[];
    closed: Promise<{ code: number; reason: string }>;
}

/// Opens a peer and declares it. `answers: false` never answers a ping while
/// its TCP connection stays established: the dead peer, as the server sees it.
function declare(role: 'agent' | 'client', session: string, answers = true): Promise<Peer> {
    return new Promise((resolve, reject) => {
        const socket = new WebSocket(`ws://127.0.0.1:${server.port}`, { autoPong: answers });
        peers.push(socket);
        const peer: Peer = {
            socket,
            messages: [],
            closed: new Promise((r) =>
                socket.once('close', (code, reason) => r({ code, reason: reason.toString() })),
            ),
        };
        socket.on('message', (raw) => peer.messages.push(JSON.parse(raw.toString())));
        socket.once('error', reject);
        socket.once('open', () => {
            socket.send(JSON.stringify({ role, session }));
            resolve(peer);
        });
    });
}

const settle = (ms = 50): Promise<void> => new Promise((r) => setTimeout(r, ms));

/// Two sweeps: the first pings, the second finds the ping unanswered.
async function twoSweeps(): Promise<void> {
    vi.advanceTimersByTime(LIVENESS_PERIOD_MS);
    await settle();
    vi.advanceTimersByTime(LIVENESS_PERIOD_MS);
    await settle();
}

describe('a peer that died without closing', () => {
    it('loses its role: the reconnection is accepted after the liveness deadline', async () => {
        const dead = await declare('agent', 'bureau', false);
        await settle();

        // The reconnection arrives while the dead socket still holds the role.
        const early = await declare('agent', 'bureau');
        const refusal = await early.closed;
        expect(early.messages).toEqual([
            {
                type: 'error',
                reason: 'an agent is already connected to session bureau',
                motif: 'role-occupe',
            },
        ]);
        expect(refusal).toEqual({ code: 1008, reason: 'role-occupe' });

        await twoSweeps();
        await dead.closed;

        // The role is free again: the next attempt holds it, and a client that
        // arrives is paired with THIS agent.
        const retry = await declare('agent', 'bureau');
        await settle();
        expect(retry.messages).toEqual([]);
        expect(retry.socket.readyState).toBe(WebSocket.OPEN);
        await declare('client', 'bureau');
        await settle();
        expect(retry.messages).toEqual([{ type: 'pair-present' }]);
    });

    it('tells its peer: the other side receives `peer-gone`', async () => {
        const agent = await declare('agent', 'w-1');
        await declare('client', 'w-1', false);
        await settle();
        agent.messages.length = 0;

        await twoSweeps();

        expect(agent.messages).toEqual([{ type: 'peer-gone' }]);
        expect(agent.socket.readyState).toBe(WebSocket.OPEN);
    });

    it('a living peer keeps its role through the sweeps', async () => {
        const agent = await declare('agent', 'bureau');
        await twoSweeps();
        await twoSweeps();

        expect(agent.socket.readyState).toBe(WebSocket.OPEN);
        const second = await declare('agent', 'bureau');
        await second.closed;
        expect(second.messages[0]?.motif).toBe('role-occupe');
    });
});

describe('a refused role', () => {
    it('closes the refused socket, after the typed refusal', async () => {
        await declare('client', 'bureau');
        await settle();
        const second = await declare('client', 'bureau');

        const closing = await second.closed;

        expect(second.messages).toEqual([
            {
                type: 'error',
                reason: 'a client is already connected to session bureau',
                motif: 'role-occupe',
            },
        ]);
        expect(closing).toEqual({ code: 1008, reason: 'role-occupe' });
    });

    it('does not disturb the peer that holds the role', async () => {
        const holder = await declare('client', 'bureau');
        const agent = await declare('agent', 'bureau');
        await settle();
        const second = await declare('client', 'bureau');
        await second.closed;
        await settle();

        expect(holder.socket.readyState).toBe(WebSocket.OPEN);
        // The refused socket never entered the table: its close must not
        // announce a departure to the agent.
        expect(agent.messages.filter((m) => m.type === 'peer-gone')).toEqual([]);
    });
});
