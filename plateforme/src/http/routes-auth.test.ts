// The two authentication routes, tested THROUGH the real HTTP server.
//
// 🔴 CRITERION ④ IS IN THIS FILE, and its check of the check was played:
// a `console.log(JSON.stringify(corps))` deliberately added in the route
// turns it red. A sweep test that did not really capture the console
// would be green whatever happened — the pattern of the vacuous check, paid four
// times by this repository.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, piloteCompteur } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { hacher } from '../identite/mot-de-passe';
import { verifyToken } from '../identite/jeton';
import { createUser } from '../depot/utilisateur';
import { startServer, type ServicePlateforme } from './serveur';
import { ECHECS_MAX_ADRESSE, ECHECS_MAX_COMPTE } from '../securite/frein';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const ORIGINE = 'http://127.0.0.1:5173';
const MOT_DE_PASSE = 'un-mot-de-passe-ordinaire-42';
const MS = 1_787_136_773_742;

function config(origineClient?: string): Config {
    return {
        hote: '127.0.0.1',
        port: 0,
        base: 'sqlite',
        urlBase: ':memory:',
        secretJeton: SECRET,
        origineClient,
        // No proxy declared: see `config.ts`, the empty set is the default
        // and means "trust nobody's announced address".
        proxyDeConfiance: new Set(),
        repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
        repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
        // ⚠️ THIS FILE TESTS THE TWO PASSWORD ROUTES THEMSELVES:
        // `auth: 'motdepasse'`, otherwise `servirAuth` would return `false` for
        // EVERY request (see its guard), and all the tests of this file
        // would collapse for the wrong reason — a withdrawn router, not
        // a faulty router.
        auth: 'motdepasse',
    };
}

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

async function servir(nom: string, origineClient?: string): Promise<string> {
    base = await baseNeuve(nom);
    await createUser(base, 'ada@exemple.test', await hacher(MOT_DE_PASSE), MS);
    service = await startServer(config(origineClient), base);
    return `http://127.0.0.1:${service.port}`;
}

/// `Response.json()` returns `unknown`: this small typing avoids scattering
/// type assertions across the tests, without asserting anything about the content.
interface Paire { acces: string; rafraichissement: string; expire_a: number; refus?: string }
async function corpsDe(r: Response): Promise<Paire & Record<string, unknown>> {
    return (await r.json()) as Paire & Record<string, unknown>;
}

function poster(url: string, corps: unknown, entetes: Record<string, string> = {}) {
    return fetch(url, {
        method: 'POST',
        headers: { 'content-type': 'application/json', ...entetes },
        body: typeof corps === 'string' ? corps : JSON.stringify(corps),
    });
}

