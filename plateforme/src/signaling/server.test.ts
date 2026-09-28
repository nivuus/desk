import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import type { Garde } from '../identite/garde';
import { Frein, REQUETES_MAX_ADRESSE } from '../securite/frein';
import { createSignalingServer } from './relais';
import { poserTurnAmbiant } from './turn-harnais';

/// A guard that accepts everything, LOCAL TO THIS TEST FILE and never exported
/// by production code.
///
/// These twelve tests test the RELAY — pairing, offer relay, isolation
/// of sessions —, never authentication, which has its own file
/// (`garde-fil.test.ts`). Giving them a real guard would add a
/// signed token without measuring anything more.
///
/// ⚠️ Nothing of the sort exists on the production side: the only guard factory requires
/// a secret, and `PLATEFORME_SECRET_JETON` has NO default (`config.ts`).
const GARDE_OUVERTE: Garde = {
    verify: () => ({ ok: true }),
    revendiquer: () => {},
    liberer: () => {},
};

/// 🔴 THESE TWELVE TESTS READ "THE NEXT MESSAGE", SO THEY REQUIRE A
/// RELAY WITHOUT TURN — and they SET it, instead of hoping for it.
///
/// `relais.ts` sends an `ice-config` to each peer as soon as it declares itself, as soon
/// as `TURN_URL` and `TURN_SECRET` are set. `nextMessage` would
/// then return that `ice-config` instead of the expected offer. This file failed
/// exactly that way — six tests out of twelve — when `scripts/verify-all.sh`
/// was run from a shell that had done `source .env`: the measurement bore
/// on the environment of whoever ran it, not on the service.
///
/// ⚠️ Neutralising does NOT mean giving up the production configuration, where TURN
/// is indeed set: the `describe` "with a configured TURN server", at
/// the bottom of this file, measures it explicitly. Without it, removing TURN from here
/// would remove this case from coverage instead of naming it.
let restaurerTurn: () => void;

beforeAll(() => {
    restaurerTurn = poserTurnAmbiant();
});

afterAll(() => {
    restaurerTurn();
});

let server: ReturnType<typeof createSignalingServer>;

function connect(role: 'agent' | 'client', session: string): Promise<WebSocket> {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
        ws.on('error', reject);
        ws.on('open', () => {
            ws.send(JSON.stringify({ role, session }));
            resolve(ws);
        });
    });
}

/// The next message that is not a SERVICE message of the relay.
///
/// 🔴 `pair-present` IS FILTERED HERE, AND THE FILTER IS NOT A CONVENIENCE.
/// Since 30 August 2026 the relay notifies an `agent` peer already in place
/// that a `client` has just joined it (`signaling/pair-present.ts`): a
/// test waiting for "the next message" on the agent's socket would
/// therefore receive that news and not the answer it caused.
///
/// ⚠️ **IT ALSO REPAIRS A FRAGILITY THAT PREDATED THIS BATCH.** The old
/// form used `ws.once`, which only listens from its attachment:
/// a message that arrived earlier was lost, and the next assertion passed or
/// not depending on scheduling. Here, the listener is set for the duration of
/// the wait and removed on exit, whatever the outcome.
///
/// **What this filter does NOT do**: establish that `pair-present` is indeed
/// emitted. It is `pair-present.test.ts` that measures it — without it, this filter
/// would be indistinguishable from sweeping under the rug.
function nextMessage(ws: WebSocket): Promise<any> {
    return new Promise((resolve, reject) => {
        const finir = (action: () => void) => {
            clearTimeout(timer);
            ws.off('message', surMessage);
            action();
        };
        const surMessage = (raw: any) => {
            const message = JSON.parse(raw.toString());
            if (message?.type === 'pair-present') return;
            finir(() => resolve(message));
        };
        const timer = setTimeout(() => finir(() => reject(new Error('no message received'))), 2000);
        ws.on('message', surMessage);
    });
}

// Closes the socket and waits for the server to have processed the `close` event
// (the server's `close` handler is synchronous but runs after a local
// network round trip; the small delay lets that round trip finish
// before the test queries the server-side state through a new connection).
function closeAndWait(ws: WebSocket): Promise<void> {
    return new Promise((resolve) => {
        ws.once('close', () => setTimeout(resolve, 50));
        ws.close();
    });
}

