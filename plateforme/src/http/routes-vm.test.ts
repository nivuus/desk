// `GET /vm` and `POST /vm/:id/:operation`, tested THROUGH a real HTTP
// server, in the style of `routes-auth.test.ts`.
//
// 🔴 CRITERION ① IS IN THIS FILE: `POST /vm/<id>/instantane` returns 501 and
// a typed body, never silence nor a 200.
//
// ⚠️ FIFTEEN TESTS AND NOT THE PLAN'S THIRTEEN, announced before being read. The two
// extra ones: the `OPTIONS` preflight request (without which neither of the two
// routes is reachable from a browser — see `routes-vm.ts`), and the
// method refusal on `/vm`.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createServer, type Server } from 'node:http';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { enroler, marquerVu } from '../depot/agent';
import { createUser } from '../depot/utilisateur';
import { ouvrirSession } from '../depot/session';
import { signer } from '../identite/jeton';
import { inventaireStatique } from '../orchestration/inventaire-statique';
import type { Orchestrateur } from '../orchestration/interface';
import type { Outcome } from '../orchestration/refus';
import { BACKEND_STATIQUE } from '../orchestration/refus';
import { Frein, REQUETES_MAX_ADRESSE } from '../securite/frein';
import { servirVm } from './routes-vm';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const ORIGINE = 'http://127.0.0.1:5173';
const MS = 1_787_136_773_742;

let base: Pilote | undefined;
let http: Server | undefined;

afterEach(async () => {
    if (http) await new Promise<void>((r) => http!.close(() => r()));
    http = undefined;
    await base?.fermer();
    base = undefined;
});

