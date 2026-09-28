// `GET /applications` and `POST /application/:id/lancer`, tested THROUGH a
// real HTTP server, in the style of `routes-vm.test.ts` and
// `routes-auth.test.ts`.
//
// 🔴 THE REGISTRY IS REAL, THE SOCKET IS A DOUBLE. It is what allows
// testing the three outcomes of the launch — success, absent agent, timeout —
// without mounting an agent: the double answers, or stays silent. What a REAL socket does
// is tested elsewhere, by `agents/canal-apps.test.ts`.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createServer, type Server } from 'node:http';
import { DELAI_LANCEMENT_MS, RegistreAgents, type SocketAgent } from '../agents/registre';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { parseDepuisLaPlateforme } from '../../../proto/ts/plateforme';
import { createUser } from '../depot/utilisateur';
// 🔴 THE HARNESS IS EXTRACTED, AND IT WAS BEFORE THE ADDITION of the
// icon route family: this file was at 480 lines for a cap of 500.
import {
    app,
    attribuer,
    withIt,
    jetonDe,
    MS,
    ORIGINE,
    poserApp,
    poserVm,
    SECRET,
} from './routes-harnais';
import { signerUrlIcone, verifyIconUrl } from '../apps/url-icone';
import { servirApplications } from './routes-applications';

let base: Pilote | undefined;
let http: Server | undefined;
let registre = new RegistreAgents();
let maintenant = MS;

