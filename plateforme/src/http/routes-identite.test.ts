import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { startServer, type ServicePlateforme } from './serveur';
import type { Config } from '../config';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { lireIdentitePomerium, ENTETE_IDENTITE } from './routes-identite';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    auth: 'pomerium',
    // The tests of this file connect over loopback: it is
    // the address `req.socket.remoteAddress` will really carry.
    proxyDeConfiance: new Set(['127.0.0.1']),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'moi-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'moi-tranches-')), 'televersements'),
};

describe('lireIdentitePomerium', () => {
    it('returns the email', () => {
        const v = lireIdentitePomerium({ [ENTETE_IDENTITE]: 'a@b.c' });
        expect(v).toEqual({ ok: true, email: 'a@b.c' });
    });

    it('trims the spaces', () => {
        const v = lireIdentitePomerium({ [ENTETE_IDENTITE]: '  a@b.c  ' });
        expect(v).toEqual({ ok: true, email: 'a@b.c' });
    });

    // 🔴 NO FALLBACK TO A DEFAULT USER: a bad proxy
    // configuration must be loud and refusing.
    it("refuses the absent header", () => {
        expect(lireIdentitePomerium({})).toEqual({ ok: false, motif: 'identite-absente' });
    });

    it("refuses the empty header", () => {
        expect(lireIdentitePomerium({ [ENTETE_IDENTITE]: '   ' })).toEqual({
            ok: false,
            motif: 'identite-absente',
        });
    });

    // Literal precedent of `porteur.ts`: choosing one would be taking a
    // decision an attacker exploits as soon as two layers do not take
    // the same one.
    it("REFUSES a REPEATED header, never disambiguates it", () => {
        expect(lireIdentitePomerium({ [ENTETE_IDENTITE]: ['a@b.c', 'mechant@x.y'] })).toEqual({
            ok: false,
            motif: 'identite-absente',
        });
    });
});

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

/// Counts the accounts. It is THE COUNT that says whether the upsert created once or
/// twice — never the mere absence of error.
async function combienDeComptes(p: Pilote): Promise<number> {
    return (await p.interroger<{ id: string }>('SELECT id FROM utilisateur', [])).length;
}

