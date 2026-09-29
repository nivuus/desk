// The security headers, tested ON EACH ROUTER separately.
//
// 🔴 ONE `it()` PER ROUTER, AND NEVER A GLOBAL TEST. A single test that
// swept "at least one route carries them" would pass as soon as ONE SINGLE router
// sets them — and it is exactly the lesson P2's red ①A paid for:
// `expect` interrupts the test at the first assertion that fails, so
// that a grouped assertion only tests its first line. The red of this
// task consists in removing the spread of ONE SINGLE router, and checking that
// ITS test fails while the other four stay green.
//
// ⚠️ THIS FILE EXISTS RATHER THAN FIVE BLOCKS SCATTERED ACROSS THE FIVE
// ROUTE TEST FILES, and it is an acknowledged divergence from P5's plan (which
// planned "a few assertions" in `routes-vm.test.ts` and
// `routes-session.test.ts`). The property tested is CROSS-CUTTING — "every
// JSON response of the service" —, and a cross-cutting property scattered across five
// places is one a sixth router will never join. G1 has just
// added a router without anyone noticing on P5's side: it is
// precisely the failure mode this file makes visible.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { createUser } from '../depot/utilisateur';
import { hacher } from '../identite/mot-de-passe';
import { ENTETES_SECURITE } from './entetes';
import { startServer, type ServicePlateforme } from './serveur';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const MOT_DE_PASSE = 'un-mot-de-passe-ordinaire-42';
const MS = 1_787_136_773_742;

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
    // 🔴 TASK 3: `servirAuth` now WITHDRAWS itself in `pomerium` mode — see
    // its guard. THREE cases of this file go through `/auth/connexion`
    // ((1) GET→405, (6) POST→200, (8) OPTIONS→204) and therefore require
    // `auth: 'motdepasse'`, otherwise they would meet a 404 instead of the
    // response of `routes-auth`.
    //
    // ⚠️ "THE GENERIC 404" WAS THE WORD UNTIL 22 AUGUST 2026, AND IT NO LONGER
    // IS: the mode guard now returns this 404 ITSELF
    // (`http/introuvable.ts`), because the page server's SPA fallback
    // swallowed it. The body and headers are the same to the byte — what
    // changes is WHO answers, and these three cases do not notice it.
    //
    // ⚠️ THIS COMMENT WROTE "THE FIVE OTHERS", AND THE COUNT WAS WRONG
    // (cross-cutting review, 21 August 2026) — in a comment whose whole purpose
    // is completeness. This file carries **ELEVEN** `it()`, not eight: the three
    // numbered OUT OF SEQUENCE — `(6bis)`, `(6ter)`, `(6quater)`, added by G2
    // and G3 — had not been counted. **Three** cases take `motdepasse`,
    // so **EIGHT OTHERS** are indifferent to this value. Recounted by
    // `grep -c '^\s*it(' <this file>` -> `11`, and by
    // `grep -c "^\s*const url = await servir(.*motdepasse" <this file>` ->
    // `3`. 🔴 **These
    // two commands are rerun; this count is not copied** — it is the
    // "487 wreck" of `docs/claude/pitfalls-docs-size-vm.md`, and the out-of-sequence numbering is
    // its mechanical cause here.
    //
    // 🔴 THE SECOND COMMAND IS ANCHORED AT LINE START ON `const url =
    // await servir(`, AND IT IS THE THIRD DRAFT: THE FIRST TWO
    // POLLUTED THEMSELVES, AND THE MEASUREMENT SAID SO EACH TIME.
    //   ① `grep -c "motdepasse' }"` -> **5** instead of 3: it counted the
    //      lines of THIS comment, which quotes the pattern.
    //   ② `grep -c "servir(.*auth: 'motdepasse'"` -> **4** instead of 3: it
    //      counted **its own quotation**, two lines above.
    // The anchor `^\s*const` is the only form that cannot count itself,
    // a comment line always starting with `//`. A file that
    // documents its invariants CONTAINS the patterns it describes — a home-grown trap
    // of `CLAUDE.md`, paid TWICE more right here.
    //
    // ⚠️ NO `it()` TARGETS `/auth/moi`, and the check that established it
    // had EXACTLY the same defect: "`grep -n 'auth/moi'` returns nothing"
    // was true when written, and false as soon as the sentence quoting it entered
    // the file. The check that matters DISCARDS comments —
    // `grep -n 'auth/moi' <this file> | grep -vc '^\s*[0-9]*:\s*//'` must
    // return **0**.
    //
    // **Decision, made case by case and not in bulk**: the SHARED `CONFIG`
    // stays at `pomerium` (the product default, `config.ts`), and THE ONLY
    // THREE cases that need it receive `{ ...CONFIG, auth:
    // 'motdepasse' }` locally — never the reverse, which would have changed the
    // mode of the EIGHT others for a reason that does not concern them.
    //
    // ⚠️ THIS SENTENCE SAID "... including for a future /auth/moi test
    // that would join this file without rereading it", AND IT HAS BECOME FALSE
    // (task 6, review "correction round 1", 22 August 2026): this file
    // now receives WITHOUT SAYING SO A SECOND TRAP. `CONFIG.proxyDeConfiance`
    // is `new Set()` — nobody is declared trusted —, and the guard
    // of `routes-identite.ts` then refuses EVERY peer, including the loopback
    // from which these tests connect. A future `/auth/moi` test
    // added here WITHOUT REREADING THIS COMMENT would receive `401
    // pair-non-de-confiance` whatever the identity header set, and
    // would have to set `proxyDeConfiance: new Set(['127.0.0.1'])` locally
    // to observe anything other than this guard. Code untouched: this
    // file still builds no request to `/auth/moi`.
    auth: 'pomerium',
};

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

