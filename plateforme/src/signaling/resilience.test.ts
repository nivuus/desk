// Correction round 1: a `null` message sent by a peer crashed
// the whole Node process (uncaught TypeError on `null.role`), killing at
// the same time every active session — a denial of service in one frame,
// without authentication required.
//
// ⚠️ "without authentication required" describes the state at THE TIME, and it remains true
// today for a reason that must be written, otherwise one will believe the sentence
// stale since sub-block P2's guard: the shape check runs on the
// FIRST message, hence BEFORE the guard has seen a token (see `relais.ts`,
// `isJsonObject`). **The one-frame denial of service is therefore still open
// to anyone who reaches the port**, and that is indeed why this file still
// exists.
//
// ❌ THIS SENTENCE CARRIED A SECOND REASON, "and the `agent` role stays
// anonymous anyway until P3", AND SUB-BLOCK P3 MADE IT FALSE (19
// August 2026, cross-cutting review at the end of the branch). The `agent` role now
// requires a token of type `agent` whose subject prefixes the session
// (`identite/garde.ts`), and a peer that presents nothing receives NO
// `ice-config` (`journaux-plateforme-p3/e2-ferme-{1,2}.log`, two runs).
//
// ⚠️ IT IS NOT THE MAIN SENTENCE THAT FALLS, IT IS ONE OF ITS TWO
// REASONS — and the distinction is the fate P3's plan prescribed
// in advance for this line (E15): **CLARIFY, not correct**. The first
// reason is enough on its own, and it is indeed what carries: the shape
// check is upstream of any guard, so NO authentication, not even
// P3's, can close this path.
//
// ⚠️ **THIS LINE SAID "THE BRAKE IS P5 ③", AND IT WAS THE WRONG REMEDY**
// (cross-cutting review, 20 August 2026). P5's brake is set on
// `/auth/connexion`, `/auth/rafraichir` and `/agent`; **it does NOT cover the
// relay** — `signaling/relais.ts` does not import `Frein`, and
// `createSignalingServer` receives none. What really closes the one-frame denial
// of service is `TRAME_MAX_OCTETS` (`http/serveur.ts`), set as
// `maxPayload` on BOTH WebSocket servers, hence applied by `ws` BEFORE
// the frame reaches the slightest shape check — and it is tested
// by the `describe` of this very file, further down.
//
// ⚠️ **WHAT REMAINED OPEN, AND WHICH `maxPayload` DID NOT CLOSE — HALF FIXED
// BY CORRECTION ROUND 1 (25 August 2026): a peer could
// still open MANY CONNECTIONS, and silent connections were
// counted by nothing.** It is now only true of ONE of the two paths:
// `signaling/relais.ts` now bounds the NUMBER of connections on
// `/signal`, at the `connection` event — before any message, hence before
// a silent peer has even had the chance to send one
// (`securite/frein.ts::BUDGET_REQUETES`). `/agent` (`agents/canal.ts`), for its part,
// IS NOT: its enrolment brake counts ATTEMPTS, per message,
// never connections; neither it nor `deploiement/nginx.conf` (which sets neither
// `limit_conn` nor `limit_req`) counts a peer that opens then stays silent.
// `http/serveur.ts` now says so next to the constant, up to date for both
// paths.
//

// This file does NOT test `createSignalingServer` in memory: vitest installs
// its own uncaught exception handler, which can make
// a test fail without the process running it really stopping. A test
// run "inside" vitest therefore cannot prove that a real Node process,
// started via `index.ts` (which installs no `process.on('uncaughtException')`),
// would survive the same attack.
//
// We launch here the real entry point (`src/index.ts`) as an independent child
// process, we send it the malicious message through a real WebSocket socket,
// then we check two distinct things:
//   1. the faulty peer receives a clean JSON error (no connection cut);
//   2. the process is still alive afterwards, and a third-party session opened in
//      parallel keeps relaying normally — the proof that there was no
//      denial of service.

