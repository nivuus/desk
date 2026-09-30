// `POST /session`: the sequence of spec §4 — the user asks for a
// session, the platform checks the assignment AND the freshness, and returns the
// prefix.
//
// 🔴 CRITERIA ③ AND ④ ARE IN THIS FILE, and each of their assertions
// has its own `it()`: `expect` interrupts a test at the first false
// assertion, so that a second assertion placed next to it would be tested
// by nothing (lesson ①A/①A-bis of P2).
//
// ⚠️ TEN TESTS AND NOT THE PLAN'S NINE, announced before being read: the
// `OPTIONS` preflight request, without which the route is unreachable from a
// browser (same plan defect as in task 9).

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createServer, type Server } from 'node:http';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { SEUIL_INJOIGNABLE_MS } from '../agents/fraicheur';
import { enroler, marquerVu } from '../depot/agent';
import { createUser } from '../depot/utilisateur';
import { signer } from '../identite/jeton';
import { hostOrchestrator } from '../orchestration/host-orchestrator';
import { inventaireStatique } from '../orchestration/inventaire-statique';
import { BACKEND_STATIQUE } from '../orchestration/refus';
import { Wake } from '../orchestration/wake';
import { Frein, REQUETES_MAX_ADRESSE } from '../securite/frein';
import { servirSession, type DependancesSession } from './routes-session';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const ORIGINE = 'http://127.0.0.1:5173';
const MS = 1_787_136_773_742;

/// 🔴 THIS BOUND WAS MEASURED BEFORE BEING WRITTEN, never guessed. The
/// protocol is the plan's: a WARM-UP request first — the first
/// request of a test carries the connection establishment, which is not what
/// is being measured —, then ten timed requests.
///
/// RECORDED on 20 August 2026 on this machine, by the test itself instrumented
/// then restored — FOUR runs, worst case of each:
///
///     sqlite   : 3.49 ms   then 5.93 ms
///     postgres : 7.27 ms   then 18.73 ms
///
/// Worst recorded across all runs: **18.73 ms**, under Postgres. The
/// 250 ms bound is **13.3 times** this worst case, and **8 times below** the
/// 2,000 ms the mutation of ③b inserts — both margins count: the
/// first avoids a flaky test, the second guarantees that the check CAN
/// fail. A bound set without having been measured would be a check one
/// does not know can fail, a pattern this repository has paid for four times.
///
/// ⚠️ FOUR RUNS ARE NOT A RATE, and the machine carries a variable
/// foreign load: this figure bounds what was observed, nothing more.
const BORNE_MS = 250;

let base: Pilote | undefined;
let http: Server | undefined;

afterEach(async () => {
    if (http) await new Promise<void>((r) => http!.close(() => r()));
    http = undefined;
    await base?.fermer();
    base = undefined;
});

/// The clock is INJECTED: it is what makes the transition of criterion ④c
/// observable. A `Date.now()` read in the module would leave only one instant.
///
/// ⚠️ `frein` IS A PARAMETER, FRESH BY DEFAULT PER CALL: each test thus
/// isolates its own budget, without any being able to exhaust another's.
async function servir(
    nom: string,
    instant = MS,
    origineClient?: string,
    frein: Frein = new Frein(),
    /// Built from the base the harness opens, so a test can wire an orchestrator on it.
    surcharges: (b: Pilote) => Partial<DependancesSession> = () => ({}),
): Promise<string> {
    base = await baseNeuve(nom);
    const b = base;
    const extra = surcharges(b);
    http = createServer((req, rep) => {
        void servirSession(req, rep, {
            base: b,
            secretJeton: SECRET,
            origineClient,
            maintenant: () => instant,
            frein,
            proxyDeConfiance: new Set<string>(),
            orchestrateur: inventaireStatique(b, () => instant),
            reveilPossible: false,
            ...extra,
        })
            .then((servie) => {
                if (servie) return;
                rep.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
                rep.end('not found\n');
            })
            .catch((cause) => {
                rep.writeHead(500, { 'content-type': 'application/json; charset=utf-8' });
                rep.end(JSON.stringify({ refus: 'interne', cause: String(cause) }));
            });
    });
    await new Promise<void>((r) => http!.listen(0, '127.0.0.1', () => r()));
    const a = http!.address();
    return `http://127.0.0.1:${typeof a === 'object' && a ? a.port : 0}`;
}

