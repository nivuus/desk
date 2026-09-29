import { describe, expect, it } from 'vitest';
import { lireConfig } from './config';

/// 42 characters: above `MIN_SECRET_LENGTH`, and never `''` — an
/// explicit test secret, as task 3 requires.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

/// The minimal setup — reused in several `describe`s.
/// ⚠️ Placing it here, before all the tests, makes `BASE` available
/// everywhere without redundancy. NONE OF THESE TESTS READS `process.env`: `lireConfig`
/// receives its environment as a PARAMETER (see `config.ts`), so nothing to set or
/// restore. Adding a variable to `Config` made no test depend on the
/// shell that runs it.
const BASE = { PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: SECRET };

describe('lireConfig', () => {
    it("refuses to start without PLATEFORME_HOTE — there is no default", () => {
        // The default MUST be the absence of a default (spec §4, criterion ④).
        // Setting '0.0.0.0' by default would pass a listen test without
        // guaranteeing anything: it is exactly the silent failure this repository fights.
        expect(() => lireConfig({})).toThrow(/PLATEFORME_HOTE/);
    });

    it("does not invent an address when the variable is empty", () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '' })).toThrow(/PLATEFORME_HOTE/);
    });

    it('reads the fields, with their non-permissive defaults', () => {
        // `PLATEFORME_SECRET_JETON` is supplied because it has NO default:
        // it is the subject of the next three tests.
        //
        // 🔴 `PLATEFORME_PROXY_DE_CONFIANCE` IS SUPPLIED, AND IT HAS BECOME
        // MANDATORY (task 6, guard of the refusal to start): without it, this
        // test's default `pomerium` mode would throw before even
        // reaching the assertion. The subject of THIS test is not that guard
        // — it has its own `describe` further down —, so we satisfy it
        // without questioning it.
        const c = lireConfig({
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_SECRET_JETON: SECRET,
            PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
        });
        // 🔴 SWITCH TO toStrictEqual, NOT toEqual: `toEqual` ignores
        // `undefined` properties, so an optional field added to `Config`
        // and returned as `undefined` would NOT cause a red. `toStrictEqual`
        // requires both objects to have exactly the same keys — it is the
        // only guard that holds against divergence. Proof:
        // `expect({a:1, u: undefined}).toEqual({a:1})` PASSES,
        // `toStrictEqual` fails.
        expect(c).toStrictEqual({
            hote: '127.0.0.1',
            port: 8080,
            base: 'sqlite',
            urlBase: ':memory:',
            secretJeton: SECRET,
            auth: 'pomerium',
            origineClient: undefined,
            proxyDeConfiance: new Set(['172.18.0.5']),
            repertoireIcones: 'donnees/icones',
            repertoireTeleversements: 'donnees/televersements',
            racinePage: undefined,
        });
    });

    it('keeps the icon directory it is GIVEN', () => {
        // 🔴 THE RED: the variable set and IGNORED. The store would
        // rebuild elsewhere, silently, re-uploading everything.
        //
        // `PLATEFORME_PROXY_DE_CONFIANCE` is set to satisfy the guard
        // of the refusal to start (task 6) — it is not the subject of this test.
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_ICONES: '/var/lib/guac/ic',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).repertoireIcones,
        ).toBe('/var/lib/guac/ic');
    });

    it('🔴 an EMPTY PLATEFORME_ICONES falls back to the default, not to the current directory', () => {
        // `env.X ?? 'defaut'` does NOT catch the empty string — P1 paid for this
        // exact mistake, where one of the two announced reds was actually green.
        //
        // `PLATEFORME_PROXY_DE_CONFIANCE` is set to satisfy the guard
        // of the refusal to start (task 6) — it is not the subject of this test.
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_ICONES: '',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).repertoireIcones,
        ).toBe('donnees/icones');
    });

    it('refuses an unknown PLATEFORME_BASE, rather than falling back to sqlite', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '::1', PLATEFORME_SECRET_JETON: SECRET, PLATEFORME_BASE: 'mysql' }))
            .toThrow(/PLATEFORME_BASE/);
    });

    it('refuses a port that is not an integer', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '::1', PLATEFORME_SECRET_JETON: SECRET, PLATEFORME_PORT: 'huit-mille' }))
            .toThrow(/PLATEFORME_PORT/);
    });

    it("refuses to start without PLATEFORME_SECRET_JETON — there is no default", () => {
        // 🔴 The default MUST be the absence of a default. A secret drawn at random
        // at startup would pass this test AND invalidate all tokens at
        // every restart, without anything saying so.
        expect(() => lireConfig({ PLATEFORME_HOTE: '127.0.0.1' })).toThrow(/PLATEFORME_SECRET_JETON/);
    });

    it("does not invent a secret when the variable is empty", () => {
        // ⚠️ P1 paid for exactly this mistake: `env.X ?? 'defaut'` does not
        // catch the empty string, and the test announced red was green.
        expect(() => lireConfig({ PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: '' }))
            .toThrow(/PLATEFORME_SECRET_JETON/);
    });

    it('refuses a secret that is too short, rather than signing with it', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: 'trop-court' }))
            .toThrow(/32/);
    });

    it("reads PLATEFORME_ORIGINE_CLIENT, which is OPTIONAL and never throws", () => {
        // It is optional where PLATEFORME_HOTE is not, and
        // the asymmetry comes from the consequences: an absent origin produces a
        // LOUD refusal from the browser, which an operator sees; an absent listen
        // address would produce a SILENT universal listen.
        // Refusing to start for it would break P5's deployment, where the
        // reverse proxy puts both on the same origin.
        //
        // `PLATEFORME_PROXY_DE_CONFIANCE` is set on the three calls to
        // satisfy the guard of the refusal to start (task 6) — it is not
        // the subject of this test.
        const sans = lireConfig({
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_SECRET_JETON: SECRET,
            PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
        });
        expect(sans.origineClient).toBeUndefined();
        const withIt = lireConfig({
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_SECRET_JETON: SECRET,
            PLATEFORME_ORIGINE_CLIENT: 'http://127.0.0.1:5173',
            PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
        });
        expect(withIt.origineClient).toBe('http://127.0.0.1:5173');
        // Empty means absent, never the empty string: an empty `Origin: ` would
        // match no real origin.
        const vide = lireConfig({
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_SECRET_JETON: SECRET,
            PLATEFORME_ORIGINE_CLIENT: '',
            PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
        });
        expect(vide.origineClient).toBeUndefined();
    });

    it("(a) PLATEFORME_PROXY_DE_CONFIANCE absent ⇒ we trust NOBODY", () => {
        // 🔴 The default is to trust nothing, never to trust everything. A permissive
        // default here would make the client's address FORGEABLE by the client
        // itself, hence the per-address brake bypassable with one header
        // line.
        //
        // 🔴 `PLATEFORME_AUTH: 'motdepasse'` IS SET, AND THAT IS THE POINT:
        // since the guard of the refusal to start (task 6), the EMPTY set
        // is only reachable in `motdepasse` mode — in `pomerium`, this same
        // setup now THROWS (see the dedicated describe). It is precisely
        // what the guard means: an empty trust set is no longer
        // a state one documents in `pomerium`, it is refused at startup.
        expect(lireConfig({ ...BASE, PLATEFORME_AUTH: 'motdepasse' }).proxyDeConfiance.size).toBe(0);
    });

    it("(b) EMPTY string ⇒ empty set, and not an empty entry", () => {
        // ⚠️ `env.X ?? 'defaut'` does not catch `''` — P1 paid for this exact
        // mistake in its task 1, where one of the two announced reds was green.
        //
        // 🔴 `PLATEFORME_AUTH: 'motdepasse'` IS SET — same reason as (a):
        // since the guard of the refusal to start (task 6), the EMPTY set
        // is only reachable in `motdepasse` mode.
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_AUTH: 'motdepasse',
                PLATEFORME_PROXY_DE_CONFIANCE: '',
            }).proxyDeConfiance.size,
        ).toBe(0);
    });

    it("(c) a comma-separated list, spaces REMOVED", () => {
        const c = lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5, 10.0.0.1' });
        expect(c.proxyDeConfiance.size).toBe(2);
        // Without the `trim`, the second entry would be ` 10.0.0.1` and would
        // NEVER match a peer address — trust would
        // fail silently, and the per-address brake would degenerate into a
        // global brake without any line saying so.
        expect(c.proxyDeConfiance.has('172.18.0.5')).toBe(true);
        expect(c.proxyDeConfiance.has('10.0.0.1')).toBe(true);
    });

    it("(d) an empty entry between two commas is IGNORED", () => {
        const c = lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5,,10.0.0.1' });
        expect(c.proxyDeConfiance.size).toBe(2);
        // An empty entry in the trust set would make trusted
        // any peer whose address is empty — that is, `ADRESSE_INCONNUE`
        // if it ever became `''`.
        expect(c.proxyDeConfiance.has('')).toBe(false);
    });
});

