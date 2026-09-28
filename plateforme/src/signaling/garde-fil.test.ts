// Criteria ①, ② and ③ AT SOCKET LEVEL, on real `WebSocket`s.
//
// 🔴 `TURN_URL` and `TURN_SECRET` are set here, and it is indispensable:
// without them `configurationIce` returns `undefined` and the relay NEVER sends
// an `ice-config` (`ice.ts`). The assertion "no `ice-config` before the
// close" would then be TRUE whatever happened — a check unable
// to fail, the pattern this repository has paid for four times. The WITNESS of the first
// test proves it in the same run: an AUTHENTICATED client, for its part,
// receives one.
//
// The clock of the test service is INJECTED: criterion ② requires it
// to move between two handshakes, and `startServer` takes no
// clock. This file therefore builds its guard itself and calls
// `createSignalingServer(port, garde, frein, proxyDeConfiance)` — 🔴 FOUR
// ARGUMENTS NOW, NOT TWO: `frein` and `proxyDeConfiance` joined it
// in sub-block P5 then in correction round 1 (the "any
// request" budget), and this line still said "(port, garde)" while
// the call further down takes four — an acknowledged implementation choice, the
// `port` form being the one `server.test.ts` has tested since milestone 1.

import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { WebSocket } from 'ws';
import { garde as fabriquerGarde, type Garde } from '../identite/garde';
import { signer, DUREE_JETON_ACCES_MS } from '../identite/jeton';
import { Frein } from '../securite/frein';
import { ProprieteDeSession } from './propriete';
import { createSignalingServer } from './relais';
import { poserTurnAmbiant } from './turn-harnais';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const T0 = 1_787_000_000_000;
/// The prefix of a simulated VM, of the REAL length that
/// `agents/prefixe.ts` produces (22 base64url characters). The guard requires
/// the subject of an agent token to PREFIX the requested session.
const P = 'RhH1x2QmTz9kLpVbNc7dAw';

let serveur: ReturnType<typeof createSignalingServer> | undefined;
let maintenant = T0;
let proprietes: ProprieteDeSession;
let garde: Garde;
/// ⚠️ THE RESTORE GOES THROUGH THE HARNESS, AND IT IS NOT FOR COMFORT.
/// Written by hand, it reassigned `process.env.TURN_URL = turnAvant.url`,
/// where `turnAvant.url` is `undefined` on a machine without TURN — and
/// `process.env` coerces to a string: the variable came out as `"undefined"`,
/// TRUTHY, so `configurationIce` then delivered an ICE configuration
/// whose URL was the word `undefined`. Measured, never observed biting:
/// the file order today puts `server.test.ts` BEFORE this one, and
/// it would have been enough for one of the two to change size to reverse it.
let restaurerTurn: () => void;

beforeAll(() => {
    restaurerTurn = poserTurnAmbiant({
        url: 'turn:127.0.0.1:3478',
        secret: 'un-secret-turn-de-test',
    });
});

afterAll(() => {
    restaurerTurn();
});

function start(): number {
    maintenant = T0;
    proprietes = new ProprieteDeSession();
    garde = fabriquerGarde(SECRET, () => maintenant, proprietes);
    // A FRESH brake per call, like `garde` and `proprietes` just above:
    // this file makes several handshakes per test, far below the
    // "any request" budget (`securite/frein.ts::BUDGET_REQUETES`), but
    // a brake shared between tests would make a count drift from one test to
    // the other.
    serveur = createSignalingServer(0, garde, new Frein(), new Set());
    return serveur.port;
}

afterEach(async () => {
    await serveur?.close();
    serveur = undefined;
    vi.restoreAllMocks();
});

interface Suivi {
    messages: any[];
    /// The REAL order of events: `message` and `close` follow each other as
    /// they arrived. It is what allows asserting that the refusal
    /// arrived BEFORE the close, and not the reverse.
    ordre: string[];
    ferme: Promise<void>;
    socket: WebSocket;
}

function poignee(port: number, corps: unknown): Promise<Suivi> {
    return new Promise((resolve, reject) => {
        const w = new WebSocket(`ws://127.0.0.1:${port}`);
        const suivi: Suivi = {
            messages: [],
            ordre: [],
            socket: w,
            ferme: new Promise((r) => w.once('close', () => r())),
        };
        const minuteur = setTimeout(() => reject(new Error('no outcome within 3000 ms')), 3000);
        w.on('message', (brut) => {
            suivi.messages.push(JSON.parse(brut.toString()));
            suivi.ordre.push('message');
        });
        w.on('close', () => suivi.ordre.push('close'));
        w.once('open', () => {
            clearTimeout(minuteur);
            w.send(JSON.stringify(corps));
            // Gives the server time to answer and, if need be, to
            // close. A bound, never an infinite wait.
            setTimeout(() => resolve(suivi), 250);
        });
        w.once('error', () => {
            clearTimeout(minuteur);
            resolve(suivi);
        });
    });
}