// 🔴 A FRESH BRAKE PER TEST, never shared: otherwise the new test of the
// "any request" budget, further down, would exhaust the budget of ALL the
// tests that follow it in this file, on the same address `127.0.0.1`.
beforeEach(() => {
    server = createSignalingServer(0, GARDE_OUVERTE, new Frein(), new Set());
});

afterEach(async () => {
    await server.close();
});

describe('signaling server', () => {
    it('relays an offer from the client to the agent', async () => {
        const agent = await connect('agent', 's1');
        const client = await connect('client', 's1');

        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 offer' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 offer' });

        agent.close();
        client.close();
    });

    it('relays an answer from the agent to the client', async () => {
        const agent = await connect('agent', 's2');
        const client = await connect('client', 's2');

        agent.send(JSON.stringify({ type: 'answer', sdp: 'v=0 answer' }));
        expect(await nextMessage(client)).toEqual({ type: 'answer', sdp: 'v=0 answer' });

        agent.close();
        client.close();
    });

    it('isolates the sessions from one another', async () => {
        const agentA = await connect('agent', 'sa');
        const clientB = await connect('client', 'sb');

        clientB.send(JSON.stringify({ type: 'offer', sdp: 'pour sb' }));
        await expect(nextMessage(agentA)).rejects.toThrow(/no message/);

        agentA.close();
        clientB.close();
    });

    it('signals the disappearance of the peer', async () => {
        const agent = await connect('agent', 's3');
        const client = await connect('client', 's3');

        agent.close();
        expect(await nextMessage(client)).toEqual({ type: 'peer-gone' });

        client.close();
    });

    it('rejects an invalid first message', async () => {
        const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
        await new Promise((resolve) => ws.on('open', resolve));
        ws.send(JSON.stringify({ bonjour: true }));
        expect(await nextMessage(ws)).toEqual({
            type: 'error',
            reason: 'invalid first message: {role, session} expected',
        });
        ws.close();
    });

    it('rejects a second agent on the same session', async () => {
        const first = await connect('agent', 's4');
        const second = await connect('agent', 's4');
        // 🔴 `motif` IS TYPED, `reason` IS A SENTENCE. `toEqual` is STRICT:
        // it is what guarantees no field slipped away, and it is
        // why this assertion is extended rather than doubled.
        expect(await nextMessage(second)).toEqual({
            type: 'error',
            reason: 'an agent is already connected to session s4',
            motif: 'role-occupe',
        });
        first.close();
        second.close();
    });

    it('the refusal carries a reason the client can decide on WITHOUT reading the sentence', async () => {
        // 🔴 IT IS THE FIELD'S REASON FOR BEING. The hub elects a carrier tab
        // through Web Locks; its FALLBACK (browser without that API) must
        // tell "the place is taken" — to be swallowed silently — from a refusal
        // for another cause, which must be displayed. Deciding on `reason`
        // would force the client to compare a human SENTENCE, which gets
        // reworded: F1's trap.
        const premier = await connect('client', 's-motif');
        const second = await connect('client', 's-motif');
        const message = await nextMessage(second);
        expect(message.motif).toBe('role-occupe');
        premier.close();
        second.close();
    });

    // Correction round 1: a first JSON message that is valid but not an object
    // (`null`) crashed the handler (`null.role` throws an uncaught
    // TypeError). These two tests lock in the fix: the faulty peer
    // receives an error and stays connected, AND an independent session opened
    // in parallel keeps working normally after the incident — which
    // proves the server (in the sense of the process hosting it in this same
    // test, see the caveat below) was not globally affected.
    //
    // Beware: vitest installs its own uncaught exception
    // handler, which can make the test fail without crashing the
    // process. These two tests therefore prove the behaviour of the
    // exported function `createSignalingServer`, but not that of the real
    // process launched via `index.ts`: it is `resilience.test.ts` (separate child
    // process, outside the vitest runtime) that brings that proof.
    it('rejects a `null` root message as first message without crashing, and allows retrying', async () => {
        const faulty = new WebSocket(`ws://127.0.0.1:${server.port}`);
        await new Promise((resolve) => faulty.on('open', resolve));

        const errorReceived = nextMessage(faulty);
        faulty.send('null');
        expect(await errorReceived).toEqual({
            type: 'error',
            reason: 'invalid first message: {role, session} expected',
        });

        // The faulty connection stays usable: a second attempt, valid this
        // time, registers normally and relays like any session.
        faulty.send(JSON.stringify({ role: 'agent', session: 'retry' }));
        const client = await connect('client', 'retry');
        client.send(JSON.stringify({ type: 'offer', sdp: 'after retry' }));
        expect(await nextMessage(faulty)).toEqual({ type: 'offer', sdp: 'after retry' });

        // A totally independent session, already open during the incident,
        // also keeps relaying normally.
        const agent = await connect('agent', 'temoin');
        const clientTemoin = await connect('client', 'temoin');
        clientTemoin.send(JSON.stringify({ type: 'offer', sdp: 'witness session' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'witness session' });

        faulty.close();
        client.close();
        agent.close();
        clientTemoin.close();
    });

    it('rejects a `null` root message as a later message without crashing, and the session keeps working', async () => {
        const agent = await connect('agent', 's5');
        const client = await connect('client', 's5');

        // Witness session opened in parallel, before sending the malicious
        // message, to prove it is not affected by the incident.
        const agentTemoin = await connect('agent', 'temoin2');
        const clientTemoin = await connect('client', 'temoin2');

        const errorReceived = nextMessage(client);
        client.send('null');
        expect(await errorReceived).toEqual({
            type: 'error',
            reason: 'invalid message: JSON object expected',
        });

        // Session s5 stays functional after the incident.
        client.send(JSON.stringify({ type: 'offer', sdp: 'after null' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'after null' });

        // The independent witness session still works normally.
        clientTemoin.send(JSON.stringify({ type: 'offer', sdp: 'witness still alive' }));
        expect(await nextMessage(agentTemoin)).toEqual({ type: 'offer', sdp: 'witness still alive' });

        agent.close();
        client.close();
        agentTemoin.close();
        clientTemoin.close();
    });

    // Sub-block D1: the control session reuses the existing agent/client
    // roles, on a reserved session_id, to talk between the
    // supervisor and the browser's "bureau" page.
    it('relays the messages of the control session between supervisor and shell', async () => {
        const agent = await connect('agent', 'bureau');
        const client = await connect('client', 'bureau');

        agent.send(JSON.stringify({ type: 'fenetre-ouverte', session: 'w-1', titre: 'Bloc-notes' }));
        expect(await nextMessage(client)).toEqual({
            type: 'fenetre-ouverte',
            session: 'w-1',
            titre: 'Bloc-notes',
        });

        client.send(JSON.stringify({ type: 'viewport', session: 'w-1', largeur: 1600, hauteur: 900 }));
        expect(await nextMessage(agent)).toEqual({
            type: 'viewport',
            session: 'w-1',
            largeur: 1600,
            hauteur: 900,
        });

        agent.close();
        client.close();
    });

    // D1's case: the page opens its connection and sends its offer BEFORE
    // the supervisor has launched its child (it is this page's viewport
    // that decides the size of the virtual output, so nothing can be
    // launched earlier). Without memorisation, the offer fell into the void and the
    // session was never established.
    it("delivers to the agent the offer that arrived before it", async () => {
        const client = await connect('client', 'w-tardive');
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 client-offer' }));

        // The agent arrives afterwards.
        const agent = await connect('agent', 'w-tardive');
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 client-offer' });

        agent.close();
        client.close();
    });

    it('delivers only the last offer, not all the ones received', async () => {
        const client = await connect('client', 'w-rejeu');
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 premiere' }));
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 second' }));

        const agent = await connect('agent', 'w-rejeu');
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 second' });

        agent.close();
        client.close();
    });

    // Without this forgetting, an agent reconnecting on a reused
    // identifier would receive the offer of a dead session.
    it("forgets the stored offer when the session empties", async () => {
        const client = await connect('client', 'w-videe');
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 stale' }));
        await closeAndWait(client);

        const agent = await connect('agent', 'w-videe');
        await expect(nextMessage(agent)).rejects.toThrow(/no message/);

        agent.close();
    });
});