describe('the refusal to start in pomerium mode without a trusted proxy', () => {
    // 🔴 A REFUSAL TO START IS READ BEFORE ACTING. A silent 401 for
    // everyone would be read AFTER, on a service that answers, listens and
    // serves the ten other routers — the most discreet failure possible.
    it('THROWS in pomerium mode without PLATEFORME_PROXY_DE_CONFIANCE', () => {
        expect(() => lireConfig({ ...BASE, PLATEFORME_AUTH: 'pomerium' })).toThrow(
            /PLATEFORME_PROXY_DE_CONFIANCE/,
        );
    });

    // ⚠️ THE GUARD IS TIED TO THE MODE, like that of PLATEFORME_HOTE: in
    // `motdepasse`, the service authenticates by itself and the header is read
    // by nobody.
    it('does NOT throw in motdepasse mode', () => {
        expect(() => lireConfig({ ...BASE, PLATEFORME_AUTH: 'motdepasse' })).not.toThrow();
    });
});

describe('PLATEFORME_PAGE', () => {
    // 🔴 NO DEFAULT, unlike PLATEFORME_ICONES: a default like
    // `client/dist` would serve a random directory relative to the current
    // directory, and would turn the nginx setup — where the platform must serve
    // NOTHING — from a plain 404 into a 200 on unwanted files.
    // `PLATEFORME_PROXY_DE_CONFIANCE` is set on the four tests of this
    // block to satisfy the guard of the refusal to start (task 6) — it
    // is not their subject, which remains `racinePage`.
    it("is ABSENT by default, and the service then serves no file", () => {
        expect(
            lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5' }).racinePage,
        ).toBeUndefined();
    });

    // ⚠️ The EMPTY string test is DISTINCT from the absence one:
    // `env.X ?? 'defaut'` does not catch `''`. P1 paid for this exact mistake.
    it('treats the EMPTY string as an absence', () => {
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_PAGE: '',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).racinePage,
        ).toBeUndefined();
    });

    it('keeps the path that was set', () => {
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_PAGE: '/srv/page',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).racinePage,
        ).toBe('/srv/page');
    });

    it('includes the racinePage field in the object, even when absent', () => {
        // 🔴 THIS TEST CLOSES THE HOLE: removing `racinePage,` from the object
        // lireConfig returns makes this test fail, while the three tests
        // above pass (since `.racinePage` can be read on undefined).
        // It is the only one that detects the plain loss of the field.
        expect(
            Object.hasOwn(
                lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5' }),
                'racinePage',
            ),
        ).toBe(true);
    });
});

