import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, piloteCompteur } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { CacheSante, PERIODE_SANTE_MS, servirSante } from './routes-sante';
import { startServer, type ServicePlateforme } from './serveur';
import type { Config } from '../config';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
/// A real epoch, on the pattern of `base/harnais.ts`: small values
/// measure nothing.
const T0 = 1_787_000_000_000;

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
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

/// A driver where EVERYTHING throws — to test the 503 without breaking a real database.
function piloteMort(): Pilote {
    const mort: Pilote = {
        async interroger() {
            throw new Error('database unreachable');
        },
        async executer() {
            throw new Error('database unreachable');
        },
        async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
            return corps(mort);
        },
        async fermer() {},
    };
    return mort;
}

describe('GET /sante', () => {
    it('(a) healthy database ⇒ 200 and `{"etat":"ok"}`', async () => {
        base = await baseNeuve('sante-ok');
        service = await startServer(CONFIG, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`);
        expect(r.status).toBe(200);
        expect(await r.json()).toEqual({ etat: 'ok' });
    });

    it('(b) database failing ⇒ 503 and `{"etat":"degrade"}`', async () => {
        service = await startServer(CONFIG, piloteMort());
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`);
        expect(r.status).toBe(503);
        expect(await r.json()).toEqual({ etat: 'degrade' });
    });

    it("(c) 🔴 the response carries NOTHING ELSE", async () => {
        // 🔴 COMPARISON OF THE WHOLE OBJECT, never a `toContain`: a chatty
        // health page is an INVENTORY offered to an anonymous peer.
        // A future addition of a version, a session count, a base URL or
        // an engine name must turn this test RED.
        // ⚠️ "THE ONLY UNAUTHENTICATED ROUTE LEFT AFTER P2" WAS
        // WRITTEN HERE, AND IT HAS BEEN FALSE SINCE 22 AUGUST 2026: the page server
        // is a SECOND one. What remains true — and is the reason for this
        // test — is that `/sante` is the only unauthenticated route that
        // TOUCHES THE DATABASE, hence the only one whose response could say
        // something about it. See `routes-sante.ts`.
        base = await baseNeuve('health-nothing-else');
        service = await startServer(CONFIG, base);
        const corps = (await (await fetch(`http://127.0.0.1:${service.port}/sante`)).json()) as Record<string, unknown>;
        expect(Object.keys(corps)).toEqual(['etat']);
    });

    it('(d) 🔴 N calls within the period make ONLY ONE query', async () => {
        // 🔴 WITHOUT THE CACHE, `/sante` TRANSLATES AN ANONYMOUS HTTP REQUEST INTO AN
        // SQL QUERY, AT WILL: it is an amplification, on the very route
        // a load balancer calls in a loop. The cache is THE POINT
        // of this route, not a refinement.
        const reel = await baseNeuve('sante-cache');
        base = reel;
        const compteur = piloteCompteur(reel);
        const cache = new CacheSante();
        let horloge = T0;
        const deps = { base: compteur.pilote, maintenant: () => horloge, cache };
        const N = 5;
        for (let i = 0; i < N; i++) {
            expect(await cache.verdict(deps.base, deps.maintenant())).toBe(true);
        }
        expect(compteur.acces()).toBe(1);
    });

    it('(e) after the period, a NEW query takes place', async () => {
        const reel = await baseNeuve('sante-cache-expire');
        base = reel;
        const compteur = piloteCompteur(reel);
        const cache = new CacheSante();
        expect(await cache.verdict(compteur.pilote, T0)).toBe(true);
        expect(compteur.acces()).toBe(1);
        // Just under the period: still the cached verdict.
        expect(await cache.verdict(compteur.pilote, T0 + PERIODE_SANTE_MS - 1)).toBe(true);
        expect(compteur.acces()).toBe(1);
        // Beyond: we ask again.
        expect(await cache.verdict(compteur.pilote, T0 + PERIODE_SANTE_MS + 1)).toBe(true);
        expect(compteur.acces()).toBe(2);
    });

    it('(d bis) 🔴 N CONCURRENT calls make ONLY ONE query either', async () => {
        // The real case of a load balancer: several probes in flight at the
        // same instant. Without deduplication of the IN-FLIGHT query, each one
        // would launch its own — and the cache would only serve afterwards,
        // that is, never under the load it exists to absorb.
        const reel = await baseNeuve('sante-cache-concurrent');
        base = reel;
        const compteur = piloteCompteur(reel);
        const cache = new CacheSante();
        const all = await Promise.all(
            Array.from({ length: 8 }, () => cache.verdict(compteur.pilote, T0)),
        );
        expect(all).toEqual(Array.from({ length: 8 }, () => true));
        expect(compteur.acces()).toBe(1);
    });

    it('(f) a method other than GET returns 405', async () => {
        base = await baseNeuve('sante-methode');
        service = await startServer(CONFIG, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`, { method: 'POST' });
        expect(r.status).toBe(405);
        expect(await r.json()).toEqual({ refus: 'methode' });
    });

    it("(g) `/sante` is NOT authenticated — no token is required", async () => {
        // ⚠️ Deliberate, and it is what makes the cache mandatory: a load balancer
        // probe presents no token, and a BRAKED probe
        // would declare the service dead. The cache is what makes it safe WITHOUT
        // a brake — both decisions live in the same paragraph.
        base = await baseNeuve('sante-anonyme');
        service = await startServer(CONFIG, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`);
        expect(r.status).toBe(200);
    });

    it("(h) a neighbouring path is NOT served by the health route", async () => {
        // EXACT comparison, never a `startsWith`: `/santelle` is not
        // `/sante`, and a prefix would open a family of paths that
        // nobody decided.
        base = await baseNeuve('sante-chemin');
        service = await startServer(CONFIG, base);
        expect((await fetch(`http://127.0.0.1:${service.port}/santelle`)).status).toBe(404);
    });

    it('(i) `servirSante` returns `false` on a path that is not its own', async () => {
        // The convention of the four routers: returning `false` lets the next one
        // try, and the generic 404 concludes.
        const req = { url: '/autre', method: 'GET', headers: {} } as never;
        const rep = { writeHead() {}, end() {}, setHeader() {} } as never;
        base = await baseNeuve('sante-faux');
        expect(await servirSante(req, rep, { base, maintenant: () => T0, cache: new CacheSante() }))
            .toBe(false);
    });
});