describe('the guard, at the socket level', () => {
    it('CRITERION ①: a client without a token is refused, and sees NO ice-config', async () => {
        const port = start();
        const refuse = await poignee(port, { role: 'client', session: 's-1' });

        // First assertion: the refusal is typed.
        expect(refuse.messages).toContainEqual(
            expect.objectContaining({ type: 'error', motif: 'jeton-absent' }),
        );
        // 🔴 Second assertion, REQUIRED by the spec just like the
        // first: a service that refused AFTER sending
        // `ice-config` would pass the first and would leak 24-hour
        // TURN credentials.
        expect(refuse.messages.map((m) => m.type)).not.toContain('ice-config');

        // WITNESS, in the same run: an AUTHENTICATED client receives one.
        // Without it, the assertion above would be true whatever happened.
        const admis = await poignee(port, {
            role: 'client',
            session: 's-temoin',
            jeton: signer('u1', SECRET, T0),
        });
        expect(admis.messages.map((m) => m.type)).toContain('ice-config');
        admis.socket.terminate();
    });

    it('the socket is CLOSED after the refusal, and the message arrived BEFORE', async () => {
        const port = start();
        const refuse = await poignee(port, { role: 'client', session: 's-1' });
        await refuse.ferme;
        // Closing before sending would truncate the message: the peer would see a
        // close with no reason.
        expect(refuse.ordre).toEqual(['message', 'close']);
        expect(refuse.socket.readyState).toBe(WebSocket.CLOSED);
    });

    it('CRITERION ②: a token whose duration has elapsed is refused jeton-expire', async () => {
        const port = start();
        const jeton = signer('u1', SECRET, T0);
        // The same token passes at t0…
        const admis = await poignee(port, { role: 'client', session: 's-2', jeton });
        expect(admis.messages.map((m) => m.type)).not.toContain('error');
        admis.socket.terminate();

        // 🔴 …and the service's clock MOVES. Freezing it would make this test inert.
        maintenant = T0 + DUREE_JETON_ACCES_MS;
        const expire = await poignee(port, { role: 'client', session: 's-3', jeton });
        expect(expire.messages).toContainEqual(
            expect.objectContaining({ type: 'error', motif: 'jeton-expire' }),
        );
    });

    it('CRITERION ③: u2 is refused the session of u1, and the LOG names it', async () => {
        const port = start();
        const journal = vi.spyOn(console, 'warn').mockImplementation(() => {});

        const un = await poignee(port, {
            role: 'client',
            session: 's-privee',
            jeton: signer('u1', SECRET, T0),
        });
        expect(un.messages.map((m) => m.type)).not.toContain('error');

        const deux = await poignee(port, {
            role: 'client',
            session: 's-privee',
            jeton: signer('u2', SECRET, T0),
        });
        const error = deux.messages.find((m) => m.type === 'error');
        expect(error).toBeDefined();
        expect(error.motif).toBe('session-refusee');
        // The message ON THE WIRE names neither the session nor its owner.
        expect(error.reason).not.toContain('s-privee');
        expect(error.reason).not.toContain('u1');

        // The LOG, for its part, carries the session name AND the requester.
        const lignes = journal.mock.calls.map((c) => String(c[0])).join('\n');
        expect(lignes).toContain('s-privee');
        expect(lignes).toContain('u2');

        un.socket.terminate();
    });

    it('after both peers left, u2 CAN take the session', async () => {
        const port = start();
        const un = await poignee(port, {
            role: 'client',
            session: 's-rendue',
            jeton: signer('u1', SECRET, T0),
        });
        expect(proprietes.proprietaire('s-rendue')).toBe('u1');

        un.socket.close();
        await un.ferme;
        await new Promise((r) => setTimeout(r, 100));
        // Never releasing would lose the session name for life.
        expect(proprietes.proprietaire('s-rendue')).toBeUndefined();

        const deux = await poignee(port, {
            role: 'client',
            session: 's-rendue',
            jeton: signer('u2', SECRET, T0),
        });
        expect(deux.messages.map((m) => m.type)).not.toContain('error');
        expect(proprietes.proprietaire('s-rendue')).toBe('u2');
        deux.socket.terminate();
    });

    it('🔴 an `agent` peer WITHOUT a token is REFUSED — the E2 window is CLOSED', async () => {
        // 🔴 THIS TEST IS THE EXACT INVERSE OF THE ONE P2 SHIPPED, and P2 had
        // foreseen it: "The day P3 inverts it, it will have to be rewritten ON PURPOSE,
        // not by surprise." It is done, on purpose, and the red is FREE —
        // P2's binary carries it.
        const port = start();
        const agent = await poignee(port, { role: 'agent', session: 'bureau' });
        expect(agent.messages.map((m) => m.type)).toContain('error');
        agent.socket.terminate();
    });

    it('🔴 …and it receives NO `ice-config` — THE LEAK of E12 is closed', async () => {
        // 🔴 THIS TEST IS THE SUBSTANCE OF E12, and the refusal above was
        // only half of it. A service that refused AFTER sending
        // `ice-config` would pass the previous test word for word, and would
        // still leak to an ANONYMOUS peer TURN credentials valid for 86,400 s
        // (`ice.ts`) — that is, exactly what P2 had named and left
        // open.
        //
        // 🔴 IT LIVES IN A DISTINCT TEST, never as the second assertion of the
        // previous one: `expect` interrupts at the first, and this leak —
        // the only one P3 really closes — would then be tested by
        // nothing. It is P2's lesson ①A-bis, applied in advance.
        const port = start();
        const agent = await poignee(port, { role: 'agent', session: 'bureau' });
        expect(agent.messages.map((m) => m.type)).not.toContain('ice-config');
        agent.socket.terminate();

        // WITNESS, IN THE SAME RUN AND ON THE SAME ROLE: an
        // AUTHENTICATED agent, for its part, receives one. Without it, the assertion above
        // would be true whatever happened the day `ice-config` stopped
        // being sent to agents — and `TURN_URL`/`TURN_SECRET`, set in
        // `beforeAll`, only prove half of this non-vacuity.
        //
        // The token is of TYPE `agent` and its subject is the VM's PREFIX:
        // the guard requires both (`identite/garde.ts`).
        const admis = await poignee(port, {
            role: 'agent',
            session: `${P}:bureau`,
            jeton: signer(P, SECRET, T0, undefined, 'agent'),
        });
        expect(admis.messages.map((m) => m.type)).toContain('ice-config');
        admis.socket.terminate();
    });
});
