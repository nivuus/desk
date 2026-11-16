import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocket, WebSocketServer } from 'ws';
import { LIVENESS_PERIOD_MS, watchLiveness } from './liveness';

/// Real sockets on the loopback, and ONLY the sweep's `setInterval` faked: the
/// frames (ping, pong, close) travel for real, the test only decides WHEN a
/// sweep runs.
let wss: WebSocketServer | undefined;
const clients: WebSocket[] = [];

afterEach(async () => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    for (const client of clients.splice(0)) client.terminate();
    if (wss) {
        for (const socket of wss.clients) socket.terminate();
        await new Promise<void>((r) => wss!.close(() => r()));
        wss = undefined;
    }
});

async function start(): Promise<number> {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
    await new Promise<void>((r) => wss!.once('listening', () => r()));
    watchLiveness(wss, '/test');
    const address = wss.address();
    return typeof address === 'object' && address ? address.port : 0;
}

/// A peer of the real `ws` client. `answers: false` is the dead peer: its TCP
/// connection stays established and it never answers a ping — what a machine
/// behind a cut link looks like from the server.
function open(port: number, answers: boolean): Promise<WebSocket> {
    return new Promise((resolve, reject) => {
        const client = new WebSocket(`ws://127.0.0.1:${port}`, { autoPong: answers });
        clients.push(client);
        client.once('open', () => resolve(client));
        client.once('error', reject);
    });
}

function closed(socket: WebSocket): Promise<void> {
    return new Promise((r) => socket.once('close', () => r()));
}

/// Real time, for the frames to cross the loopback. Not faked: only the
/// interval is.
const settle = (ms = 50): Promise<void> => new Promise((r) => setTimeout(r, ms));

async function serverSocket(): Promise<WebSocket> {
    for (let i = 0; i < 40 && wss!.clients.size === 0; i++) await settle(5);
    const [socket] = [...wss!.clients];
    if (!socket) throw new Error('the server never saw the connection');
    return socket;
}

describe('watchLiveness', () => {
    it('terminates a peer that did not answer the ping by the next sweep', async () => {
        const port = await start();
        const client = await open(port, false);
        const server = await serverSocket();
        const serverClosed = closed(server);
        const pinged = new Promise<void>((r) => client.once('ping', () => r()));

        vi.advanceTimersByTime(LIVENESS_PERIOD_MS);
        await pinged;
        await settle();
        // One unanswered ping is not a verdict yet: the deadline is the NEXT sweep.
        expect(server.readyState).toBe(WebSocket.OPEN);

        vi.advanceTimersByTime(LIVENESS_PERIOD_MS);
        await serverClosed;
        expect(wss!.clients.size).toBe(0);
    });

    it('keeps a peer that answers, sweep after sweep', async () => {
        const port = await start();
        const client = await open(port, true);
        const server = await serverSocket();
        let pings = 0;
        client.on('ping', () => pings++);

        for (let sweep = 0; sweep < 4; sweep++) {
            vi.advanceTimersByTime(LIVENESS_PERIOD_MS);
            await settle();
        }
        expect(pings).toBe(4);
        expect(server.readyState).toBe(WebSocket.OPEN);
        expect(client.readyState).toBe(WebSocket.OPEN);
    });

    it('judges each peer on its own answer', async () => {
        const port = await start();
        const alive = await open(port, true);
        const dead = await open(port, false);
        const deadClosed = closed(dead);

        vi.advanceTimersByTime(LIVENESS_PERIOD_MS);
        await settle();
        vi.advanceTimersByTime(LIVENESS_PERIOD_MS);
        await deadClosed;
        await settle();
        expect(alive.readyState).toBe(WebSocket.OPEN);
        expect(wss!.clients.size).toBe(1);
    });

    it('stops sweeping when the server closes', async () => {
        await start();
        expect(vi.getTimerCount()).toBe(1);
        await new Promise<void>((r) => wss!.close(() => r()));
        wss = undefined;
        expect(vi.getTimerCount()).toBe(0);
    });
});