/// Mounts a server that carries ONLY this route, plus the generic 404 of
/// `serveur.ts` reproduced word for word: that is how a `false` returned by
/// `servirVm` becomes observable.
///
/// ⚠️ `frein` IS A PARAMETER, NOT A VALUE FROZEN IN THE CLOSURE: without
/// it, each test would receive the SAME default through the same expression, which
/// would be without consequence HERE (each test calls `servir` once), but
/// it is the same construction `routes-auth.test.ts` uses for its
/// own brake tests — keeping it here saves a future test of this
/// file from having to reinvent it.
async function servir(
    nom: string,
    origineClient?: string,
    frein: Frein = new Frein(),
    orchestrateurDe: (b: Pilote) => Orchestrateur = (b) => inventaireStatique(b, () => MS),
): Promise<string> {
    base = await baseNeuve(nom);
    const b = base;
    http = createServer((req, rep) => {
        void servirVm(req, rep, {
            base: b,
            secretJeton: SECRET,
            origineClient,
            maintenant: () => MS,
            frein,
            proxyDeConfiance: new Set<string>(),
            orchestrateur: orchestrateurDe(b),
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

async function poserVm(p: Pilote, id: string, nom: string, prefixe: string): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [id, nom, '192.168.3.2']);
    await enroler(p, id, 'empreinte-opaque', prefixe);
    await marquerVu(p, id, MS);
}

async function attribuer(p: Pilote, vmId: string, email: string): Promise<string> {
    const u = await createUser(p, email, 'empreinte-opaque-de-test', MS);
    await p.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', [u, vmId]);
    return u;
}

function jetonDe(sujet: string): string {
    return signer(sujet, SECRET, MS);
}

function withIt(jeton?: string, autres: Record<string, string> = {}): Record<string, string> {
    return jeton === undefined ? autres : { authorization: `Bearer ${jeton}`, ...autres };
}

interface CorpsVm { vms?: Array<Record<string, unknown>>; refus?: string; motif?: string }
async function corpsDe(r: Response): Promise<CorpsVm & Record<string, unknown>> {
    return (await r.json()) as CorpsVm & Record<string, unknown>;
}

describe(`routes /vm, engine=${MOTEUR}`, () => {
    it('`GET /vm` WITHOUT a header → 401 jeton-absent', async () => {
        // 🔴 The red: serving without a token. The inventory would become public.
        const url = await servir('rvm-401');
        const r = await fetch(`${url}/vm`);
        expect(r.status).toBe(401);
        expect((await corpsDe(r)).refus).toBe('jeton-absent');
    });

    it('🔴 `GET /vm` with an AGENT token → 403 jeton-agent', async () => {
        // 🔴 The red: accepting the `agent` type. An agent would see a human's
        // inventory — the second of the two confusions `identite/jeton.ts`
        // lists.
        const url = await servir('rvm-403');
        const jetonAgent = signer('PREFIXEdelaVM', SECRET, MS, undefined, 'agent');
        const r = await fetch(`${url}/vm`, { headers: withIt(jetonAgent) });
        expect(r.status).toBe(403);
        expect((await corpsDe(r)).refus).toBe('jeton-agent');
    });

    it('🔴 `GET /vm` returns ONLY the VMs of the requester', async () => {
        // 🔴 The red: returning the whole inventory. ⚠️ THIS TEST IS DISTINCT FROM
        // THAT OF `vmsDe`: that one tests the pure function, this one
        // tests that it is indeed CALLED. A route that did not call the
        // filter would leave `selection.test.ts` perfectly green.
        const url = await servir('rvm-filtre');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        await poserVm(base!, 'v2', 'w2', 'PREFIXEv2');
        await poserVm(base!, 'v3', 'w3', 'PREFIXEv3'); // in the pool, to nobody
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        await attribuer(base!, 'v2', 'bob@exemple.test');

        const r = await fetch(`${url}/vm`, { headers: withIt(jetonDe(alice)) });
        expect(r.status).toBe(200);
        const corps = await corpsDe(r);
        expect(corps.vms!.map((v) => v.id)).toEqual(['v1']);
    });

    it('`GET /vm` returns `etat`, `prefixe` and `sessions_ouvertes`', async () => {
        // 🔴 The red: omitting a field. The hub would have nothing to display.
        const url = await servir('rvm-champs');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        await ouvrirSession(base!, 'PREFIXEv1:bureau', MS, alice, 'v1');

        const r = await fetch(`${url}/vm`, { headers: withIt(jetonDe(alice)) });
        const [vm] = (await corpsDe(r)).vms!;
        expect(vm.id).toBe('v1');
        expect(vm.nom).toBe('w1');
        expect(vm.etat).toBe('prete');
        expect(vm.prefixe).toBe('PREFIXEv1');
        expect(vm.sessions_ouvertes).toBe(1);
    });

    it('🔴 `GET /vm` does NOT return `adresse`', async () => {
        // 🔴 The red: including it. A VM's address is internal topology
        // of which the browser has NO use — it talks to signaling,
        // never to the VM. Returning it would expose it to every authenticated
        // user without any need requiring it (D7).
        const url = await servir('rvm-adresse');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm`, { headers: withIt(jetonDe(alice)) });
        const [vm] = (await corpsDe(r)).vms!;
        expect(Object.keys(vm).sort()).toEqual(
            ['etat', 'id', 'nom', 'prefixe', 'sessions_ouvertes'].sort(),
        );
        expect(vm.adresse).toBeUndefined();
    });

    it('🔴 `POST /vm/<id>/instantane` → 501 and a TYPED body — criterion ①', async () => {
        // 🔴 The red: returning 200, or silence. It is LITERALLY
        // criterion ① of spec §4 "P4".
        const url = await servir('rvm-instantane');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm/v1/instantane`, {
            method: 'POST',
            headers: withIt(jetonDe(alice)),
        });
        expect(r.status).toBe(501);
        expect(await corpsDe(r)).toEqual({
            motif: 'non-supporte',
            operation: 'instantane',
            backend: BACKEND_STATIQUE,
        });
    });

    it('`POST /vm/<id>/demarrer` → 501', async () => {
        const url = await servir('rvm-start');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm/v1/demarrer`, {
            method: 'POST',
            headers: withIt(jetonDe(alice)),
        });
        expect(r.status).toBe(501);
        expect((await corpsDe(r)).operation).toBe('demarrer');
    });

    it('`POST /vm/<id>/arreter` → 501', async () => {
        const url = await servir('rvm-arreter');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm/v1/arreter`, {
            method: 'POST',
            headers: withIt(jetonDe(alice)),
        });
        expect(r.status).toBe(501);
        expect((await corpsDe(r)).operation).toBe('arreter');
    });

    it('🔴 `POST /vm/<id>/attribuer` → GENERIC 404, never 501 nor 200', async () => {
        // 🔴 The red: adding `attribuer` to `OPERATIONS_HTTP`. The assignment
        // would become reachable by any authenticated user — there
        // is no administration role in this service, so the route
        // could require nothing more than an ordinary token (D8).
        //
        // ⚠️ The body is that of the 404 of `serveur.ts`, `not found\\n`: the
        // route does NOT SERVE this path, it does not refuse it.
        const url = await servir('rvm-attribuer');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm/v1/attribuer`, {
            method: 'POST',
            headers: withIt(jetonDe(alice)),
        });
        expect(r.status).toBe(404);
        expect(await r.text()).toBe('not found\n');
    });

    it('🔴 an INVENTED verb → generic 404, never a 501 that would lie', async () => {
        // 🔴 The red: validating AFTER serving. An unknown verb would receive
        // a 501, which would claim the operation EXISTS and is not
        // supported — while it does not exist. The list is an ALLOWLIST.
        const url = await servir('rvm-invente');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        for (const verbe of ['exploser', 'lister', 'etat', '']) {
            const r = await fetch(`${url}/vm/v1/${verbe}`, {
                method: 'POST',
                headers: withIt(jetonDe(alice)),
            });
            expect(r.status).toBe(404);
        }
    });

    it('🔴 SOMEONE ELSE\'S VM → 404 `vm-inconnue`', async () => {
        // 🔴 The red: telling "unknown" apart from "someone else's".
        const url = await servir('rvm-autrui');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        await poserVm(base!, 'v2', 'w2', 'PREFIXEv2');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        await attribuer(base!, 'v2', 'bob@exemple.test');
        const r = await fetch(`${url}/vm/v2/instantane`, {
            method: 'POST',
            headers: withIt(jetonDe(alice)),
        });
        expect(r.status).toBe(404);
        expect((await corpsDe(r)).motif).toBe('vm-inconnue');
    });

    it('🔴 …and its body is IDENTICAL, character for character, to that of a non-existent VM', async () => {
        // ⚠️ `it()` DISTINCT from the previous one, and it is the heart: `expect`
        // would stop at the first assertion, so that the equality of the two
        // bodies — the very oracle being closed — would be tested by nothing.
        // It is the exact form of P3's criterion ②.
        const url = await servir('rvm-autrui-identique');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        await poserVm(base!, 'v2', 'w2', 'PREFIXEv2');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        await attribuer(base!, 'v2', 'bob@exemple.test');
        const j = withIt(jetonDe(alice));

        const autrui = await fetch(`${url}/vm/v2/instantane`, { method: 'POST', headers: j });
        const inexistante = await fetch(`${url}/vm/v-jamais-creee/instantane`, {
            method: 'POST',
            headers: j,
        });
        expect(autrui.status).toBe(inexistante.status);
        // CHARACTER FOR CHARACTER, not field by field: a different spacing or
        // key order would already be a signal.
        expect(await autrui.text()).toBe(await inexistante.text());
    });

    it('the CORS headers are set when `origineClient` is configured', async () => {
        // 🔴 The red: omitting them. The browser would refuse to read the
        // response WITHOUT any Node test seeing it — `cors.ts` says so of
        // itself, and it is why this assertion exists.
        const url = await servir('rvm-cors', ORIGINE);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm`, { headers: withIt(jetonDe(alice), { origin: ORIGINE }) });
        expect(r.headers.get('access-control-allow-origin')).toBe(ORIGINE);
        expect(r.headers.get('vary')).toBe('Origin');
        // And on a REFUSAL too: a 401 the browser cannot read
        // shows as a network failure, not as an invitation to
        // reconnect.
        const refus = await fetch(`${url}/vm`, { headers: { origin: ORIGINE } });
        expect(refus.status).toBe(401);
        expect(refus.headers.get('access-control-allow-origin')).toBe(ORIGINE);
    });

    it('🔴 the `OPTIONS` preflight request is served — otherwise the route is UNREACHABLE', async () => {
        // 🔴 A DEFECT OF THE PLAN, FOUND AND NOT COPIED: task 9 prescribes
        // no handling of `OPTIONS`. But `GET /vm` carries `Authorization`,
        // which makes the request NON-SIMPLE: the browser first emits a
        // preflight request, to which a 404 opposes a refusal — and the real
        // request is never sent. The route would therefore be unreachable
        // from a browser, exactly as without the
        // `Access-Control-Allow-Headers: authorization` header of task 8.
        // 🔴 The red: not handling `OPTIONS`.
        const url = await servir('rvm-options', ORIGINE);
        for (const chemin of ['/vm', '/vm/v1/instantane']) {
            const r = await fetch(`${url}${chemin}`, {
                method: 'OPTIONS',
                headers: { origin: ORIGINE },
            });
            expect(r.status).toBe(204);
            expect(r.headers.get('access-control-allow-origin')).toBe(ORIGINE);
            expect(r.headers.get('access-control-allow-headers')).toContain('authorization');
        }
    });

    it('a method other than GET on `/vm` → 405, never 404', async () => {
        // The path EXISTS; it is the method that does not fit. A 404
        // would send one looking for an absent route. Same choice as `routes-auth.ts`.
        const url = await servir('rvm-methode');
        const alice = await createUser(base!, 'alice@exemple.test', 'e', MS);
        const r = await fetch(`${url}/vm`, { method: 'POST', headers: withIt(jetonDe(alice)) });
        expect(r.status).toBe(405);
        expect((await corpsDe(r)).refus).toBe('methode');
    });

    it('🔴 the « any request » budget brakes `GET /vm` after too many requests from the same address', async () => {
        // 🔴 The red: never consulting `BUDGET_REQUETES`. Without a token,
        // each request would return 401 forever — `GET /vm` has no
        // notion of failure, and it is exactly why this budget exists
        // (see `securite/frein.ts`).
        const url = await servir('rvm-frein-requetes');
        let last: Response | undefined;
        for (let i = 0; i < REQUETES_MAX_ADRESSE + 1; i++) {
            last = await fetch(`${url}/vm`);
        }
        expect(last!.status).toBe(429);
        expect((await corpsDe(last!)).refus).toBe('trop-de-requetes');
        const retry = last!.headers.get('retry-after');
        expect(retry).not.toBeNull();
        expect(Number(retry)).toBeGreaterThan(0);
    });

    it('🔴 `POST /vm/<id>/demarrer` on the requester\'s VM reaches the INJECTED orchestrator', async () => {
        // 🔴 The red: the route building its own `inventaireStatique` per
        // request. Every other test would stay green, yet the real backend
        // (the one that wakes the VM) would never be called.
        const start = vi.fn(
            async (_vm: string): Promise<Outcome> => ({
                ok: false,
                motif: 'hote-inaccessible',
                operation: 'demarrer',
                backend: 'double',
            }),
        );
        const url = await servir('rvm-injecte', undefined, undefined, (b) => ({
            ...inventaireStatique(b, () => MS),
            start,
        }));
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm/v1/demarrer`, {
            method: 'POST',
            headers: withIt(jetonDe(alice)),
        });
        expect(start).toHaveBeenCalledTimes(1);
        expect(start).toHaveBeenCalledWith('v1');
        expect(r.status).toBe(503);
        expect(await corpsDe(r)).toEqual({
            motif: 'hote-inaccessible',
            operation: 'demarrer',
            backend: 'double',
        });
    });

    it('🔴 `POST /vm/<id>/demarrer` on SOMEONE ELSE\'S VM does not reach the orchestrator', async () => {
        const start = vi.fn(async (_vm: string): Promise<Outcome> => ({ ok: true }));
        const url = await servir('rvm-injecte-autrui', undefined, undefined, (b) => ({
            ...inventaireStatique(b, () => MS),
            start,
        }));
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        await poserVm(base!, 'v2', 'w2', 'PREFIXEv2');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        await attribuer(base!, 'v2', 'bob@exemple.test');
        const r = await fetch(`${url}/vm/v2/demarrer`, {
            method: 'POST',
            headers: withIt(jetonDe(alice)),
        });
        expect(r.status).toBe(404);
        expect(start).not.toHaveBeenCalled();
    });
});
