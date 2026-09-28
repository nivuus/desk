// Three properties, two of which are acceptance criteria of P1.
//
// ⚠️ The first test can NOT prove inaccessibility from another
// interface: on a development machine, `127.0.0.1` and the address of
// the interface are both local, and a probe from outside
// would require a machine outside the network. It proves the service listens on
// the NAMED address, not that it listens nowhere else. The real guarantee
// of criterion ④ comes from `config.ts` — no default, hence no unnamed
// listen — and its test is `config.test.ts`, seen red in task 1.

import { afterEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { signer } from '../identite/jeton';
import { startServer, TRAME_MAX_OCTETS, type ServicePlateforme } from './serveur';
import type { Pilote as TypePilote } from '../base/pilote';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// An EXPLICIT test secret, never `''`: `lireConfig` refuses the empty
// string, and a `Config` literal built by hand must carry a value
// a service would really accept.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

/// The prefix of the simulated VM, of the real length `agents/prefixe.ts`
/// produces. The guard requires the agent token's subject to prefix the session.
const P = 'RhH1x2QmTz9kLpVbNc7dAw';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    // No proxy declared: see `config.ts`, the empty set is the default
    // and means "trust nobody's announced address".
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
    auth: 'pomerium',
};

let service: ServicePlateforme | undefined;
let base: Pilote | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

/// The server requires a database: it is REQUIRED, not optional (see
/// `startServer`). These three tests do not measure the trace — it is
/// `signaling/trace.test.ts` that measures it —, they only need a
/// live database to start.
async function servir(nom: string, config: Config = CONFIG): Promise<ServicePlateforme> {
    base = await baseNeuve(nom);
    return startServer(config, base);
}

/// Opens a socket and returns its outcome: `ouvert` if it reached `open`, otherwise
/// `ferme`. A time bound prevents a test from hanging forever.
function tenter(url: string, borneMs = 3000): Promise<'ouvert' | 'ferme'> {
    return new Promise((resolve, reject) => {
        const w = new WebSocket(url);
        const minuteur = setTimeout(() => {
            w.terminate();
            reject(new Error(`no outcome for ${url} within ${borneMs} ms`));
        }, borneMs);
        const finir = (issue: 'ouvert' | 'ferme') => {
            clearTimeout(minuteur);
            w.removeAllListeners();
            w.terminate();
            resolve(issue);
        };
        w.on('open', () => finir('ouvert'));
        w.on('error', () => finir('ferme'));
        w.on('close', () => finir('ferme'));
    });
}