afterEach(async () => {
    if (http) await new Promise<void>((r) => http!.close(() => r()));
    http = undefined;
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

/// Mounts a server that carries ONLY this route, plus the generic 404 of
/// `serveur.ts` reproduced word for word: that is how a `false` returned by
/// `servirApplications` becomes observable.
async function servir(nom: string, origineClient?: string): Promise<string> {
    base = await baseNeuve(nom);
    registre = new RegistreAgents();
    maintenant = MS;
    const b = base;
    http = createServer((req, rep) => {
        void servirApplications(req, rep, {
            base: b,
            secretJeton: SECRET,
            origineClient,
            registre,
            maintenant: () => maintenant,
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

/// A socket that answers every order with the given outcome — or stays silent.
function agentQuiRepond(issue: 'raccourci' | 'cible' | 'echec' | null): SocketAgent {
    return {
        readyState: 1,
        send(data: string) {
            if (issue === null) return;
            const ordre = parseDepuisLaPlateforme(data);
            if (ordre.type !== 'lancer') return;
            // On the next loop turn, as a real socket would.
            setTimeout(() => registre.resoudre(ordre.demande, issue), 0);
        },
        close() {},
    };
}

describe(`routes /applications, engine=${MOTEUR}`, () => {
    it("returns `false` on a foreign path: the server's 404 follows", async () => {
        // 🔴 Returning `true` would make this route eat the 404s of all the
        // others, and an unknown path would answer an application JSON body.
        const url = await servir('apps-etranger');
        const r = await fetch(`${url}/nothing-at-all`);
        expect(r.status).toBe(404);
        expect(await r.text()).toBe('not found\n');
    });

    it('🔴 the pattern of `/application/:id/lancer` is ANCHORED AT BOTH ENDS', async () => {
        // 🔴 A `startsWith` would open "a whole family of paths that
        // nobody decided" (`serveur.ts`).
        //
        // 🔴 IT IS THE BODY THAT DISCRIMINATES, NOT THE CODE. A first draft
        // only asserted `404`, and a mutation `startsWith('/application')`
        // SURVIVED IT: the route then ate the whole family and returned
        // its OWN typed 404, indistinguishable from the generic 404 as long as one
        // only read the status. The body `not found\n` is that of
        // `serveur.ts`, and it can only be returned if the route did decline.
        const url = await servir('apps-ancre');
        const declines = [
            `${url}/application/x/lancer/y`,
            `${url}/application/x`,
            `${url}/applicationsdetournees`,
            `${url}/applications/x`,
        ];
        for (const cible of declines) {
            const r = await fetch(cible, { method: 'POST' });
            expect([cible, r.status, await r.text()]).toEqual([cible, 404, 'not found\n']);
        }
        // And the RIGHT path is served — without this witness, the assertions
        // above would hold for a route that serves NOTHING.
        expect((await fetch(`${url}/application/x/lancer`, { method: 'POST' })).status).not.toBe(404);
    });

    it('WITHOUT an `Authorization` header, returns 401', async () => {
        // 🔴 The two routes of P1/P2 are open by construction, and nothing
        // in this repository authenticated an HTTP request before P4. Omitting it
        // here would make the catalogue of any VM readable by anyone.
        const url = await servir('apps-sans-jeton');
        const r = await fetch(`${url}/applications?vm=v-1`);
        expect(r.status).toBe(401);
        expect(await r.json()).toEqual({ refus: 'jeton-absent' });
    });

    it("🔴 with an AGENT token, refuses — agent and human are signed by the SAME secret", async () => {
        // 🔴 Accepting any valid token would reopen P3's E5: without the type
        // claim, the two identities are INTERCHANGEABLE. A compromised agent
        // would then read its own user's catalogue, and launch it.
        const url = await servir('apps-jeton-agent');
        const r = await fetch(`${url}/applications?vm=v-1`, {
            headers: withIt(jetonDe('RhH1x2QmTz9kLpVbNc7dAw', 'agent')),
        });
        expect(r.status).toBe(403);
        expect(await r.json()).toEqual({ refus: 'jeton-agent' });
    });

    it('with an EXPIRED token, refuses — and the test ADVANCES the clock', async () => {
        // 🔴 A frozen clock would make this case inert: it would read a final
        // state instead of seeing the transition. The token is signed at `MS`, and
        // the request is served well after its expiry.
        const url = await servir('apps-jeton-expire');
        const jeton = jetonDe('u-1');
        maintenant = MS + 24 * 60 * 60 * 1000;
        const r = await fetch(`${url}/applications?vm=v-1`, { headers: withIt(jeton) });
        expect(r.status).toBe(401);
        expect(await r.json()).toEqual({ refus: 'jeton-expire' });
    });

    it('serves the `OPTIONS` preflight request, without which nothing is reachable', async () => {
        // ⚠️ Both routes require `Authorization`, which makes the request
        // NON-SIMPLE: the browser first emits an `OPTIONS`, and a 404 would
        // make it give up without ever sending the real request. NO Node
        // test can see the origin policy — it is this assertion, and
        // nothing else, that holds the header.
        const url = await servir('apps-options', ORIGINE);
        const r = await fetch(`${url}/applications`, {
            method: 'OPTIONS',
            headers: { origin: ORIGINE },
        });
        expect(r.status).toBe(204);
        expect(r.headers.get('access-control-allow-origin')).toBe(ORIGINE);
    });

    it('returns the applications of the requested VM, and nothing else', async () => {
        const url = await servir('apps-liste');
        await poserVm(base!, 'v-1');
        await poserVm(base!, 'v-2');
        const u = await attribuer(base!, 'v-1', 'a@exemple.test');
        await poserApp(base!, 'v-1', 'Firefox', 'c-1');
        await poserApp(base!, 'v-2', 'Excel', 'c-2');

        const r = await fetch(`${url}/applications?vm=v-1`, { headers: withIt(jetonDe(u)) });
        expect(r.status).toBe(200);
        const corps = (await r.json()) as { applications: Array<{ nom: string }> };
        expect(corps.applications.map((a) => a.nom)).toEqual(['Firefox']);
    });

    it("🔴 strikes the SIGNED URL of the icon, and `null` when there is none", async () => {
        // 🔴 DECISION OF THE REPOSITORY OWNER, 30 AUGUST 2026: it is HERE, under
        // the bearer token and AFTER the VM ownership check,
        // that an icon URL is minted — never freely. This test pins this
        // chaining: without it, one could move the minting to an
        // open route without anything saying so.
        const url = await servir('apps-icone-url');
        await poserVm(base!, 'v-1');
        const u = await attribuer(base!, 'v-1', 'a@exemple.test');
        const empreinte = 'a'.repeat(64);
        await poserApp(base!, 'v-1', 'Avec', 'c-1', empreinte, { pixels: 256 });
        await poserApp(base!, 'v-1', 'Sans', 'c-2');

        const r = await fetch(`${url}/applications?vm=v-1`, { headers: withIt(jetonDe(u)) });
        const corps = (await r.json()) as {
            applications: Array<{ id: string; nom: string; icone_url: string | null }>;
        };
        const parNom = new Map(corps.applications.map((a) => [a.nom, a]));
        // Without an icon: `null`, never a URL that would return 404.
        expect(parNom.get('Sans')!.icone_url).toBe(null);
        // With an icon: EXACTLY what the product rule mints — never
        // a URL rewritten here, which would only test itself.
        const withIcon = parNom.get('Avec')!;
        expect(withIcon.icone_url).toBe(
            signerUrlIcone(withIcon.id, 'v-1', empreinte, SECRET, maintenant),
        );
        // 🔴 AND IT VERIFIES: the minted signature is the one the icon
        // route will accept. A chaining that minted with ANOTHER key
        // would return a well-formed and systematically refused URL.
        const p = new URL(withIcon.icone_url!, 'http://interne');
        expect(verifyIconUrl(withIcon.id, p.searchParams, SECRET, maintenant)).toEqual({
            ok: true,
            vm: 'v-1',
        });
    });

    it("🔴 a FOREIGN VM answers EXACTLY like an UNKNOWN VM", async () => {
        // 🔴 THIS TEST WAS REVERSED. It pinned `403 {refus:'vm-etrangere'}`,
        // that is decision D9 of G1's plan — an ENUMERATION
        // ORACLE: the return code confirmed to someone not entitled to it
        // that a VM exists. The repository owner ruled for the
        // INDISTINGUISHABLE refusal of `routes-vm.ts`, and the test now pins
        // the indistinguishability itself.
        //
        // 🔴 BOTH BODIES ARE COMPARED CHARACTER FOR CHARACTER, not
        // only the two statuses: a test that only read `404` would be
        // satisfied by the WRONG 404 — the generic one of `serveur.ts` —
        // exactly the trap the pattern-anchoring test, further up in this
        // file, has already paid for once.
        //
        // ⚠️ THIS TEST SETS `vm.utilisateur_id` BY HAND, since nothing
        // fills it before P4 — `npm run admin:agent` leaves the column NULL.
        const url = await servir('apps-etrangere');
        await poserVm(base!, 'v-1');
        await attribuer(base!, 'v-1', 'proprietaire@exemple.test');
        const autre = await createUser(base!, 'autre@exemple.test', 'x', MS);
        await poserApp(base!, 'v-1', 'Firefox', 'c-1');

        const entetes = withIt(jetonDe(autre));
        const etrangere = await fetch(`${url}/applications?vm=v-1`, { headers: entetes });
        const inconnue = await fetch(`${url}/applications?vm=jamais-vue`, { headers: entetes });

        expect([etrangere.status, await etrangere.text()]).toEqual([
            inconnue.status,
            await inconnue.text(),
        ]);
        expect(etrangere.status).toBe(404);
    });

    it("🔴 the body of the refusal CARRIES NO TRACE of the real case", async () => {
        // 🔴 IT IS THE VERY OBJECT OF THE DECISION: the log line distinguishes,
        // the HTTP response never does. Without this assertion, a diagnostic field
        // added "to help" would restore the oracle without any test turning
        // red — the two bodies would keep the same SHAPE while
        // differing, and the test above comparing them with each other would
        // see it, but this one says so by name.
        const url = await servir('apps-etrangere-muette');
        await poserVm(base!, 'v-1');
        await attribuer(base!, 'v-1', 'proprietaire@exemple.test');
        const autre = await createUser(base!, 'autre@exemple.test', 'x', MS);
        await poserApp(base!, 'v-1', 'Firefox', 'c-1');

        const r = await fetch(`${url}/applications?vm=v-1`, { headers: withIt(jetonDe(autre)) });
        const corps = await r.text();
        expect(corps).toBe(JSON.stringify({ refus: 'vm-inconnue' }));
        expect(corps).not.toContain('etrangere');
        expect(corps).not.toContain('proprietaire');
    });

    it("🔴 the LOG LINE, however, tells the two cases apart", async () => {
        // 🔴 IT IS THE EXPLICIT COUNTERPART OF THE INDISTINGUISHABLE REFUSAL. Without
        // it, uniformising would cost the operator all diagnosis: "the
        // VM does not exist" and "it belongs to someone else" would read
        // the same on BOTH sides, and nobody could tell a
        // typing error from an enumeration attempt any more.
        //
        // WHAT MAKES THIS CHECK RED, and the state is REACHABLE: a line
        // that disappeared, a line that named the same case in both
        // situations, or a line that kept quiet about the VM's identifier.
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        const url = await servir('apps-journal-distingue');
        await poserVm(base!, 'v-1');
        await attribuer(base!, 'v-1', 'proprietaire@exemple.test');
        const autre = await createUser(base!, 'autre@exemple.test', 'x', MS);

        const entetes = withIt(jetonDe(autre));
        await fetch(`${url}/applications?vm=v-1`, { headers: entetes });
        const apresEtrangere = traces.join(' | ');
        await fetch(`${url}/applications?vm=jamais-vue`, { headers: entetes });
        const apresInconnue = traces.join(' | ').slice(apresEtrangere.length);

        expect(apresEtrangere).toContain('cas=etrangere');
        expect(apresEtrangere).toContain('v-1');
        expect(apresInconnue).toContain('cas=inconnue');
        expect(apresInconnue).toContain('jamais-vue');
        // And the two lines are NOT the same: without this witness, a single
        // line always saying "refusal" would satisfy the four assertions
        // above as soon as it carried both words.
        expect(apresInconnue).not.toContain('cas=etrangere');
    });

    it('🔴 an UNASSIGNED VM is served, AND the log line is EMITTED', async () => {
        // 🔴 SERVING SILENTLY WOULD MAKE THE LACK OF ISOLATION INVISIBLE. As long
        // as no VM is assigned, every authenticated user sees
        // all VMs — it is NOT isolation, and the log line
        // is what makes the state visible to the operator. The test READS THE TRACE,
        // not only the response code.
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        const url = await servir('apps-non-attribuee');
        await poserVm(base!, 'v-1');
        const u = await createUser(base!, 'quiconque@exemple.test', 'x', MS);
        await poserApp(base!, 'v-1', 'Firefox', 'c-1');

        const r = await fetch(`${url}/applications?vm=v-1`, { headers: withIt(jetonDe(u)) });
        expect(r.status).toBe(200);
        expect(traces.join(' | ')).toContain('unassigned vm');
        expect(traces.join(' | ')).toContain('v-1');
    });

    it('returns 404 on an UNKNOWN application', async () => {
        const url = await servir('apps-lancer-inconnue');
        const u = await createUser(base!, 'u@exemple.test', 'x', MS);
        const r = await fetch(`${url}/application/jamais-vue/lancer`, {
            method: 'POST',
            headers: withIt(jetonDe(u)),
        });
        expect(r.status).toBe(404);
        expect(await r.json()).toEqual({ refus: 'application-inconnue' });
    });

    it('🔴 returns 503 when the agent is ABSENT, never 200', async () => {
        // 🔴 Returning 200 would make the hub display a success for a launch that
        // did NOT happen — the hardest failure to diagnose there
        // is, because nothing anywhere contradicts it.
        const url = await servir('apps-lancer-absent');
        await poserVm(base!, 'v-1');
        const u = await attribuer(base!, 'v-1', 'u@exemple.test');
        const id = await poserApp(base!, 'v-1', 'Firefox', 'c-1');

        const r = await fetch(`${url}/application/${id}/lancer`, {
            method: 'POST',
            headers: withIt(jetonDe(u)),
        });
        expect(r.status).toBe(503);
        expect(await r.json()).toEqual({ refus: 'agent-injoignable' });
    });

    it("🔴 returns 504 when the agent DOES NOT ANSWER, never 202 without waiting", async () => {
        // 🔴 Returning 202 without waiting would pass the acceptance criterion on
        // a binary that launched NOTHING: the platform would say "off it goes"
        // for an order whose outcome nobody ever saw.
        const url = await servir('apps-lancer-delai');
        await poserVm(base!, 'v-1');
        const u = await attribuer(base!, 'v-1', 'u@exemple.test');
        const id = await poserApp(base!, 'v-1', 'Firefox', 'c-1');
        // An agent registered, but SILENT.
        registre.inscrire('v-1', agentQuiRepond(null));

        const debut = Date.now();
        const r = await fetch(`${url}/application/${id}/lancer`, {
            method: 'POST',
            headers: withIt(jetonDe(u)),
        });
        expect(r.status).toBe(504);
        expect(await r.json()).toEqual({ refus: 'delai' });
        // ⚠️ AND IT REALLY WAITED: without this bound, a route that
        // returned 504 immediately would pass the test while having left
        // the agent no chance.
        expect(Date.now() - debut).toBeGreaterThanOrEqual(DELAI_LANCEMENT_MS - 50);
    }, 20_000);

    it("🔴 a successful launch returns THE OUTCOME, never a boolean", async () => {
        // 🔴 Flattening the outcome into a boolean would make the acceptance criterion lose
        // all discrimination: `raccourci` versus `cible` is what says whether
        // it is indeed the `.lnk` that was launched, or a rebuilt target.
        const url = await servir('apps-lancer-ok');
        await poserVm(base!, 'v-1');
        const u = await attribuer(base!, 'v-1', 'u@exemple.test');
        const id = await poserApp(base!, 'v-1', 'Firefox', 'c-1');
        registre.inscrire('v-1', agentQuiRepond('raccourci'));

        const r = await fetch(`${url}/application/${id}/lancer`, {
            method: 'POST',
            headers: withIt(jetonDe(u)),
        });
        expect(r.status).toBe(200);
        expect(await r.json()).toEqual({ issue: 'raccourci' });
    });

    it("a launch that FAILS on the agent side returns 200 and the outcome `echec`, never an HTTP error", async () => {
        // ⚠️ THE ORDER WENT THROUGH: the platform did its job, and the agent
        // answered. Returning a 5xx would say the SERVICE failed, which is
        // false — and telling it apart from `agent-injoignable` is the whole point
        // of having an outcome rather than a boolean.
        const url = await servir('apps-lancer-echec');
        await poserVm(base!, 'v-1');
        const u = await attribuer(base!, 'v-1', 'u@exemple.test');
        const id = await poserApp(base!, 'v-1', 'Firefox', 'c-1');
        registre.inscrire('v-1', agentQuiRepond('echec'));

        const r = await fetch(`${url}/application/${id}/lancer`, {
            method: 'POST',
            headers: withIt(jetonDe(u)),
        });
        expect(r.status).toBe(200);
        expect(await r.json()).toEqual({ issue: 'echec' });
    });

    it("🔴 launching an application of a FOREIGN VM answers like an UNKNOWN VM", async () => {
        // Without this guard, the application identifier would be enough to launch a
        // program on someone else's machine — and the agent, for its part,
        // has no way of knowing who asked.
        //
        // 🔴 THIS TEST WAS REVERSED, for the same reason as its twin of the
        // list: it pinned `403 {refus:'vm-etrangere'}`, the enumeration
        // oracle the repository owner ruled against. The
        // guard itself has not moved an inch — only the refusal it returns
        // changes, and the witness of its EFFECT is that the registered agent receives
        // nothing.
        //
        // 🔴 THE BODY IS COMPARED, never the status alone: `404` alone would be
        // returned by the generic 404 of `serveur.ts` as well as by
        // this one. The witness used is the refusal the OTHER route of this
        // file returns on a truly unknown VM — which at the same time tests
        // "a single reason, a single code" BETWEEN THE TWO ROUTES.
        //
        // ⚠️ WHY THE WITNESS COMES FROM THE OTHER ROUTE, AND NOT FROM THIS ONE:
        // `application.vm_id` carries `REFERENCES vm(id)` without `ON DELETE`
        // (migration 0003) — an application whose VM does not exist is
        // INSERTABLE nowhere, the constraint turns red. The `inconnue` verdict
        // is therefore UNREACHABLE on `/application/:id/lancer`: the only VM
        // this route can read is the one the application designates, and
        // it exists by construction. *(A first draft of this test
        // set an orphan application to serve as a witness; SQLite
        // refused it with `FOREIGN KEY constraint failed`, and it is that red that
        // corrected the assumption.)*
        const url = await servir('apps-lancer-etrangere');
        await poserVm(base!, 'v-1');
        await attribuer(base!, 'v-1', 'proprietaire@exemple.test');
        const autre = await createUser(base!, 'autre@exemple.test', 'x', MS);
        const id = await poserApp(base!, 'v-1', 'Firefox', 'c-1');
        registre.inscrire('v-1', agentQuiRepond('raccourci'));

        const entetes = withIt(jetonDe(autre));
        const etrangere = await fetch(`${url}/application/${id}/lancer`, {
            method: 'POST',
            headers: entetes,
        });
        const temoin = await fetch(`${url}/applications?vm=jamais-vue`, { headers: entetes });

        expect([etrangere.status, await etrangere.text()]).toEqual([
            temoin.status,
            await temoin.text(),
        ]);
        expect(etrangere.status).toBe(404);
    });

    it('refuses the METHOD on a path that exists, rather than a 404', async () => {
        // The path EXISTS, it is the method that does not fit: a 404
        // would send one looking for an absent route. Same choice as `routes-vm.ts`.
        const url = await servir('apps-methode');
        expect((await fetch(`${url}/applications`, { method: 'POST' })).status).toBe(405);
        expect((await fetch(`${url}/application/x/lancer`)).status).toBe(405);
    });

    it('requires the `vm` parameter, and says so', async () => {
        const url = await servir('apps-sans-vm');
        const u = await createUser(base!, 'u@exemple.test', 'x', MS);
        const r = await fetch(`${url}/applications`, { headers: withIt(jetonDe(u)) });
        expect(r.status).toBe(400);
        expect(await r.json()).toEqual({ refus: 'vm-absente' });
    });
});