describe('PLATEFORME_AUTH', () => {
    /// The minimal setup — copied from the top of this file, never reinvented.
    ///
    /// ⚠️ ACKNOWLEDGED DEPARTURE FROM THE BRIEF: it carried `'x'.repeat(32)` as a
    /// literal, which `securite/secrets.test.ts` flags — it sweeps every
    /// LITERAL ASSIGNMENT of `PLATEFORME_SECRET_JETON` in a versioned
    /// file, and a quoted string is one, however trivial it
    /// is. `SECRET`, the constant already declared at the top of this file,
    /// is an IDENTIFIER — the same convention as `BASE` just below,
    /// exempted by name by `inoffensive()`.
    const base = {
        PLATEFORME_HOTE: '127.0.0.1',
        PLATEFORME_SECRET_JETON: SECRET,
    };

    it('defaults to pomerium', () => {
        // `PLATEFORME_PROXY_DE_CONFIANCE` is set to satisfy the guard
        // of the refusal to start (task 6) — the subject of this test is the
        // `pomerium` mode itself, not that guard.
        expect(lireConfig({ ...base, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5' }).auth).toBe(
            'pomerium',
        );
    });

    it('accepts motdepasse', () => {
        expect(lireConfig({ ...base, PLATEFORME_AUTH: 'motdepasse' }).auth).toBe('motdepasse');
    });

    it('falls back to the default when the value is EMPTY', () => {
        // `PLATEFORME_PROXY_DE_CONFIANCE` is set to satisfy the guard
        // of the refusal to start (task 6): an EMPTY value falls back to
        // `pomerium`, which requires the variable.
        expect(
            lireConfig({
                ...base,
                PLATEFORME_AUTH: '',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).auth,
        ).toBe('pomerium');
    });

    // 🔴 THE RED OF CRITERION ④: a case typo must NOT fall back
    // to the default, otherwise the password mode would run under the name of the
    // Pomerium mode — and the other direction is an OPENING.
    it('THROWS on an unknown value, never a fallback', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_AUTH: 'Pomerium' })).toThrow(
            /PLATEFORME_AUTH/,
        );
    });
});