describe('demarrerServeur', () => {
    it("listens ONLY on the named address", async () => {
        service = await servir('http-adresse');
        expect(service.port).toBeGreaterThan(0);
        await expect(tenter(`ws://127.0.0.1:${service.port}/signal`)).resolves.toBe('ouvert');
    });

    it('refuses the WebSocket upgrade on an unknown path', async () => {
        service = await servir('http-chemin');
        await expect(tenter(`ws://127.0.0.1:${service.port}/inconnu`)).resolves.toBe('ferme');
    });

    it('🔴 accepts the WebSocket upgrade on /agent — the second branch', async () => {
        // 🔴 The red: not adding the branch. The `404` written by hand
        // for any path other than `/signal` closes it, and it is exactly
        // what the test "refuses the upgrade on an unknown path" tests.
        service = await servir('http-agent');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agent`)).resolves.toBe('ouvert');
    });

    // 🔴 THE RELAY MOVED FROM THE ROOT TO `/signal` ON 21 AUGUST 2026 —
    // FOR THE POMERIUM PROXY, NOT TASTE. See the header of `serveur.ts`.
    // The three tests that follow ARE the move: without the first, the
    // root could stay open (Pomerium would believe it guards the relay, and
    // the relay would answer beside its guard); the second is the witness that
    // makes the first interpretable; the third is the witness that /agent
    // did not move.
    it('🔴 accepts the WebSocket upgrade on /signal — the relay has moved', async () => {
        service = await servir('http-signal');
        await expect(tenter(`ws://127.0.0.1:${service.port}/signal`)).resolves.toBe('ouvert');
    });

    it('🔴 now REFUSES the upgrade on the ROOT', async () => {
        service = await servir('http-racine-fermee');
        await expect(tenter(`ws://127.0.0.1:${service.port}/`)).resolves.toBe('ferme');
    });

    it('/agent is UNCHANGED', async () => {
        service = await servir('http-agent-inchange');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agent`)).resolves.toBe('ouvert');
    });

    it('🔴 ALWAYS refuses an unknown path — /agent does not open the service', async () => {
        // 🔴 The red: replacing `if (chemin !== CHEMIN_SIGNAL)` with a
        // comparison to a denylist (`if (chemin === '/inconnu')`), which
        // would make the service open to ANY path. The test at l. 79 already exists
        // and must stay green; this one adds the neighbouring bound — a path
        // that STARTS with `/agent` without being it is not `/agent`, and a
        // path that STARTS with `/signal` without being it is not `/signal`
        // (review of task 5, finding I2: the EXACT comparison of
        // `/signal` was guarded by NO test — a mutation to
        // `startsWith` passed the file's 16 tests).
        service = await servir('http-unknown-again');
        await expect(tenter(`ws://127.0.0.1:${service.port}/inconnu`)).resolves.toBe('ferme');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agentaire`)).resolves.toBe('ferme');
        await expect(tenter(`ws://127.0.0.1:${service.port}/signalement`)).resolves.toBe('ferme');
    });

    it('serves the signaling relay on /signal, handshake included', async () => {
        // An 'agent' peer and a 'client' peer on '/signal': the client's offer
        // reaches the agent — exactly what `server.test.ts`
        // already tests, replayed here through the HTTP server to prove that
        // going through the upgrade changes nothing. ⚠️ THIS TEST OPENED TWO
        // SOCKETS ON '/' BEFORE 21 AUGUST 2026: fixed with the move of the
        // relay, otherwise it would have turned red without any assertion saying
        // so.
        service = await servir('http-signal-poignee');
        const url = `ws://127.0.0.1:${service.port}/signal`;

        const agent = new WebSocket(url);
        await new Promise((r) => agent.once('open', r));
        // 🔴 The `agent` role also requires its token since P3: E2's anonymous
        // window is closed. The token is of TYPE `agent`, and its subject
        // is the PREFIX the session must carry.
        agent.send(JSON.stringify({
            role: 'agent',
            session: `${P}:racine-1`,
            jeton: signer(P, SECRET, Date.now(), undefined, 'agent'),
        }));

        const client = new WebSocket(url);
        await new Promise((r) => client.once('open', r));
        // The `client` role now requires an access token (sub-block P2):
        // without it the guard refuses and closes the socket. The token is signed with
        // the secret `CONFIG` carries, the very one the service uses.
        client.send(JSON.stringify({
            role: 'client',
            session: `${P}:racine-1`,
            jeton: signer('u-racine', SECRET, Date.now()),
        }));

        const offreRecue = new Promise<string>((resolve) => {
            agent.on('message', (brut) => {
                const m = JSON.parse(brut.toString());
                if (m.type === 'offer') resolve(m.sdp);
            });
        });
        // Lets the client declare itself before emitting its offer.
        await new Promise((r) => setTimeout(r, 50));
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 racine' }));

        await expect(offreRecue).resolves.toBe('v=0 racine');
        agent.terminate();
        client.terminate();
    });
});