async function poserVm(p: Pilote, id: string, nom: string, prefixe: string, vuA: number | null) {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [id, nom, '192.168.3.2']);
    await enroler(p, id, 'empreinte-opaque', prefixe);
    if (vuA !== null) await marquerVu(p, id, vuA);
}

async function attribuer(p: Pilote, vmId: string, email: string): Promise<string> {
    const u = await createUser(p, email, 'empreinte-opaque-de-test', MS);
    await p.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', [u, vmId]);
    return u;
}

function demander(url: string, jeton?: string, entetes: Record<string, string> = {}) {
    return fetch(`${url}/session`, {
        method: 'POST',
        headers: jeton === undefined ? entetes : { authorization: `Bearer ${jeton}`, ...entetes },
    });
}

async function corpsDe(r: Response): Promise<Record<string, unknown>> {
    return (await r.json()) as Record<string, unknown>;
}

describe(`route POST /session, engine=${MOTEUR}`, () => {
    it('without a token → 401', async () => {
        // 🔴 The red: serving without a token. A VM's prefix would be delivered to
        // anyone.
        const url = await servir('rs-401');
        const r = await demander(url);
        expect(r.status).toBe(401);
        expect((await corpsDe(r)).refus).toBe('jeton-absent');
    });

    it('🔴 with an AGENT token → 403', async () => {
        // ⚠️ `it()` DISTINCT from the previous one: the plan puts them on a single
        // line, but they are two refusals through two different paths.
        // 🔴 The red: accepting the `agent` type.
        const url = await servir('rs-403');
        const r = await demander(url, signer('PREFIXEdelaVM', SECRET, MS, undefined, 'agent'));
        expect(r.status).toBe(403);
        expect((await corpsDe(r)).refus).toBe('jeton-agent');
    });

    it('🔴 success → 200 { vm, nom, prefixe, etat }', async () => {
        // 🔴 The red: omitting `prefixe`. It is THE source P3 waits for —
        // `client/src/prefixe.ts` says in so many words "P4 will plug in the
        // source, and will only have to write into the vault". Without it, the whole
        // sub-block delivers nothing to the browser.
        const url = await servir('rs-ok');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await demander(url, signer(alice, SECRET, MS));
        expect(r.status).toBe(200);
        expect(await corpsDe(r)).toEqual({
            vm: 'v1',
            nom: 'w1',
            prefixe: 'PREFIXEv1',
            etat: 'prete',
        });
    });

    it('🔴 the body carries NEITHER `ice`, NOR `adresse`, NOR a composed session name', async () => {
        // 🔴 The red: adding them (D7). ⚠️ `it()` DISTINCT from the previous one.
        //
        // `ice`: the ICE configuration is PER SESSION — `signaling/ice.ts`
        // composes `${expiration}:${session}` and signs the whole. A VM opens
        // `<prefix>:bureau` PLUS one session per window: the route would only
        // know one out of N, and the relay would keep serving all
        // the others. A second delivery path that covers one session out of
        // N is not a simplification, it is a second place to keep
        // in sync that one has no right to use.
        //
        // `bureau`: the constant already lives in Rust and, on the TypeScript side, as an
        // inline literal (`client/src/bureau/porteur-dom.ts` does
        // `composer(deps.prefixe, 'bureau')` — a `shell-page.ts` that
        // named it was removed in task 9, 31 August 2026), and spec §2.6
        // already names this duplication as a known defect. Adding a
        // THIRD one to save the browser a concatenation would be
        // worsening a defect one knows how to name.
        const url = await servir('rs-sobre');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const corps = await corpsDe(await demander(url, signer(alice, SECRET, MS)));
        expect(Object.keys(corps).sort()).toEqual(['etat', 'nom', 'prefixe', 'vm']);
        for (const interdit of ['ice', 'iceServers', 'ice_config', 'adresse', 'session']) {
            expect(corps[interdit]).toBeUndefined();
        }
        // And the prefix is BARE: never `PREFIXEv1:bureau`.
        expect(corps.prefixe).toBe('PREFIXEv1');
        expect(String(corps.prefixe)).not.toContain(':');
    });

    it('🔴 ③a — a user WITHOUT a VM → 409 { motif: aucune-vm }', async () => {
        // 🔴 The red: returning 200 with an empty list. An empty listing is
        // NOT a refusal: the browser would write an empty string into the vault,
        // `lirePrefixe` would fall back to `''`, and the page would
        // SILENTLY join the shared namespace — the exact silent failure
        // spec §10 names.
        const url = await servir('rs-none');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        await attribuer(base!, 'v1', 'bob@exemple.test');
        const carole = await createUser(base!, 'carole@exemple.test', 'e', MS);
        const r = await demander(url, signer(carole, SECRET, MS));
        expect(r.status).toBe(409);
        expect((await corpsDe(r)).motif).toBe('aucune-vm');
    });

    it('🔴 ③b — the response arrives UNDER THE measured BOUND', async () => {
        // 🔴 The red: inserting `await new Promise(r => setTimeout(r, 2000))`
        // into the route. MEASURED REACHABLE — the bound is an order of
        // magnitude above the worst recorded and far below 2,000 ms.
        //
        // ⚠️ A WARM-UP REQUEST PRECEDES THE MEASUREMENT: the first request
        // carries the connection establishment, which is not what is measured.
        const url = await servir('rs-borne');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const jeton = signer(alice, SECRET, MS);

        await demander(url, jeton); // warm-up, not measured

        let pire = 0;
        for (let i = 0; i < 10; i += 1) {
            const debut = performance.now();
            const r = await demander(url, jeton);
            const duree = performance.now() - debut;
            expect(r.status).toBe(200);
            pire = Math.max(pire, duree);
        }
        expect(pire).toBeLessThan(BORNE_MS);
    });

    it('🔴 ④a — a VM whose `vu_a` is too old → 503, body carrying `etat: injoignable`', async () => {
        // 🔴 The red: hiding behind a generic `{motif:'reessayez'}`.
        // The user must know that THEIR VM does not answer, and not believe in
        // an unavailability of the service.
        const url = await servir('rs-injoignable', MS + SEUIL_INJOIGNABLE_MS + 1);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await demander(url, signer(alice, SECRET, MS));
        expect(r.status).toBe(503);
        const corps = await corpsDe(r);
        expect(corps.motif).toBe('agent-injoignable');
        expect(corps.etat).toBe('injoignable');
        // The prefix is returned ANYWAY: it is known and correct, and the
        // browser needs it so as not to join the shared namespace.
        expect(corps.prefixe).toBe('PREFIXEv1');
    });

    it('🔴 ④b — the SAME body carries `redemarrage: { possible:false, … }`', async () => {
        // 🔴 The red: removing the field. ⚠️ `it()` DISTINCT from ④a, and it is
        // exactly the "and says it cannot restart it" of the
        // criterion: `expect` would stop at the first assertion of ④a.
        //
        // It is the ADMISSION, not the feature: the framing promises "offers
        // a restart", and the v1 backend drives no hypervisor. Saying so
        // is better than saying nothing (D3, "product consequence to
        // own").
        const url = await servir('rs-redemarrage', MS + SEUIL_INJOIGNABLE_MS + 1);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const corps = await corpsDe(await demander(url, signer(alice, SECRET, MS)));
        expect(corps.redemarrage).toEqual({
            possible: false,
            motif: 'non-supporte',
            backend: BACKEND_STATIQUE,
        });
    });

    it('🔴 during a wake, `etat` is `demarrage` and `redemarrage.possible` is true', async () => {
        const instant = MS + SEUIL_INJOIGNABLE_MS + 1;
        const send = vi.fn(async () => ({ ok: true }) as const);
        // The orchestrator is wired on the SAME base as the route.
        let orchestrateur: DependancesSession['orchestrateur'] | undefined;
        const url = await servir('rs-reveil', instant, undefined, new Frein(), (b) => {
            orchestrateur = hostOrchestrator(b, () => instant, new Wake(send, () => instant));
            return { orchestrateur, reveilPossible: true };
        });
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        expect((await orchestrateur!.start('v1')).ok).toBe(true);

        const r = await demander(url, signer(alice, SECRET, MS));
        expect(r.status).toBe(503);
        const corps = await corpsDe(r);
        expect(corps.etat).toBe('demarrage');
        expect(corps.redemarrage).toEqual({ possible: true });
        expect(corps.prefixe).toBe('PREFIXEv1');
    });

    it('🔴 without a channel, `redemarrage.possible` stays false (the admission from before)', async () => {
        // Default harness: `reveilPossible: false` and the static inventory.
        const url = await servir('rs-sans-canal', MS + SEUIL_INJOIGNABLE_MS + 1);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const corps = await corpsDe(await demander(url, signer(alice, SECRET, MS)));
        expect(corps.etat).toBe('injoignable');
        expect(corps.redemarrage).toEqual({
            possible: false,
            motif: 'non-supporte',
            backend: BACKEND_STATIQUE,
        });
    });

    it('🔴 ④c — the TRANSITION is seen: 200 at vu_a + THRESHOLD, 503 one ms later', async () => {
        // 🔴 The red: freezing the injected clock. The bound would no longer be
        // besieged from both sides, and a threshold never crossed proves nothing.
        // ⚠️ `it()` DISTINCT: it is a property of the BOUND, not of the body.
        const juste = await servir('rs-borne-prete', MS + SEUIL_INJOIGNABLE_MS);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        expect((await demander(juste, signer(alice, SECRET, MS))).status).toBe(200);

        // A second service, one millisecond later.
        await new Promise<void>((r) => http!.close(() => r()));
        http = undefined;
        await base!.fermer();
        base = undefined;
        const apres = await servir('rs-borne-injoignable', MS + SEUIL_INJOIGNABLE_MS + 1);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice2 = await attribuer(base!, 'v1', 'alice@exemple.test');
        expect((await demander(apres, signer(alice2, SECRET, MS))).status).toBe(503);
    });

    it('🔴 the `OPTIONS` preflight request is served', async () => {
        // 🔴 Same plan defect as in task 9, found and not copied:
        // `POST /session` carries `Authorization`, so the request is NON-
        // SIMPLE, so the browser first emits an `OPTIONS`. A 404 would
        // make it give up before sending the real request.
        const url = await servir('rs-options', MS, ORIGINE);
        const r = await fetch(`${url}/session`, {
            method: 'OPTIONS',
            headers: { origin: ORIGINE },
        });
        expect(r.status).toBe(204);
        expect(r.headers.get('access-control-allow-origin')).toBe(ORIGINE);
        expect(r.headers.get('access-control-allow-headers')).toContain('authorization');
    });

    it('🔴 the « any request » budget brakes `POST /session` after too many requests from the same address', async () => {
        // 🔴 The red: never consulting `BUDGET_REQUETES`. Without a token,
        // each request would return 401 forever — this route has no
        // notion of failure (see `securite/frein.ts`).
        const url = await servir('rs-frein-requetes');
        let last: Response | undefined;
        for (let i = 0; i < REQUETES_MAX_ADRESSE + 1; i++) {
            last = await demander(url);
        }
        expect(last!.status).toBe(429);
        expect((await corpsDe(last!)).refus).toBe('trop-de-requetes');
        const retry = last!.headers.get('retry-after');
        expect(retry).not.toBeNull();
        expect(Number(retry)).toBeGreaterThan(0);
    });
});