async function servir(nom: string, config: Config = CONFIG): Promise<string> {
    base = await baseNeuve(nom);
    await createUser(base, 'ada@exemple.test', await hacher(MOT_DE_PASSE), MS);
    service = await startServer(config, base);
    return `http://127.0.0.1:${service.port}`;
}

/// Checks BOTH headers, by name, on a response.
function porteLesEntetes(r: Response, quoi: string): void {
    for (const [cle, value] of Object.entries(ENTETES_SECURITE)) {
        expect(r.headers.get(cle.toLowerCase()), `${quoi}: header ${cle}`).toBe(value);
    }
}

describe('the security headers, one router at a time', () => {
    it('(1) `routes-auth` sets them — including on an ERROR response', async () => {
        // ⚠️ `auth: 'motdepasse'` LOCAL: without it, `servirAuth` WITHDRAWS
        // (task 3) and this request would meet the generic 404, never
        // the 405 of `routes-auth`.
        const url = await servir('entetes-auth', { ...CONFIG, auth: 'motdepasse' });
        // ⚠️ ON AN ERROR, and it is deliberate: an error response often carries
        // MORE information than a normal response, and it is the one
        // a hasty fix would forget.
        const r = await fetch(`${url}/auth/connexion`, { method: 'GET' });
        expect(r.status).toBe(405);
        porteLesEntetes(r, '405 of /auth/connexion');
    });

    it('(2) `routes-vm` sets them', async () => {
        const url = await servir('entetes-vm');
        const r = await fetch(`${url}/vm`);
        // 401: no bearer presented. The error response carries the headers.
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 of /vm');
    });

    it('(3) `routes-session` sets them', async () => {
        const url = await servir('entetes-session');
        const r = await fetch(`${url}/session`, { method: 'POST' });
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 of /session');
    });

    it('(4) `routes-applications` sets them — the FIFTH router, added by G1', async () => {
        // ⚠️ THIS ROUTER IS NOT IN P5'S PLAN, which counts "the four
        // routers". G1 shipped it between the writing of the plan and its
        // execution. Without this `it()`, the property "every JSON response of the
        // service" would be FALSE on the very day it shipped.
        const url = await servir('entetes-applications');
        const r = await fetch(`${url}/applications`);
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 of /applications');
    });

    it('(5) `routes-sante` sets them', async () => {
        const url = await servir('entetes-sante');
        const r = await fetch(`${url}/sante`);
        expect(r.status).toBe(200);
        porteLesEntetes(r, '200 of /sante');
    });

    it('(6) 🔴 the 200 response of `/auth/connexion` carries `Cache-Control: no-store`', async () => {
        // 🔴 IT IS THE ONLY RESPONSE OF THE SERVICE THAT CARRIES TOKENS — the access
        // AND refresh tokens, in clear in its JSON body. An intermediate
        // cache, or simply the browser's disk, would
        // keep them. A generic test that only tested the error
        // responses would miss this one, which is the only one that really
        // matters.
        // ⚠️ `auth: 'motdepasse'` LOCAL — see case (1): without it, this
        // route does not exist and the request would return 404, not 200.
        const url = await servir('entetes-jetons', { ...CONFIG, auth: 'motdepasse' });
        const r = await fetch(`${url}/auth/connexion`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ email: 'ada@exemple.test', motdepasse: MOT_DE_PASSE }),
        });
        expect(r.status).toBe(200);
        expect(r.headers.get('cache-control')).toBe('no-store');
        porteLesEntetes(r, '200 of /auth/connexion');
    });

    it('(6bis) `routes-icone` sets them — THE SIXTH ROUTER', async () => {
        // 🔴 G2 ADDS THE SIXTH ROUTER, and the header of this file names the
        // precedent: "G1 has just added a router without anyone
        // noticing on P5's side". Do not replay the defect this file
        // exists to prevent.
        const url = await servir('entetes-icone');
        // Without a token: a 401, hence an ERROR response — the one a
        // hasty fix would forget.
        const r = await fetch(`${url}/icone/${'a'.repeat(64)}`, { method: 'PUT' });
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 of /icone/:sha256');

        // And on the other path of this same router.
        //
        // ⚠️ THIS PATH NO LONGER RETURNS 401 BUT 400 SINCE 30 AUGUST 2026, and
        // it is not a regression: the `GET` no longer requires `Authorization`
        // — it requires a SIGNED URL (decision of the repository owner, see
        // `routes-icone.ts`). Without the signature parameters, the request is
        // INCOMPLETE, not unauthenticated. **What this test tests is
        // unchanged**: that the security headers are set on an
        // ERROR response of this router.
        const g = await fetch(`${url}/application/x/icone?e=${'a'.repeat(64)}`);
        expect(g.status).toBe(400);
        porteLesEntetes(g, '400 of /application/:id/icone');
    });

    it('(6ter) `routes-televersement` sets them — THE SEVENTH ROUTER', async () => {
        // 🔴 G3 ADDS THE SEVENTH, and the header of this file names the two
        // previous ones: G1 shipped the fifth "without anyone
        // noticing on P5's side", G2 the sixth. It is the third time, and the
        // file only exists so that it is the last.
        const url = await servir('entetes-televersement');
        // Without a token: an ERROR response, the one a hasty fix
        // would forget — and on the STATE route, which is the only one of the four
        // a bodiless `GET` reaches.
        const r = await fetch(`${url}/televersement/inexistant`);
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 of /televersement/:id');
    });

    it('(6quater) `routes-installation` sets them — THE EIGHTH ROUTER', async () => {
        // ⚠️ THIS ONE ONLY SERVES THE AGENT, and its refusal therefore goes through the
        // AGENT bearer and not the user's. Two different
        // guards, one cross-cutting property: it is precisely the
        // kind of gap through which a router escapes a sweep.
        const url = await servir('entetes-installation');
        const r = await fetch(`${url}/televersement/inexistant/contenu`);
        expect(r.status).toBe(401);
        porteLesEntetes(r, '401 of /televersement/:id/contenu');
    });

    it('(7) the generic 404 and the 500 carry them too', async () => {
        // The 404 comes from no router: it is written in `serveur.ts`.
        // Without it, an unknown path would be the only response of the service not
        // carrying `nosniff`.
        const url = await servir('entetes-404');
        const r = await fetch(`${url}/path-that-does-not-exist`);
        expect(r.status).toBe(404);
        porteLesEntetes(r, 'generic 404');
    });

    it('(8) the OPTIONS preflight response carries them too', async () => {
        // ⚠️ `auth: 'motdepasse'` LOCAL — see case (1): in `pomerium` mode,
        // `servirAuth` withdraws BEFORE even its OPTIONS branch (the guard
        // precedes the whole rest of the function), and this OPTIONS would meet
        // the generic 404 instead of the preflight 204.
        const url = await servir('entetes-options', { ...CONFIG, auth: 'motdepasse' });
        const r = await fetch(`${url}/auth/connexion`, { method: 'OPTIONS' });
        expect(r.status).toBe(204);
        porteLesEntetes(r, '204 preflight');
    });
});