describe('the chaining of the four routers', () => {
    it('🔴 the FOUR paths answer, and `/inconnu` returns the 404 WORD FOR WORD', async () => {
        // 🔴 The red: removing a link from the chain. Its route then returns
        // 404 — and it is the most discreet failure possible, since the service
        // answers, listens, and serves the other two.
        // ⚠️ `auth: 'motdepasse'` LOCAL (task 3): this test tests that
        // `/auth/connexion` is indeed CHAINED in `startServer` — a
        // property of P2/P4, distinct from the authentication mode. In
        // `pomerium` mode (the default of `CONFIG`), this route NO LONGER EXISTS AT
        // ALL (see `routes-auth.ts`), and the `400` assertion below
        // would become `404` for a reason outside the scope of this test.
        service = await servir('http-chain', { ...CONFIG, auth: 'motdepasse' });
        const url = `http://127.0.0.1:${service.port}`;

        // `/auth/connexion` STILL ANSWERS IN MOTDEPASSE MODE: P2 is not
        // broken by P4. Without a body it returns 400 `{refus:'forme'}`, which
        // proves it was SERVED — a 404 would say it was not.
        const auth = await fetch(`${url}/auth/connexion`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: '{}',
        });
        expect(auth.status).toBe(400);

        // `/vm` answers: without a token, 401 — not 404.
        const vm = await fetch(`${url}/vm`);
        expect(vm.status).toBe(401);

        // `/session` answers: without a token, 401 — not 404.
        const session = await fetch(`${url}/session`, { method: 'POST' });
        expect(session.status).toBe(401);

        // 🔴 `/applications` AND `/application/:id/lancer` ANSWER: without a
        // token, 401 — not 404. It is THE ONLY LINE that proves the
        // fourth link is really chained in `startServer`, and
        // it is the same argument as the one for the `/agent` channel: without it, the
        // peer would see a service that answers, listens, serves the other three, and
        // returns 404 on this one.
        //
        // ⚠️ THE BODY IS READ, NOT ONLY THE CODE. A 401 `{refus:...}` can
        // only come from the route; a generic 404 carries `not found\n`.
        const list = await fetch(`${url}/applications?vm=v-1`);
        expect([list.status, await list.json()]).toEqual([401, { refus: 'jeton-absent' }]);

        const lancer = await fetch(`${url}/application/a-1/lancer`, { method: 'POST' });
        expect([lancer.status, await lancer.json()]).toEqual([401, { refus: 'jeton-absent' }]);

        // And P1's 404 is intact, CHARACTER FOR CHARACTER.
        const inconnu = await fetch(`${url}/inconnu`);
        expect(inconnu.status).toBe(404);
        expect(await inconnu.text()).toBe('not found\n');
        expect(inconnu.headers.get('content-type')).toBe('text/plain; charset=utf-8');
    });

    it('🔴 a route that REJECTS returns 500 { refus: interne }, and the process SURVIVES', async () => {
        // 🔴 The red: removing the `.catch`. A promise rejected in a
        // Node event handler TAKES DOWN THE WHOLE PROCESS — it is the failure
        // mode `serveur.ts` already documents, and P4's chaining
        // adds two routers that touch the database, hence two new sources
        // of rejection.
        //
        // The database is SABOTAGED: every read throws. `GET /vm` with a valid
        // token then reaches `orchestrateur.lister()` and rejects.
        const sabotee: TypePilote = {
            async interroger<T>(): Promise<T[]> {
                throw new Error('database unreachable');
            },
            async executer() {
                throw new Error('database unreachable');
            },
            async transaction<T>(corps: (p: TypePilote) => Promise<T>): Promise<T> {
                return corps(sabotee);
            },
            async fermer() {},
        };
        service = await startServer(CONFIG, sabotee);
        const url = `http://127.0.0.1:${service.port}`;
        const r = await fetch(`${url}/vm`, {
            headers: { authorization: `Bearer ${signer('u-ada', SECRET, Date.now())}` },
        });
        expect(r.status).toBe(500);
        expect(await r.json()).toEqual({ refus: 'interne' });

        // 🔴 THE PROCESS SURVIVES: the next request is served. Without this
        // second request, a killed process would read exactly like a
        // healthy process — the test would already have given its verdict.
        const apres = await fetch(`${url}/inconnu`);
        expect(apres.status).toBe(404);
    });

    it('🔴 BOTH WebSocket upgrades are UNCHANGED BY THE HTTP CHAINING', async () => {
        // 🔴 The red: touching the `upgrade` routing FROM THIS CHAINING.
        // It is off topic for this task, and this test pins it — the CALL
        // POINT of the HTTP chaining (`void servirTout(...)`, in
        // `startServer`) and the WebSocket routing live in the same
        // function, hence within reach. ⚠️ SINCE THE EXTRACTION OF 22 AUGUST
        // 2026, THE LIST OF ROUTERS ITSELF NO LONGER LIVES HERE: it is
        // in `./chaine.ts` (`servirTout`, exported) — only the CALL POINT
        // stays next to the `upgrade` routing, and it is that neighbourhood this
        // test holds. ⚠️ This test checked `/` before 21 August 2026: the
        // relay moved to `/signal` IN THIS SAME COMMIT (see the header
        // of `serveur.ts`), for a reason outside the scope of THIS test — the
        // proof of the move itself lives in the three dedicated tests further
        // up. What remains its responsibility is added: that the root stays
        // CLOSED, just as `/signal` and `/agent` stay what they are, even
        // after the chaining of the four HTTP routers.
        service = await servir('http-chain-ws');
        await expect(tenter(`ws://127.0.0.1:${service.port}/signal`)).resolves.toBe('ouvert');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agent`)).resolves.toBe('ouvert');
        await expect(tenter(`ws://127.0.0.1:${service.port}/vm`)).resolves.toBe('ferme');
        await expect(tenter(`ws://127.0.0.1:${service.port}/`)).resolves.toBe('ferme');
    });
});