import { type ChildProcessWithoutNullStreams, spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import { TRAME_MAX_OCTETS } from '../http/serveur';
import { signer } from '../identite/jeton';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const signalingRoot = path.join(__dirname, '..', '..');
const tsxBin = path.join(signalingRoot, 'node_modules', '.bin', 'tsx');

/// The child process's signing secret, named ONCE: it is set
/// in its `env` below and serves to sign the tokens `connectTo`
/// sends. Two diverging values would make every `client` handshake
/// refused, with an obscure diagnosis.
const SECRET_ENFANT = 'un-secret-de-plateforme-de-quarante-octets';

/// The prefix of the simulated VM. The guard requires the agent token's subject
/// to prefix the requested session (sub-block P3): all the sessions of this
/// file therefore carry it.
const P = 'RhH1x2QmTz9kLpVbNc7dAw';

let child: ChildProcessWithoutNullStreams;
let port: number;

// Starts `index.ts` as a real Node process and waits for it to announce its
// listening port (SIGNALING_PORT=0: the system assigns a free one).
function startRealServer(): Promise<{ child: ChildProcessWithoutNullStreams; port: number }> {
    return new Promise((resolve, reject) => {
        const proc = spawn(tsxBin, [path.join(signalingRoot, 'src', 'index.ts')], {
            cwd: signalingRoot,
            // `PLATEFORME_BASE` is FIXED, and does not inherit from the environment:
            // this file tests the resilience of the RELAY, never the choice of
            // engine. Without this guard, `npm run test:postgres` would pass
            // `PLATEFORME_BASE=postgres` to the child without passing it
            // the URL (which the harness holds hardcoded), the child would fall back to
            // `:memory:` which `pg` takes for a host, and would die on
            // ECONNREFUSED — the service being RIGHT to refuse to start.
            env: {
                ...process.env,
                PLATEFORME_HOTE: '127.0.0.1',
                PLATEFORME_PORT: '0',
                PLATEFORME_BASE: 'sqlite',
                PLATEFORME_BASE_URL: ':memory:',
                // `lireConfig` now refuses to start without a signing
                // secret, and invents none: without this line
                // the child dies before announcing its port.
                PLATEFORME_SECRET_JETON: SECRET_ENFANT,
                // 🔴 TASK 6: `lireConfig` now refuses to start in
                // `pomerium` mode — the default, not redefined here — without
                // `PLATEFORME_PROXY_DE_CONFIANCE`. This file does not test
                // identity, only the resilience of the relay: the value
                // therefore does not matter, its mere PRESENCE is enough to
                // let the child start.
                //
                // ⚠️ A FOREIGN ADDRESS, DELIBERATELY — fixed in review
                // ("correction round 1", 22 August 2026): this comment
                // already said "the value does not matter" while
                // setting `127.0.0.1`, which is precisely the address from
                // which this file connects (`connectTo`, further down).
                // The sentence was therefore only true by accident. `10.9.9.9`
                // makes it true BY CONSTRUCTION: this file never opens
                // `/auth/moi`, so the guard of `routes-identite.ts` is
                // never consulted here, whatever the declared address.
                PLATEFORME_PROXY_DE_CONFIANCE: '10.9.9.9',
            },
        });

        let output = '';
        const onStdout = (chunk: Buffer) => {
            output += chunk.toString();
            const match = output.match(/port (\d+)/);
            if (match) {
                proc.stdout.off('data', onStdout);
                clearTimeout(timer);
                resolve({ child: proc, port: Number(match[1]) });
            }
        };
        proc.stdout.on('data', onStdout);

        let stderr = '';
        proc.stderr.on('data', (chunk: Buffer) => {
            stderr += chunk.toString();
        });

        const timer = setTimeout(() => {
            reject(new Error(`startup of the signaling process timed out. stderr: ${stderr}`));
        }, 10000);

        proc.once('error', reject);
        proc.once('exit', (code) => {
            clearTimeout(timer);
            reject(new Error(`signaling process ended prematurely (code ${code}). stderr: ${stderr}`));
        });
    });
}

function connectTo(targetPort: number, role: 'agent' | 'client', session: string): Promise<WebSocket> {
    return new Promise((resolve, reject) => {
        // ⚠️ `/signal` SINCE 21 AUGUST 2026: the relay moved from the
        // root (see the header of `http/serveur.ts`). This file launches the
        // REAL `index.ts`, hence the REAL upgrade routine — a regression
        // on this path would be seen by NO "in-memory" test.
        const ws = new WebSocket(`ws://127.0.0.1:${targetPort}/signal`);
        ws.on('error', reject);
        ws.on('open', () => {
            // 🔴 BOTH ROLES require a token, signed with the SAME secret
            // as the one set in the child's `env` above: the
            // `client` role since P2, the `agent` role since P3, which closed
            // E2's anonymous window. The agent token is of TYPE `agent`, and
            // its subject is the PREFIX its session must carry.
            const jeton = role === 'client'
                ? signer('u-resilience', SECRET_ENFANT, Date.now())
                : signer(P, SECRET_ENFANT, Date.now(), undefined, 'agent');
            ws.send(JSON.stringify({ role, session, jeton }));
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

beforeAll(async () => {
    ({ child, port } = await startRealServer());
}, 15000);

afterAll(() => {
    child.kill();
});

describe('resilience of the real process (index.ts) to a `null` message', () => {
    it('survives a `null` as first message: the culprit receives an error, a third session keeps working', async () => {
        // ⚠️ `/signal`: see the note of `connectTo` above.
        const faulty = new WebSocket(`ws://127.0.0.1:${port}/signal`);
        await new Promise((resolve, reject) => {
            faulty.on('open', resolve);
            faulty.on('error', reject);
        });

        const errorReceived = nextMessage(faulty);
        faulty.send('null');
        expect(await errorReceived).toEqual({
            type: 'error',
            reason: 'invalid first message: {role, session} expected',
        });

        // Proof no. 1: the process is not dead.
        expect(child.exitCode).toBeNull();
        expect(child.killed).toBe(false);

        // Proof no. 2: an independent session, opened after the incident,
        // relays normally — the server still answers the network.
        const agent = await connectTo(port, 'agent', `${P}:preuve-null-premier`);
        const client = await connectTo(port, 'client', `${P}:preuve-null-premier`);
        client.send(JSON.stringify({ type: 'offer', sdp: 'still alive (first message)' }));
        expect(await nextMessage(agent)).toEqual({
            type: 'offer',
            sdp: 'still alive (first message)',
        });

        faulty.close();
        agent.close();
        client.close();
    });

    it('survives a `null` as a later message: the culprit receives an error, a third session keeps working', async () => {
        const agent = await connectTo(port, 'agent', `${P}:proof-null-later`);
        const client = await connectTo(port, 'client', `${P}:proof-null-later`);

        // Witness session opened before the incident, to prove it is
        // not affected by what will happen to the previous session.
        const agentTemoin = await connectTo(port, 'agent', `${P}:witness-null-later`);
        const clientTemoin = await connectTo(port, 'client', `${P}:witness-null-later`);

        const errorReceived = nextMessage(client);
        client.send('null');
        expect(await errorReceived).toEqual({
            type: 'error',
            reason: 'invalid message: JSON object expected',
        });

        // Proof no. 1: the process is not dead.
        expect(child.exitCode).toBeNull();
        expect(child.killed).toBe(false);

        // Proof no. 2: the witness session, opened before the incident, still
        // works normally afterwards.
        clientTemoin.send(JSON.stringify({ type: 'offer', sdp: 'witness still alive' }));
        expect(await nextMessage(agentTemoin)).toEqual({
            type: 'offer',
            sdp: 'witness still alive',
        });

        agent.close();
        client.close();
        agentTemoin.close();
        clientTemoin.close();
    });
});

describe('resilience of the real process to a TOO LARGE frame (P5)', () => {
    /// 🔴 THIS TEST LIVES HERE, AND NOT IN `http/serveur.test.ts`, FOR THE EXACT
    /// REASON THE HEADER OF THIS FILE GIVES: "vitest installs its own
    /// uncaught exception handler", so that a test
    /// run INSIDE vitest cannot prove that a real Node process
    /// would survive. `serveur.test.ts` tests that the frame is REFUSED; only
    /// this file can test that the service SURVIVES it.
    ///
    /// 🔴 AND THE DANGER IS NEW, INTRODUCED BY THE FIX ITSELF. Setting
    /// `maxPayload` makes `ws` emit `error` on the SERVER socket; yet
    /// no server socket of this service had an `error` listener — found
    /// on 20 August 2026, `grep -n "on('error'" relais.ts canal.ts serveur.ts`
    /// only returned the startup's `http.once('error', reject)`. An
    /// `EventEmitter` that emits `error` without a listener THROWS, and an exception
    /// not caught in a Node event handler takes down the whole
    /// process. Without the listener, THE ANTI-DENIAL-OF-SERVICE FIX WOULD HAVE
    /// GIVEN A WORSE DENIAL OF SERVICE: a single anonymous frame killing the
    /// service instead of slowing it down.
    it('🔴 survives a frame beyond `maxPayload`, on `/signal` as on `/agent`', async () => {
        // ⚠️ `/signal`, NOT `/`, SINCE 21 AUGUST 2026: see the note of
        // `connectTo` above. `/` is now closed, and pushing a
        // frame there would no longer prove anything about `maxPayload`.
        for (const chemin of ['/signal', '/agent']) {
            const gros = new WebSocket(`ws://127.0.0.1:${port}${chemin}`);
            await new Promise((resolve, reject) => {
                gros.on('open', resolve);
                gros.on('error', reject);
            });
            const ferme = new Promise<number>((resolve) => gros.once('close', resolve));
            // An `error` listener ON THE CLIENT SIDE: it is the faulty peer, and its
            // socket throws when the server cuts it mid-write.
            gros.on('error', () => {});
            gros.send('x'.repeat(TRAME_MAX_OCTETS + 1));
            // 1009 = « message trop grand » (RFC 6455).
            expect(await ferme).toBe(1009);
        }

        // Proof no. 1: the process is not dead.
        expect(child.exitCode).toBeNull();
        expect(child.killed).toBe(false);

        // Proof no. 2: a session opened AFTER the incident relays
        // normally. Without it, a killed process would read exactly
        // like a healthy process — `exitCode` does not flip instantly.
        const agent = await connectTo(port, 'agent', `${P}:preuve-trame-geante`);
        const client = await connectTo(port, 'client', `${P}:preuve-trame-geante`);
        client.send(JSON.stringify({ type: 'offer', sdp: 'alive after the giant frame' }));
        expect(await nextMessage(agent)).toEqual({
            type: 'offer',
            sdp: 'alive after the giant frame',
        });
        agent.close();
        client.close();
    }, 20000);
});
