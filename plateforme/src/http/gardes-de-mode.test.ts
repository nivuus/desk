// THE TWO MODE GUARDS, TESTED IN THEIR INACTIVE MODE WITH THE PAGE ARMED.
//
// 🔴 IT IS THE COMPOSITION NOTHING TESTED, AND IT IS WHAT TURNS RED. The
// two guards were tested, the page server was tested, and the defect
// lived EXACTLY at their boundary: each half was right, and no
// per-task review could see it. `routes-identite.test.ts` and
// `routes-auth.test.ts` never set `racinePage`; `routes-page.test.ts`
// never visits `/auth/*`.
//
// 🔴 MEASURED BEFORE THE FIX, service in `motdepasse` mode with the page armed:
// `/auth/moi` returned `200 text/html` instead of `404`. The page server's
// SPA fallback, chained last, swallowed the generic `404` the two guards
// called — and that `404` is the documented mechanism that carries the MODE
// to the client (`client/src/connexion.ts`: "a 404 means this setup
// authenticates by password").
//
// ⚠️ THE METHOD IS `GET`, AND IT IS WHAT MAKES THESE TESTS DISCRIMINATING. The
// page server withdraws outside `GET`/`HEAD`, so that a `POST /auth/connexion`
// returned `404` anyway — by accident, and proving nothing. It is the
// `GET` that goes through the SPA fallback, hence the only one that can turn red.

import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { startServer, type ServicePlateforme } from './serveur';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: 'un-secret-de-plateforme-de-quarante-octets',
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'garde-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'garde-tranches-')), 'televersements'),
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

/// A root built by hand, carrying the `hub.html` of the SPA fallback: without
/// it, the page server would fail at `stat` and the `404` would come back for a
/// reason that is NOT the one under test.
///
/// 🔴 `hub.html`, NOT `index.html`, since the decision "serve the hub at the
/// root" (30 August 2026): it is now THIS name that `resolution.ts::PAGE`
/// resolves for the root and for every path without extension. Writing the other
/// name would, if a mode guard regressed, let the page server fail at
/// `stat` for a reason that is not the one under test — exactly the
/// trap this comment denounces.
function racineArmee(): string {
    const racine = mkdtempSync(join(tmpdir(), 'garde-page-'));
    writeFileSync(join(racine, 'hub.html'), '<!doctype html><title>page</title>');
    return racine;
}

function requeteFermee(url: string): Promise<Response> {
    return fetch(url, { headers: { Connection: 'close' } });
}

async function servir(auth: 'pomerium' | 'motdepasse', nom: string): Promise<string> {
    base = await baseNeuve(nom);
    service = await startServer({ ...CONFIG, auth, racinePage: racineArmee() }, base);
    return `http://127.0.0.1:${service.port}`;
}

describe("the mode guard of `/auth/moi`, in motdepasse mode", () => {
    it('returns 404, never the page', async () => {
        const url = await servir('motdepasse', 'garde-moi-statut');
        expect((await requeteFermee(`${url}/auth/moi`)).status).toBe(404);
    });

    // 🔴 SEPARATE, AND ANCHORED ON THE TYPE: `expect` stops at the first failure,
    // and this assertion would turn red EVEN if a future defect returned the right
    // status with the wrong body. It is what says the `404` comes from
    // the guard and not from a page server that would have failed for another reason.
    it("returns the 404 of the service — text/plain, never text/html", async () => {
        const url = await servir('motdepasse', 'garde-moi-type');
        const r = await requeteFermee(`${url}/auth/moi`);
        expect(r.headers.get('content-type')).toContain('text/plain');
    });

    it('returns the body of the 404 of the service, word for word', async () => {
        const url = await servir('motdepasse', 'garde-moi-corps');
        expect(await (await requeteFermee(`${url}/auth/moi`)).text()).toBe('not found\n');
    });
});

describe("the mode guard of `/auth/connexion`, in pomerium mode", () => {
    // 🔴 THE SYMMETRICAL TWIN. The two guards have OPPOSITE polarities and
    // PARTITION the modes: they therefore have the same defect, each in
    // the other mode. Testing only one would leave the other whole.
    it('returns 404, never the page', async () => {
        const url = await servir('pomerium', 'garde-connexion-statut');
        expect((await requeteFermee(`${url}/auth/connexion`)).status).toBe(404);
    });

    it("returns the 404 of the service — text/plain, never text/html", async () => {
        const url = await servir('pomerium', 'garde-connexion-type');
        const r = await requeteFermee(`${url}/auth/connexion`);
        expect(r.headers.get('content-type')).toContain('text/plain');
    });

    it('returns the body of the 404 of the service, word for word', async () => {
        const url = await servir('pomerium', 'garde-connexion-corps');
        expect(await (await requeteFermee(`${url}/auth/connexion`)).text()).toBe('not found\n');
    });

    it('`/auth/rafraichir` is guarded the same way', async () => {
        const url = await servir('pomerium', 'garde-rafraichir');
        expect((await requeteFermee(`${url}/auth/rafraichir`)).status).toBe(404);
    });
});

// 🔴 THE NEGATIVE WITNESS OF THIS WHOLE FILE. Without it, the seven `404`s above
// would be returned identically by a page that was NOT armed — hence
// by a setup where the defect does not exist. This test proves that the root
// used above really serves something.
describe('the page is indeed armed in this setup', () => {
    it('GET / returns 200 on the same configuration', async () => {
        const url = await servir('motdepasse', 'garde-temoin-negatif');
        expect((await requeteFermee(`${url}/`)).status).toBe(200);
    });
});