describe('the maximum frame accepted before any authentication', () => {
    /// Opens a socket on `url`, sends `octets` bytes to it, and returns the
    /// close code — or `'servi'` if the socket is still open at the end of
    /// the bound.
    ///
    /// ⚠️ BOUNDED, never an infinite wait: a server that applied
    /// no bound would leave the socket open, and the test must TURN RED, not
    /// hang.
    function pousser(url: string, octets: number): Promise<number | 'servi'> {
        return new Promise((resolve, rejeter) => {
            const w = new WebSocket(url);
            const minuteur = setTimeout(() => {
                w.terminate();
                resolve('servi');
            }, 1500);
            w.once('open', () => {
                // 🔴 A SINGLE FRAME, and its content is VALID JSON once
                // mentally truncated: what is measured is the SIZE,
                // not the shape. `ws` must close BEFORE the `message`
                // handler sees anything at all.
                w.send('x'.repeat(octets));
            });
            w.once('close', (code) => {
                clearTimeout(minuteur);
                resolve(code);
            });
            w.once('error', () => {
                // A socket closed mid-write throws on the client side: it
                // is not a test failure, it is the close it measures.
            });
            setTimeout(() => rejeter(new Error('neither closing nor verdict within 3000 ms')), 3000);
        });
    }

    it('(a) 🔴 a TOO LARGE frame closes the socket with 1009, on `/signal`', async () => {
        // The peer is ANONYMOUS: `signaling/relais.ts:84-86` itself says that
        // the SHAPE check runs some thirty lines BEFORE
        // `garde.verify`. Without a bound, `JSON.parse` on the frame is an
        // allocation then a CPU spike, per socket and per frame, offered to
        // anyone who reaches the port. ⚠️ THIS TEST HIT `/` BEFORE 21 AUGUST
        // 2026: the relay moved to `/signal` in this same commit (see
        // the header of `serveur.ts`) — without this fix, the frame would
        // no longer EVEN go up, and the test would turn red for a reason
        // that has nothing to do with `TRAME_MAX_OCTETS`.
        service = await servir('trame-signal');
        // 1009 = « message trop grand » (RFC 6455).
        await expect(pousser(`ws://127.0.0.1:${service.port}/signal`, TRAME_MAX_OCTETS + 1))
            .resolves.toBe(1009);
    });

    it('(a bis) 🔴 and on `/agent` TOO, which is the other anonymous door', async () => {
        // The enrolment channel is open before any identity: bounding it
        // only on `/signal` would leave half the problem whole.
        service = await servir('trame-agent');
        await expect(pousser(`ws://127.0.0.1:${service.port}/agent`, TRAME_MAX_OCTETS + 1))
            .resolves.toBe(1009);
    });

    it('(b) 🔴 a frame JUST UNDER the bound is accepted and served', async () => {
        // 🔴 WITHOUT THIS TEST, A `maxPayload: 1` WOULD PASS TEST (a). It is the
        // half that prevents the bound from becoming a denial of service set up
        // with our own hands.
        service = await servir('frame-under-bound');
        // The socket stays open: the message is malformed, and the relay
        // lets a malformed message be retried rather than closing.
        await expect(pousser(`ws://127.0.0.1:${service.port}/signal`, TRAME_MAX_OCTETS - 1))
            .resolves.toBe('servi');
    });

    it('(d) 🔴 THE PROCESS SURVIVES the refused frame, and serves the next request', async () => {
        // 🔴 THIS TEST EXISTS BECAUSE THE FIX OF (a) NEARLY WAS WORSE
        // THAN THE DEFECT. Setting `maxPayload` makes `ws` emit `error` on
        // the SERVER socket; yet no server socket of this service had
        // an `error` listener (checked on 20 August 2026:
        // `grep -n "on('error'" relais.ts canal.ts serveur.ts` only returned
        // the startup's `http.once('error', reject)`). An `EventEmitter` that
        // emits `error` without a listener THROWS, and an uncaught exception in
        // a Node event handler TAKES DOWN THE WHOLE PROCESS — the exact failure
        // mode that `signaling/relais.ts` and `signaling/trace.ts`
        // both document.
        //
        // In other words: without the listener, A SINGLE ANONYMOUS FRAME KILLED THE
        // SERVICE, where before it only slowed it down. Vitest saw it
        // ("Vitest caught 2 unhandled errors"), and this test pins it.
        service = await servir('trame-survie');
        const url = `http://127.0.0.1:${service.port}`;
        // ⚠️ Hit `/` before the relay moved to `/signal` — see
        // test (a) above.
        await expect(pousser(`ws://127.0.0.1:${service.port}/signal`, TRAME_MAX_OCTETS + 1))
            .resolves.toBe(1009);
        await expect(pousser(`ws://127.0.0.1:${service.port}/agent`, TRAME_MAX_OCTETS + 1))
            .resolves.toBe(1009);
        // Without this request, a killed process would read exactly like
        // a healthy process — the test would already have given its verdict.
        const apres = await fetch(`${url}/inconnu`);
        expect(apres.status).toBe(404);
    });

    it('(c) the bound is the one the module announces, and it is large', () => {
        // ⚠️ NOT CALIBRATED, and its floor is REASONED, not measured: see
        // the header of `serveur.ts`. This test pins the value so that a
        // change is a deliberate gesture.
        expect(TRAME_MAX_OCTETS).toBe(256 * 1024);
    });
});