// 🔴 THE RELAY'S "ANY REQUEST" BUDGET — legacy of the missing brakes
// (25 August 2026). Before this batch, NOTHING bounded the number of connections
// a single address could open on `/signal`: `TRAME_MAX_OCTETS`
// (`http/serveur.ts`) bounds the size of a message, not the number of
// sockets. This `describe` is SEPARATE from the first: it needs to count
// raw CONNECTIONS, without ever sending `{role, session}` — the brake
// bites BEFORE the first message, see `relais.ts`.
describe('the « any request » budget of the relay', () => {
    it('🔴 refuses the connection after too many connections from the same address', async () => {
        // 🔴 The red: never consulting `BUDGET_REQUETES` at connection.
        // Without a token or message, each connection would stay open
        // forever — this relay has no notion of failure at this stage.
        const sockets: WebSocket[] = [];
        try {
            let lastMessage: { type: string; motif?: string; retryApresS?: number } | undefined;
            for (let i = 0; i <= REQUETES_MAX_ADRESSE; i++) {
                const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
                sockets.push(ws);
                if (i < REQUETES_MAX_ADRESSE) {
                    // Under the budget: the connection opens normally, and
                    // receives NOTHING as long as it sends no message.
                    await new Promise<void>((resolve, reject) => {
                        ws.once('open', () => resolve());
                        ws.once('error', reject);
                    });
                } else {
                    // The (N+1)th: refused before any message, on the
                    // `connection` event alone.
                    lastMessage = await new Promise((resolve, reject) => {
                        const minuteur = setTimeout(
                            () => reject(new Error('no message received')),
                            2000,
                        );
                        ws.once('message', (raw) => {
                            clearTimeout(minuteur);
                            resolve(JSON.parse(raw.toString()));
                        });
                        ws.once('error', reject);
                    });
                }
            }
            expect(lastMessage?.type).toBe('error');
            expect(lastMessage?.motif).toBe('trop-de-requetes');
            // 🔴 correction round 1, critical ②: without `retryApresS`,
            // the agent refused here cannot know how long
            // to wait before retrying — it is the cheapest half
            // of the remedy to the lockout documented by
            // `agent/src/superviseur/boucle/surveillance_pont.rs`.
            expect(lastMessage?.retryApresS).toBeGreaterThan(0);
        } finally {
            for (const s of sockets) s.close();
        }
    });
});