describe('GET /auth/moi', () => {
    it('① unknown account ⇒ 200, and the account is CREATED', async () => {
        base = await baseNeuve('moi-inconnu');
        service = await startServer(CONFIG, base);
        expect(await combienDeComptes(base)).toBe(0);

        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { 'x-pomerium-claim-email': 'a@b.c' },
        });

        expect(r.status).toBe(200);
        const corps = (await r.json()) as { acces?: unknown };
        expect(typeof corps.acces).toBe('string');
        expect(await combienDeComptes(base)).toBe(1);
    });

    it('② known account ⇒ 200, and NO extra account', async () => {
        base = await baseNeuve('moi-connu');
        service = await startServer(CONFIG, base);
        const url = `http://127.0.0.1:${service.port}/auth/moi`;
        const en = { 'x-pomerium-claim-email': 'a@b.c' };

        await fetch(url, { headers: en });
        const r = await fetch(url, { headers: en });

        expect(r.status).toBe(200);
        expect(await combienDeComptes(base)).toBe(1);
    });

    // 🔴 THE RED OF CRITERION ①: without `pass_identity_headers` at Pomerium,
    // the service REFUSES — it falls back to no default user.
    it('③ header absent ⇒ 401 identite-absente', async () => {
        base = await baseNeuve('moi-absent');
        service = await startServer(CONFIG, base);

        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`);

        expect(r.status).toBe(401);
        expect(await r.json()).toEqual({ refus: 'identite-absente' });
        expect(await combienDeComptes(base)).toBe(0);
    });

    // 🔴 THE GUARD THAT CLOSES THE BYPASS: BOTH ARMS, OTHERWISE THE 401
    // ALONE WOULD PROVE NOTHING — it would be indistinguishable from an entirely
    // broken route. The GREEN arm is ① and ② above, which connect over
    // loopback and succeed precisely because `CONFIG.proxyDeConfiance`
    // declares `127.0.0.1`.
    it("REFUSES (401) the identity header coming from an undeclared peer", async () => {
        base = await baseNeuve('moi-pair-etranger-statut');
        service = await startServer({ ...CONFIG, proxyDeConfiance: new Set(['10.9.9.9']) }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { [ENTETE_IDENTITE]: 'a@b.c' },
        });
        expect(r.status).toBe(401);
    });

    it("REFUSES the identity header coming from an undeclared peer, with the reason named", async () => {
        base = await baseNeuve('moi-pair-etranger-motif');
        service = await startServer({ ...CONFIG, proxyDeConfiance: new Set(['10.9.9.9']) }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { [ENTETE_IDENTITE]: 'a@b.c' },
        });
        const corps = (await r.json()) as { refus?: unknown };
        expect(corps.refus).toBe('pair-non-de-confiance');
    });

    // 🔴 CORRECTION ROUND 1 — THE GUARD AGAINST AN UNAUTHENTICATED ACCOUNT
    // CREATION, RESTORED AS A DISTINCT TEST ("one assertion
    // per test" convention). Removed once on the judgement that it "brought
    // nothing the status and the reason did not already say": FALSE, measured — by
    // moving the guard AFTER `identifiantDe`, the 401 and the reason stay
    // IDENTICAL and yet 3 accounts are created in `user` from an
    // undeclared peer, one per email chosen by the attacker. It is the
    // vector `CLAUDE.md` names in the `auth-pomerium` legacy §: "creates one
    // `user` row per distinct email, without bound".
    it("REFUSES the identity header coming from an undeclared peer, WITHOUT CREATING AN ACCOUNT", async () => {
        base = await baseNeuve('moi-pair-etranger-compte');
        service = await startServer({ ...CONFIG, proxyDeConfiance: new Set(['10.9.9.9']) }, base);
        await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { [ENTETE_IDENTITE]: 'attaquant@x.y' },
        });
        expect(await combienDeComptes(base)).toBe(0);
    });

    // 🔴 CORRECTION ROUND 1 — THE PROPERTY THIS TASK EXISTS TO
    // ESTABLISH, PINNED BY A TEST. Without it, replacing
    // `req.socket.remoteAddress` with the first hop of `X-Forwarded-For`
    // in `routes-identite.ts` leaves THE WHOLE SUITE GREEN (measured:
    // 62 files / 645 tests) and reopens the full bypass: an
    // UNDECLARED peer forges the header to pass itself off as a declared peer,
    // and receives a valid internal token. `pairDeConfiance` reads ONLY
    // `remoteAddress` by construction (`adresse-source.ts`); this test
    // tests that the ROUTE, in turn, is not convinced by
    // the header.
    it("IGNORES X-Forwarded-For: an undeclared peer forging the address of a declared peer stays REFUSED", async () => {
        base = await baseNeuve('moi-xff-forge');
        // The REAL peer of this test is `127.0.0.1` (loopback); the
        // trust declares ONLY `10.9.9.9`, the address the header will
        // claim to carry. If the guard read the header, `10.9.9.9` would be
        // recognised as trusted and the request would succeed.
        service = await startServer({ ...CONFIG, proxyDeConfiance: new Set(['10.9.9.9']) }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { [ENTETE_IDENTITE]: 'a@b.c', 'x-forwarded-for': '10.9.9.9' },
        });
        expect(r.status).toBe(401);
    });

    // 🔴 THE REDS OF CRITERIA ② AND ③ IN A SINGLE TEST, AND THE HEADER IS
    // PRESENT ON PURPOSE: it is what proves a FORGED header is ignored
    // in `motdepasse` mode, and not only that the route is absent.
    it('④ motdepasse mode ⇒ 404, forged header IGNORED', async () => {
        base = await baseNeuve('moi-motdepasse');
        service = await startServer({ ...CONFIG, auth: 'motdepasse' }, base);

        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { 'x-pomerium-claim-email': 'forge@x.y' },
        });

        expect(r.status).toBe(404);
        expect(await combienDeComptes(base)).toBe(0);
    });
});