// 🔴 THE TEST THAT WOULD HAVE CAUGHT CORRECTION ROUND 1: `evincer` of the two
// stores (`apps/icones.ts`, `apps/magasin-tranches.ts`) was declared,
// defined, UNIT tested — and called by NOBODY. A test that calls
// `unTour` or `evincer` by hand can NOT see this defect: it proves
// the mechanism works, never that the SERVICE triggers it. This one only
// touches `startServer`, the real entry point.
describe('the wiring of the background clean-up (correction round 1)', () => {
    it(
        '🔴 starting the service evicts an orphan and old icon — ' +
            'WITHOUT a manual call to evincer, unTour, nor demarrerNettoyage',
        async () => {
            const racineIcones = join(mkdtempSync(join(tmpdir(), 'g2-icones-cablage-')), 'icones');
            const racineTranches = join(
                mkdtempSync(join(tmpdir(), 'g3-tranches-cablage-')),
                'televersements',
            );
            mkdirSync(racineIcones, { recursive: true });

            const orpheline = createHash('sha256').update('orpheline-cablage').digest('hex');
            const chemin = join(racineIcones, orpheline);
            writeFileSync(chemin, 'content never revalidated by lire()');
            // 400 days in the past: well beyond AGE_EVICTION_ICONE_MS
            // (180 days, `apps/icones.ts`).
            const vieux = new Date(Date.now() - 400 * 24 * 60 * 60_000);
            utimesSync(chemin, vieux, vieux);
            expect(existsSync(chemin)).toBe(true);

            const config: Config = {
                ...CONFIG,
                repertoireIcones: racineIcones,
                repertoireTeleversements: racineTranches,
            };
            // 🔴 NO CALL TO `evincer`, `unTour` NOR `startCleanup` HERE:
            // only `startServer` ran. Under round 1's code — the
            // function existed, nothing invoked it —, this file would
            // STILL BE THERE, and this assertion would have turned red. `startCleanup`
            // is AWAITED by `serveur.ts` before this `await` returns
            // control: no arbitrary wait, no polling.
            service = await servir('cablage-nettoyage', config);

            expect(existsSync(chemin)).toBe(false);
        },
    );
});