describe('authentication routes', () => {
    it('valid sign-in: the access is a token the service accepts, and the refresh is valid', async () => {
        const base_ = await servir('auth-ok');
        const r = await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        });
        expect(r.status).toBe(200);
        const corps = await corpsDe(r);
        expect(typeof corps.acces).toBe('string');
        expect(typeof corps.rafraichissement).toBe('string');
        expect(corps.expire_a).toBeGreaterThan(Date.now());
        // The returned token is really verifiable by this service.
        expect(verifyToken(corps.acces, SECRET, Date.now()).ok).toBe(true);

        // And the refresh rotates.
        const r2 = await poster(`${base_}/auth/rafraichir`, {
            rafraichissement: corps.rafraichissement,
        });
        expect(r2.status).toBe(200);
        const corps2 = await corpsDe(r2);
        expect(corps2.rafraichissement).not.toBe(corps.rafraichissement);
    });

    it('wrong password and unknown email return the SAME message', async () => {
        // 🔴 A message that tells them apart is an account ENUMERATION
        // oracle: the attacker learns which addresses exist.
        const base_ = await servir('auth-refus');
        const faux = await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: 'ce-n-est-pas-le-bon-mot-de-passe',
        });
        const inconnu = await poster(`${base_}/auth/connexion`, {
            email: 'personne@exemple.test',
            motdepasse: MOT_DE_PASSE,
        });
        expect(faux.status).toBe(401);
        expect(inconnu.status).toBe(401);
        expect(await corpsDe(faux)).toEqual(await corpsDe(inconnu));
        const troisieme = await corpsDe(await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: 'encore-faux',
        }));
        expect(troisieme.refus).toBe('identifiants');
    });

    it('REPLAY: refreshing the same token twice returns 401, and the family falls', async () => {
        const base_ = await servir('auth-rejeu');
        const { rafraichissement } = await corpsDe(await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        }));
        const premier = await corpsDe(await poster(`${base_}/auth/rafraichir`, { rafraichissement }));

        const rejeu = await poster(`${base_}/auth/rafraichir`, { rafraichissement });
        expect(rejeu.status).toBe(401);
        expect((await corpsDe(rejeu)).refus).toBe('rejeu');

        // The NEW token, which the thief would hold, is dead too.
        const neuf = await poster(`${base_}/auth/rafraichir`, {
            rafraichissement: premier.rafraichissement,
        });
        expect(neuf.status).toBe(401);
    });

    it('bounds the body at 4 KiB: 5 KiB returns 413', async () => {
        // Without a bound, an anonymous peer grows memory at will.
        const base_ = await servir('auth-gros');
        const r = await poster(`${base_}/auth/connexion`, JSON.stringify({
            email: 'ada@exemple.test',
            motdepasse: 'x'.repeat(5 * 1024),
        })).catch(() => undefined);
        // The connection may be cut before the response: both outcomes
        // are acceptable, what matters is that no 200 comes out.
        if (r) expect(r.status).toBe(413);
    });

    it('refuses a method other than POST with 405, and a non-JSON body with 400', async () => {
        const base_ = await servir('auth-methode');
        const g = await fetch(`${base_}/auth/connexion`);
        expect(g.status).toBe(405);
        const mauvais = await poster(`${base_}/auth/connexion`, 'this-is-not-json');
        expect(mauvais.status).toBe(400);
        expect((await corpsDe(mauvais)).refus).toBe('forme');
    });

    it('OPTIONS returns 204, with the CORS headers ONLY if an origin is allowed', async () => {
        const withIt = await servir('auth-cors-oui', ORIGINE);
        const r = await fetch(`${withIt}/auth/connexion`, {
            method: 'OPTIONS',
            headers: { origin: ORIGINE },
        });
        expect(r.status).toBe(204);
        expect(r.headers.get('access-control-allow-origin')).toBe(ORIGINE);
        expect(r.headers.get('vary')).toBe('Origin');
        await service!.close();
        service = undefined;
        await base!.fermer();
        base = undefined;

        const sans = await servir('auth-cors-non');
        const r2 = await fetch(`${sans}/auth/connexion`, {
            method: 'OPTIONS',
            headers: { origin: ORIGINE },
        });
        expect(r2.status).toBe(204);
        // The default is refusal: no header, never `*`.
        expect(r2.headers.get('access-control-allow-origin')).toBeNull();
    });

    it('leaves the 404 of P1 intact on a path that is not an authentication one', async () => {
        // ⚠️ Nothing tested this 404: changing it would be an undeclared
        // side effect.
        const base_ = await servir('auth-404');
        const r = await fetch(`${base_}/autre-chose`);
        expect(r.status).toBe(404);
        expect(await r.text()).toBe('not found\n');
    });

    it('CRITERION ④: no trace and no response carries the password FIELD', async () => {
        // 🔴 The sweep looks for THE FIELD, not the value: a log of the WHOLE
        // request BODY would make the password appear without the
        // exact string being searched anywhere.
        const base_ = await servir('auth-trace');
        const capture: string[] = [];
        for (const canal of ['log', 'warn', 'error'] as const) {
            vi.spyOn(console, canal).mockImplementation((...a: unknown[]) => {
                capture.push(a.map(String).join(' '));
            });
        }

        const reussie = await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        });
        const echouee = await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: 'ce-n-est-pas-le-bon',
        });

        const balai = /motdepasse|mot_de_passe|empreinte_mdp/i;
        expect(capture.join('\n')).not.toMatch(balai);
        expect(await reussie.text()).not.toMatch(balai);
        expect(await echouee.text()).not.toMatch(balai);
        // And the check CAN fail: the capture does catch the console.
        console.log('capture witness');
        expect(capture.join('\n')).toContain('capture witness');
    });
});

