import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocket, WebSocketServer } from 'ws';
import { encodeEnroler, encodeLancer } from '../../../proto/ts/plateforme';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { LIVENESS_PERIOD_MS } from '../http/liveness';
import { Frein } from '../securite/frein';
import { servirLeCanalAgent } from './canal';
import { enrolerUneVm, ouvrir, SECRET, SECRET_VM, T0 } from './canal-harnais';
import { RegistreAgents } from './registre';

/// The `/agent` channel facing sockets that die: one that stops answering, and
/// one that dies WHILE its enrolment is still being verified. Real sockets and
/// a real database; only the liveness sweep's `setInterval` is faked.
let base: Pilote | undefined;
let wss: WebSocketServer | undefined;
const raw: WebSocket[] = [];

afterEach(async () => {
    for (const socket of raw.splice(0)) socket.terminate();
    if (wss) {
        for (const socket of wss.clients) socket.terminate();
        await new Promise<void>((r) => wss!.close(() => r()));
        wss = undefined;
    }
    await base?.fermer();
    base = undefined;
    vi.useRealTimers();
    vi.restoreAllMocks();
});

async function start(p: Pilote, registre: RegistreAgents): Promise<number> {
    wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
    await new Promise<void>((r) => wss!.once('listening', () => r()));
    servirLeCanalAgent(wss, {
        base: p,
        secretJeton: SECRET,
        maintenant: () => T0,
        registre,
        frein: new Frein(),
        proxyDeConfiance: new Set(),
    });
    const address = wss.address();
    return typeof address === 'object' && address ? address.port : 0;
}

const settle = (ms = 50): Promise<void> => new Promise((r) => setTimeout(r, ms));

/// A driver whose FIRST read waits for the test: it is the read
/// `verifyEnrolment` starts with, so the first enrolment stays pending for as
/// long as the test decides — the window the real `scrypt` leaves open,
/// widened instead of raced.
function gateTheFirstRead(real: Pilote): { pilote: Pilote; started: Promise<void>; release: () => void } {
    let release = (): void => {};
    const gate = new Promise<void>((r) => {
        release = r;
    });
    let signalStarted = (): void => {};
    const started = new Promise<void>((r) => {
        signalStarted = r;
    });
    let first = true;
    const pilote: Pilote = {
        executer: (sql, params) => real.executer(sql, params),
        async interroger<T>(sql: string, params: unknown[]): Promise<T[]> {
            if (first) {
                first = false;
                signalStarted();
                await gate;
            }
            return real.interroger<T>(sql, params);
        },
        transaction: (corps) => real.transaction(corps),
        fermer: () => real.fermer(),
    };
    return { pilote, started, release };
}

describe('an agent that died without closing', () => {
    it('leaves the registry at the liveness deadline: the VM stops being reachable', async () => {
        vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
        vi.spyOn(console, 'warn').mockImplementation(() => {});
        base = await baseNeuve('canal-dead-agent');
        await enrolerUneVm(base, 'v-1');
        const registre = new RegistreAgents();
        const port = await start(base, registre);

        // A peer that never answers a ping while its TCP connection holds.
        const dead = new WebSocket(`ws://127.0.0.1:${port}`, { autoPong: false });
        raw.push(dead);
        const enrolled = new Promise<void>((r) => dead.once('message', () => r()));
        dead.once('open', () => dead.send(encodeEnroler('v-1', SECRET_VM)));
        await enrolled;
        expect(registre.pousser('v-1', encodeLancer('d-1', 'k'))).toBe(true);

        vi.advanceTimersByTime(LIVENESS_PERIOD_MS);
        await settle();
        vi.advanceTimersByTime(LIVENESS_PERIOD_MS);
        await settle();

        expect(registre.pousser('v-1', encodeLancer('d-2', 'k'))).toBe(false);
    });
});

describe('an enrolment that resolves after its socket died', () => {
    it('registers nothing, and leaves the live socket of the VM in place', async () => {
        base = await baseNeuve('canal-late-enrolment');
        await enrolerUneVm(base, 'v-1');
        const registre = new RegistreAgents();
        const gated = gateTheFirstRead(base);
        const port = await start(gated.pilote, registre);

        // The first attempt: its enrolment is being verified when it dies.
        const first = new WebSocket(`ws://127.0.0.1:${port}`);
        raw.push(first);
        await new Promise<void>((r) => first.once('open', () => r()));
        first.send(encodeEnroler('v-1', SECRET_VM));
        await gated.started;
        first.terminate();
        for (let i = 0; i < 40 && wss!.clients.size > 0; i++) await settle(5);
        expect(wss!.clients.size).toBe(0);

        // The agent reconnects and enrols: this socket is the live one.
        const live = await ouvrir(port);
        const answer = await live.dire(encodeEnroler('v-1', SECRET_VM));
        expect(answer.type).toBe('enrole');

        // The first verification resolves only now, for a socket long dead.
        gated.release();
        await settle(300);

        expect(live.ordre).toEqual(['message']);
        expect(live.socket.readyState).toBe(WebSocket.OPEN);
        expect(registre.pousser('v-1', encodeLancer('d-1', 'k'))).toBe(true);
        expect((await live.recevoir()).type).toBe('lancer');
    });
});