describe("the listen guard of pomerium mode", () => {
    // Same acknowledged departure as above: `SECRET` rather than the brief's
    // literal, so as not to trigger `securite/secrets.test.ts`.
    const base = { PLATEFORME_SECRET_JETON: SECRET };

    // 🔴 THE RED OF CRITERION ⑤, and it describes the REAL setup of
    // 21 August 2026: the service runs today on 0.0.0.0:8080.
    it('REFUSES 0.0.0.0 in pomerium mode', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '0.0.0.0' })).toThrow(
            /universal listen/,
        );
    });

    it('REFUSES :: in pomerium mode', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '::' })).toThrow(
            /universal listen/,
        );
    });

    // ⚠️ ARBITRATION: the brief proposes `ECOUTES_UNIVERSELLES` with FOUR
    // members (`0.0.0.0`, `::`, `[::]`, `*`) but only has the two
    // above tested. An untested member is exactly "a check never
    // seen red" (§ measurement method, docs/claude/pitfalls-measurement-method.md): the next two
    // tests close that hole rather than removing members from the set.
    it('REFUSES [::] in pomerium mode', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '[::]' })).toThrow(
            /universal listen/,
        );
    });

    it('REFUSES * in pomerium mode', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '*' })).toThrow(
            /universal listen/,
        );
    });

    // The guard is tied to the MODE, not universal: without this test, one would not know
    // whether it refuses 0.0.0.0 or refuses always.
    it('LETS THROUGH 0.0.0.0 in motdepasse mode', () => {
        const c = lireConfig({ ...base, PLATEFORME_HOTE: '0.0.0.0', PLATEFORME_AUTH: 'motdepasse' });
        expect(c.hote).toBe('0.0.0.0');
    });

    // The Docker deployment sets a SERVICE NAME, not an address: the guard
    // must let it through, otherwise it breaks the safest setup of the three.
    //
    // `PLATEFORME_PROXY_DE_CONFIANCE` is set on these two tests to
    // satisfy the guard of the refusal to start (task 6) — the mode stays
    // `pomerium` by default, it is the subject of this describe.
    it('LETS THROUGH an internal network service name', () => {
        const c = lireConfig({
            ...base,
            PLATEFORME_HOTE: 'plateforme',
            PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
        });
        expect(c.hote).toBe('plateforme');
    });

    // The setup retained by spec § 7.1.
    it("LETS THROUGH the address of the libvirt bridge", () => {
        expect(
            lireConfig({
                ...base,
                PLATEFORME_HOTE: '192.168.3.1',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).hote,
        ).toBe('192.168.3.1');
    });
});