describe('the authentication mode (Config.auth)', () => {
    // 🔴 THE RED OF CRITERION ③, IN BOTH DIRECTIONS. A single direction would leave
    // the other route alive in the wrong mode: `/auth/connexion` open
    // behind Pomerium would be a SECOND door, with a password nobody
    // rotates any more.
    //
    // ⚠️ ADAPTED TO THIS FILE'S SETUP (`poster`/`startServer`), not to the
    // brief's template that called `servirAuth` directly: this file
    // tests the routes THROUGH the real HTTP server, never the bare router
    // (see the file header), and `false` shows there as the generic
    // 404 that `serveur.ts` returns when no router has served.
    it('returns 404 on /auth/connexion in pomerium mode — the route NO LONGER EXISTS', async () => {
        base = await baseNeuve('auth-mode-connexion-pomerium');
        await createUser(base, 'ada@exemple.test', await hacher(MOT_DE_PASSE), MS);
        service = await startServer({ ...config(), auth: 'pomerium' }, base);
        const url = `http://127.0.0.1:${service.port}`;
        const r = await poster(`${url}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        });
        expect(r.status).toBe(404);
    });

    it('returns 404 on /auth/rafraichir in pomerium mode — the route NO LONGER EXISTS', async () => {
        base = await baseNeuve('auth-mode-rafraichir-pomerium');
        service = await startServer({ ...config(), auth: 'pomerium' }, base);
        const url = `http://127.0.0.1:${service.port}`;
        const r = await poster(`${url}/auth/rafraichir`, { rafraichissement: 'un-jeton-quelconque' });
        expect(r.status).toBe(404);
    });

    // The witness that makes the two previous ones interpretable: in
    // motdepasse mode, the route ALWAYS serves. Without it, a 404 could come from an
    // entirely unplugged service rather than from the mode guard itself.
    it('ALWAYS serves /auth/connexion in motdepasse mode (witness)', async () => {
        const url = await servir('auth-mode-motdepasse-temoin');
        const r = await poster(`${url}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        });
        expect(r.status).toBe(200);
    });
});

describe('the brake of the authentication routes', () => {
    /// Like `servir`, but ALSO returns the database access counter.
    async function servirCompte(
        nom: string,
        origineClient?: string,
    ): Promise<{ url: string; acces: () => number; remettre: () => void }> {
        const reel = await baseNeuve(nom);
        base = reel;
        await createUser(reel, 'ada@exemple.test', await hacher(MOT_DE_PASSE), MS);
        const compteur = piloteCompteur(reel);
        service = await startServer(config(origineClient), compteur.pilote);
        return { url: `http://127.0.0.1:${service.port}`, acces: compteur.acces, remettre: compteur.remettre };
    }

    function echouer(url: string, email: string) {
        return poster(`${url}/auth/connexion`, { email, motdepasse: 'ce-n-est-pas-le-bon' });
    }

    it('(a) the n+1th attempt on the SAME account is refused by the brake', async () => {
        const url = await servir('frein-compte');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            expect((await echouer(url, 'ada@exemple.test')).status).toBe(401);
        }
        // 🔴 The n+1th no longer costs the service anything: it is refused BEFORE
        // any verification.
        const refus = await echouer(url, 'ada@exemple.test');
        expect(refus.status).toBe(429);
        expect((await corpsDe(refus)).refus).toBe('trop-de-tentatives');
    }, 30000);

    it('(a bis) the account brake bites even with the RIGHT password', async () => {
        // ⚠️ It is D1's acknowledged trade-off, and it turns against the
        // legitimate user: an attacker can burn the budget of an
        // account they target and deny access to its owner during
        // the window. It is tested here rather than left implicit.
        const url = await servir('frein-compte-legitime');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) await echouer(url, 'ada@exemple.test');
        const legitime = await poster(`${url}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        });
        expect(legitime.status).toBe(429);
    }, 30000);

    it('(a ter) the case of the email does NOT give a new budget', async () => {
        // Without lower-case normalisation, `ADA@exemple.test` would be a
        // second key, and the account's budget would be multiplied by the number
        // of cases the attacker can write.
        const url = await servir('frein-compte-casse');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) await echouer(url, 'ada@exemple.test');
        expect((await echouer(url, 'ADA@Exemple.TEST')).status).toBe(429);
    }, 30000);

    it('(b) the n+1th from the SAME address, accounts all DISTINCT, is refused', async () => {
        // No account budget can bite here: each email is
        // tried ONCE only. Only the address key can refuse — it is
        // the only brake that closes account SWEEPING.
        const url = await servir('frein-adresse');
        for (let i = 0; i < ECHECS_MAX_ADRESSE; i++) {
            expect((await echouer(url, `n${i}@exemple.test`)).status).toBe(401);
        }
        expect((await echouer(url, 'yet-another-one@exemple.test')).status).toBe(429);
    }, 60000);

    it('(c) 🔴 the braked refusal DOES NOT TOUCH the database — so no scrypt', async () => {
        // 🔴 IT IS THE ASSERTION THAT GIVES THE BRAKE ITS MEANING. A brake placed
        // AFTER hashing would count failures it has already paid for at a high
        // price: `scrypt` is memory-hard and deliberately expensive
        // (measured today: 68 ms per hash), and an attacker who
        // triggers it at will exhausts the service without ever guessing a secret.
        const { url, acces, remettre } = await servirCompte('frein-sans-base');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) await echouer(url, 'ada@exemple.test');
        // The counter is reset AFTER the paid attempts, so that
        // the measurement bears ONLY on the braked request.
        remettre();
        const refus = await echouer(url, 'ada@exemple.test');
        expect(refus.status).toBe(429);
        expect(acces()).toBe(0);
    }, 30000);

    it('(d) a SUCCESS resets the ACCOUNT counter to zero', async () => {
        const url = await servir('frein-succes-compte');
        for (let i = 0; i < ECHECS_MAX_COMPTE - 1; i++) await echouer(url, 'ada@exemple.test');
        expect((await poster(`${url}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        })).status).toBe(200);
        // Without the reset, the `max`-th of this second series
        // would cross the budget and return 429.
        for (let i = 0; i < ECHECS_MAX_COMPTE - 1; i++) {
            expect((await echouer(url, 'ada@exemple.test')).status).toBe(401);
        }
    }, 30000);

    it("(e) 🔴 a success does NOT reset the ADDRESS counter to zero", async () => {
        // 🔴 Applying it to the address key WOULD LAUNDER an attacker who
        // owns a valid account: they would only need to log into it between
        // two bursts to reset their address budget to zero.
        const url = await servir('frein-succes-adresse');
        for (let i = 0; i < ECHECS_MAX_ADRESSE - 1; i++) await echouer(url, `m${i}@exemple.test`);
        expect((await poster(`${url}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        })).status).toBe(200);
        // The address is at `max - 1`; this attempt brings it to `max`…
        expect((await echouer(url, 'second-to-last@exemple.test')).status).toBe(401);
        // …and the next one is refused. Had the success cleared the address,
        // it would be at 1 and this one would pass.
        expect((await echouer(url, 'last@exemple.test')).status).toBe(429);
    }, 60000);

    it('(f) the 429 carries `Retry-After` AND the CORS headers', async () => {
        // 🔴 Without the CORS headers, the BROWSER cannot read the refusal,
        // and the user sees an opaque failure instead of "try again in
        // n minutes". No Node test would see it — Node `fetch` does not apply
        // the origin policy — hence the assertion on the header itself.
        const url = await servir('frein-entetes', ORIGINE);
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            await poster(`${url}/auth/connexion`, { email: 'ada@exemple.test', motdepasse: 'faux' },
                { origin: ORIGINE });
        }
        const refus = await poster(`${url}/auth/connexion`,
            { email: 'ada@exemple.test', motdepasse: 'faux' }, { origin: ORIGINE });
        expect(refus.status).toBe(429);
        const retry = refus.headers.get('retry-after');
        expect(retry).not.toBeNull();
        // In SECONDS, a positive integer — never 0, which would invite the
        // requester to come back immediately.
        expect(Number(retry)).toBeGreaterThan(0);
        expect(Number.isInteger(Number(retry))).toBe(true);
        expect(refus.headers.get('access-control-allow-origin')).toBe(ORIGINE);
    }, 30000);

    it("(g) the trace NAMES the retained address", async () => {
        // ⚠️ It is the ONLY remedy for the failure mode named in
        // `http/adresse-source.ts`: an operator who sets up a proxy without
        // declaring its trust will see their per-address brake degenerate into a
        // GLOBAL brake, and the only thing that will tell them is this line, where they
        // will recognise their proxy's address.
        const avertir = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const url = await servir('frein-trace');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) await echouer(url, 'ada@exemple.test');
        await echouer(url, 'ada@exemple.test');
        const lignes = avertir.mock.calls.map((c) => String(c[0]));
        const freinees = lignes.filter((l) => l.startsWith('frein '));
        expect(freinees.length).toBeGreaterThanOrEqual(1);
        expect(freinees[0]).toContain('adresse=');
        expect(freinees[0]).toContain('route=/auth/connexion');
    }, 30000);

    it("(h) `/auth/rafraichir` is braked by the ADDRESS alone", async () => {
        // ⚠️ The requester presents NO email there, only an opaque
        // token: taking that token as the key would amount to indexing a table
        // on a secret.
        const url = await servir('frein-rafraichir');
        for (let i = 0; i < ECHECS_MAX_ADRESSE; i++) {
            expect((await poster(`${url}/auth/rafraichir`, { rafraichissement: `faux-${i}` })).status)
                .toBe(401);
        }
        const refus = await poster(`${url}/auth/rafraichir`, { rafraichissement: 'encore-faux' });
        expect(refus.status).toBe(429);
    }, 60000);

    it("(h bis) 🔴 a failure on `/auth/rafraichir` consumes NO account budget", async () => {
        // Otherwise, an attacker who knows no email could still
        // lock accounts — or, more subtly, the account key
        // would be the token itself.
        const url = await servir('frein-rafraichir-compte');
        for (let i = 0; i < ECHECS_MAX_COMPTE + 2; i++) {
            await poster(`${url}/auth/rafraichir`, { rafraichissement: `faux-${i}` });
        }
        // Ada's account was never named: her login must pass.
        expect((await poster(`${url}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        })).status).toBe(200);
    }, 30000);
});