// The PRODUCTION configuration: a TURN server is set. It is the case that
// this file's `poserTurnAmbiant()` rules out everywhere else, and it would be
// dishonest to rule it out without measuring it anywhere — one would then remove it
// from coverage while believing one was only stabilising the tests.
//
// ⚠️ It is also the test that would have CAUGHT the defect: it states that
// the `ice-config` arrives, and that the relay keeps relaying AFTER it. The
// twelve tests above stated it backwards, without saying so, by assuming
// that the first message received was always the one they expected.
describe('with a TURN server configured', () => {
    let restaurer: () => void;

    beforeAll(() => {
        restaurer = poserTurnAmbiant({
            url: 'turn:127.0.0.1:3478',
            secret: 'un-secret-turn-de-test',
        });
    });

    afterAll(() => {
        restaurer();
    });

    it("delivers the ice-config to each peer, THEN relays normally", async () => {
        const agent = await connect('agent', 'turn-1');
        // First message of the agent: its ICE configuration, before any relay.
        const iceAgent = await nextMessage(agent);
        expect(iceAgent.type).toBe('ice-config');
        expect(iceAgent.iceServers[0].urls).toBe('turn:127.0.0.1:3478');

        const client = await connect('client', 'turn-1');
        // The client receives its own: both ends need one.
        expect((await nextMessage(client)).type).toBe('ice-config');

        // And the relay still relays, once the ice-config has gone through.
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 after ice' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 after ice' });

        agent.close();
        client.close();
    });
});
