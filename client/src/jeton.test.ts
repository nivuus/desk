import { describe, expect, it, vi } from 'vitest';
import { accesDeReponse, accesParPomerium, assurerAccesFrais, CLE_ACCES, CLE_RAFRAICHISSEMENT, expiresBefore, jetonAcces, paireDeReponse, poser, poserAcces, rafraichirSiNecessaire, drain } from './jeton';
import type { Coffre } from './jeton';

/// A fake in-memory store. There is NO `localStorage` in
/// the test environment (observed: `client/` has no `vitest.config.*`,
/// so the environment is the default Node) — that is precisely why
/// the `Coffre` is a parameter and not a global.
function coffreFactice(initial: Record<string, string> = {}): Coffre & { contenu: Map<string, string> } {
    const contenu = new Map(Object.entries(initial));
    return {
        contenu,
        getItem: (c) => contenu.get(c) ?? null,
        setItem: (c, v) => void contenu.set(c, v),
        removeItem: (c) => void contenu.delete(c),
    };
}

/// Builds a token with the SHAPE of a JWT, with the requested `exp` — in
/// MILLISECONDS, like the service (`plateforme/src/identite/jeton.ts` declares
/// this divergence from RFC 7519). The signature is padding: this
/// module never checks it, and that is what the `signature` test exercises.
function jetonFactice(expMs: number, signature = 'peu-importe'): string {
    return `${b64url({ alg: 'HS256', typ: 'JWT' })}.${b64url({ sub: 'u-1', exp: expMs })}.${signature}`;
}

/// Encodes as base64url with `btoa`, never with `Buffer`: `client/` does not have
/// `@types/node` (observed: `npm run typecheck` returns `TS2580 Cannot find name
/// 'Buffer'`), and the tested code runs in a browser anyway.
function b64url(value: unknown): string {
    const octets = new TextEncoder().encode(JSON.stringify(value));
    const binaire = Array.from(octets, (o) => String.fromCharCode(o)).join('');
    return btoa(binaire).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

describe('the browser token store', () => {
    it("returns the access that was just set", () => {
        const coffre = coffreFactice();
        poser(coffre, { acces: 'a-1', rafraichissement: 'r-1' });
        expect(jetonAcces(coffre)).toBe('a-1');
    });

    it("returns `undefined` when no token was set", () => {
        expect(jetonAcces(coffreFactice())).toBeUndefined();
    });

    it('`vider` erases BOTH keys', () => {
        // 🔴 The red of this test is to erase only one: a sign-out
        // that left the refresh token behind would leave a
        // way to sign in again without a password, in a storage that
        // any script of the page reads.
        const coffre = coffreFactice();
        poser(coffre, { acces: 'a-1', rafraichissement: 'r-1' });
        drain(coffre);
        expect(coffre.contenu.has(CLE_ACCES)).toBe(false);
        expect(coffre.contenu.has(CLE_RAFRAICHISSEMENT)).toBe(false);
    });
});

describe('poserAcces', () => {
    it("sets the access", () => {
        const c = coffreFactice();
        poserAcces(c, 'J');
        expect(c.getItem(CLE_ACCES)).toBe('J');
    });

    // 🔴 THIS TEST IS THE FUNCTION'S RAISON D'ÊTRE. A refresh token
    // left by an earlier `motdepasse` setup would survive
    // the mode change and would be presented to a route that now returns
    // 404 — a failure whose symptom would be an unexplained sign-out ten
    // minutes after each page opening.
    it('ERASES the refresh token left by an earlier mount', () => {
        const c = coffreFactice();
        c.setItem(CLE_RAFRAICHISSEMENT, 'vieux');
        poserAcces(c, 'J');
        expect(c.getItem(CLE_RAFRAICHISSEMENT)).toBeNull();
    });
});

/* ── VALIDATING THE BODY OF `GET /auth/moi` ────────────────────────────────
   🔴 THESE TESTS EXIST BECAUSE THE RULE LIVED IN `connexion.ts`, WHICH
   IS NOT TESTED. The header of that file sets the criterion that tells
   a rule from wiring — "a condition is a rule if changing it changes
   what the PRODUCT decides" —, and the guard on `corps.acces` crosses it:
   without it, the product writes the string `"undefined"` to the store, sends
   `Bearer undefined`, shows a session error instead of the form, and
   **leaves the store poisoned**. It was therefore MOVED DOWN here, where
   the tests hold it.

   🔴 THE RED, PLAYED — THREE MUTATIONS, AND THEY DO NOT GO RED THE SAME WAY.
   The first draft of this comment announced "the FOUR refusal `it()`
   fall" for a single mutation: **that was wrong, and measurement said so**.
   Observed on August 21st, 2026, `cd client && npx vitest run src/jeton.test.ts`,
   replacing the body of `accesDeReponse` with:

     A. `return (corps as {acces?: string} | undefined | null)?.acces;`
        -> **2 failures** (empty string, non-string). The cases `{}`, `undefined`,
           `null` and `'J'` stay GREEN: optional chaining already returns
           `undefined` for them, so those tests do not discriminate THIS
           mutation.
     B. `return (corps as {acces?: string}).acces;` — the literal removal, as
        `connexion.ts` carried the guard
        -> **3 failures**, the third through `TypeError: Cannot read properties
           of undefined`.
     C. `return String((corps as {acces?: string} | undefined | null)?.acces);`
        — **the reproduction of the product's REAL defect**, the one that writes the
        string `"undefined"` to the store
        -> **4 failures**, the four refusal `it()`.

   🔴 WHAT THIS SPREAD TEACHES, AND WHY IT IS WRITTEN HERE RATHER
   THAN SMOOTHED OVER: the `{}` body test **cannot** go red on a mere
   guard removal — accessing an absent key returns `undefined` anyway.
   It only earns its value against mutation C, that is, against
   the defect we really seek to prevent. Announcing "all four
   fall" without saying UNDER WHICH mutation would have been exactly the pattern
   `CLAUDE.md` calls "a check never seen red".

   ⚠️ IN ALL THREE CASES, THE FIRST `it()` — the one exercising ACCEPTANCE
   — stays GREEN. That is what makes each red discriminating: it does not denounce
   an unplugged module. */
describe('the body of `GET /auth/moi`', () => {
    it("returns the token when the body carries one", () => {
        expect(accesDeReponse({ acces: 'J' })).toBe('J');
    });

    // The EXACT case the `motdepasse` mode would produce if the 404 did not carry
    // the mode: a body without `acces`.
    it("returns `undefined` on a body WITHOUT `acces` — otherwise the store receives the string « undefined »", () => {
        expect(accesDeReponse({})).toBeUndefined();
    });

    // ⚠️ DISTINCT FROM THE CASE ABOVE, AND NOT REDUNDANT: `typeof '' === 'string'`.
    // A `''` put in the store would be a token no `Authorization` can
    // carry, and `jetonAcces` would return it as if it were worth something.
    it('returns `undefined` on an EMPTY string', () => {
        expect(accesDeReponse({ acces: '' })).toBeUndefined();
    });

    it("returns `undefined` when `acces` is not a string", () => {
        expect(accesDeReponse({ acces: 42 })).toBeUndefined();
        expect(accesDeReponse({ acces: null })).toBeUndefined();
    });

    // `reponse.json().catch(() => undefined)` returns `undefined` on an unreadable
    // body, and `null` is perfectly valid JSON: both
    // reach this function, and neither must make it throw.
    it('returns `undefined` on `undefined`, `null` and a body that is not an object', () => {
        expect(accesDeReponse(undefined)).toBeUndefined();
        expect(accesDeReponse(null)).toBeUndefined();
        expect(accesDeReponse('J')).toBeUndefined();
    });
});

/// 🔴 ADDED AS A REVIEW FIX (round 1): `paireDeReponse` reuses
/// `accesDeReponse` for the `acces` half — its cases are therefore shown again
/// only once here, not repeated in full — and applies the SAME criterion
/// (string, not empty) to `rafraichissement`, on the exact model of the tests
/// above.
describe('the body of `POST /auth/rafraichir`', () => {
    it('returns the pair when the body carries a complete one', () => {
        expect(paireDeReponse({ acces: 'A', rafraichissement: 'R' })).toEqual({
            acces: 'A',
            rafraichissement: 'R',
        });
    });

    it('returns `undefined` when `acces` is absent, empty or not a string — through `accesDeReponse`', () => {
        expect(paireDeReponse({ rafraichissement: 'R' })).toBeUndefined();
        expect(paireDeReponse({ acces: '', rafraichissement: 'R' })).toBeUndefined();
        expect(paireDeReponse({ acces: 42, rafraichissement: 'R' })).toBeUndefined();
    });

    it("returns `undefined` when `rafraichissement` is ABSENT — the case the review found missing", () => {
        expect(paireDeReponse({ acces: 'A' })).toBeUndefined();
    });

    // ⚠️ DISTINCT FROM THE CASE ABOVE, AND NOT REDUNDANT: `typeof '' === 'string'`,
    // the same trap as in `accesDeReponse`, applied here to the second field.
    it('returns `undefined` on an EMPTY `rafraichissement`', () => {
        expect(paireDeReponse({ acces: 'A', rafraichissement: '' })).toBeUndefined();
    });

    it("returns `undefined` when `rafraichissement` is not a string", () => {
        expect(paireDeReponse({ acces: 'A', rafraichissement: 42 })).toBeUndefined();
        expect(paireDeReponse({ acces: 'A', rafraichissement: null })).toBeUndefined();
    });

    it('returns `undefined` on `undefined`, `null` and a body that is not an object', () => {
        expect(paireDeReponse(undefined)).toBeUndefined();
        expect(paireDeReponse(null)).toBeUndefined();
        expect(paireDeReponse('J')).toBeUndefined();
    });
});

describe('freshness, read WITHOUT checking the signature', () => {
    it("reads `exp` from a token whose SIGNATURE is wrong, and does not refuse it", () => {
        // 🔴 It is the decidable half of "the browser never verifies":
        // this token carries a signature that is nobody's, and
        // `expiresBefore` still returns `false` because its `exp` is far away.
        // The red is to claim to verify — the client does not have the secret, and
        // believing it verifies would be worse than knowing it does not.
        const jeton = jetonFactice(10_000, 'signature-that-is-no-one-s');
        expect(expiresBefore(jeton, 5_000)).toBe(false);
    });

    it('returns `true` when `exp` has already passed', () => {
        expect(expiresBefore(jetonFactice(1_000), 5_000)).toBe(true);
    });

    it('returns `true` on a MALFORMED token', () => {
        // The red is to return `false`: an unreadable token would then be believed
        // valid, and the session would fail later, elsewhere, on a refusal
        // from the service that nothing would link to this read.
        expect(expiresBefore('not-a-token', 0)).toBe(true);
        expect(expiresBefore('a.b.c', 0)).toBe(true);
        expect(expiresBefore(`${b64url({})}.${b64url({})}.x`, 0)).toBe(true);
    });
});

describe('the refresh', () => {
    it("DOES NOT CALL the network when the token is fresh beyond the margin", async () => {
        // 🔴 The red is to always call: one network round trip per
        // window opening, on a path that has nothing to do.
        const coffre = coffreFactice({
            [CLE_ACCES]: jetonFactice(100_000),
            [CLE_RAFRAICHISSEMENT]: 'r-1',
        });
        const appel = vi.fn();
        expect(await rafraichirSiNecessaire(coffre, 0, 30_000, appel)).toBe(true);
        expect(appel).not.toHaveBeenCalled();
    });

    it('calls, sets the new pair and returns `true` when the margin is crossed', async () => {
        const coffre = coffreFactice({
            [CLE_ACCES]: jetonFactice(10_000),
            [CLE_RAFRAICHISSEMENT]: 'r-1',
        });
        // The call is INJECTED, never the global `fetch`: with `fetch`, this test
        // would require a network and would stop being a test.
        const appel = vi.fn(async () => ({ acces: 'a-2', rafraichissement: 'r-2' }));
        expect(await rafraichirSiNecessaire(coffre, 0, 30_000, appel)).toBe(true);
        expect(appel).toHaveBeenCalledWith({ rafraichissement: 'r-1' });
        expect(coffre.contenu.get(CLE_ACCES)).toBe('a-2');
        expect(coffre.contenu.get(CLE_RAFRAICHISSEMENT)).toBe('r-2');
    });

    it("a refusal of the call EMPTIES the store and returns `false`", async () => {
        // The red is to keep the dead pair: the user then loops
        // on a refusal without ever seeing the sign-in screen again.
        const coffre = coffreFactice({
            [CLE_ACCES]: jetonFactice(10_000),
            [CLE_RAFRAICHISSEMENT]: 'r-1',
        });
        expect(await rafraichirSiNecessaire(coffre, 0, 30_000, async () => undefined)).toBe(false);
        expect(coffre.contenu.size).toBe(0);
    });

    it('returns `false` without calling when no refresh is stored', async () => {
        const coffre = coffreFactice({ [CLE_ACCES]: jetonFactice(10_000) });
        const appel = vi.fn();
        expect(await rafraichirSiNecessaire(coffre, 0, 30_000, appel)).toBe(false);
        expect(appel).not.toHaveBeenCalled();
    });
});

/* ── AUTOMATIC ACCESS — WHAT WAS MISSING, FOUND IN PRODUCTION ON AUGUST 30TH,
   2026 ────────────────────────────────────────────────────────────────────
   The hub (`hub/page.ts`) merely READ the store and complained
   if it was empty ("No token: sign in first."); the only code
   that knew how to get a token through Pomerium (`connexion.ts::tenterPomerium`)
   only ran WHEN THE SIGN-IN PAGE LOADED. As long as the root
   served the session page, nobody had seen a visitor land
   DIRECTLY on the hub without going through that screen — the batch that put the
   hub at the root had checked that `/` SERVES the hub, never that a visitor
   WITHOUT A TOKEN could use it. `accesParPomerium` and `assurerAcces`
   did not exist: that is WHAT this block turns red, before any implementation
   — `accesParPomerium` and `assurerAcces` are absent from the exports of
   `./jeton` on today's product, so this `import`, on its own,
   makes the WHOLE file fail (see the task report for the real
   output of this red). */
function appelFactice(
    reponses: { ok?: boolean; corps?: unknown; leve?: boolean }[],
): (url: string) => Promise<{ ok: boolean; json(): Promise<unknown> }> {
    let i = 0;
    return vi.fn(async () => {
        const r = reponses[Math.min(i, reponses.length - 1)];
        i += 1;
        if (r.leve === true) throw new Error('network unreachable');
        return { ok: r.ok ?? false, json: async () => r.corps };
    });
}

describe('accesParPomerium — the `tenterPomerium` path, extracted', () => {
    it("returns the token when `/auth/moi` answers 200 with a valid body", async () => {
        const appel = appelFactice([{ ok: true, corps: { acces: 'J' } }]);
        expect(await accesParPomerium('https://h', appel)).toBe('J');
        expect(appel).toHaveBeenCalledWith('https://h/auth/moi');
    });

    it("returns `undefined` on the 404 THAT `routes-identite.ts` RETURNS IN motdepasse MODE", async () => {
        // 🔴 THIS CASE CARRIES THE MODE THIS FAR (header of `connexion.ts`): a
        // 404 is not a failure, it is the service saying "this setup
        // authenticates by password". Taking it for a failure would
        // already be correct HERE (both return `undefined`); it is the MEANING
        // that differs, and it needs no extra branch.
        const appel = appelFactice([{ ok: false }]);
        expect(await accesParPomerium('https://h', appel)).toBeUndefined();
    });

    it('returns `undefined` on an unreachable network, without throwing', async () => {
        const appel = appelFactice([{ leve: true }]);
        expect(await accesParPomerium('https://h', appel)).toBeUndefined();
    });

    it("returns `undefined` on a body without a usable `acces`", async () => {
        const appel = appelFactice([{ ok: true, corps: {} }]);
        expect(await accesParPomerium('https://h', appel)).toBeUndefined();
    });
});

describe('assurerAccesFrais', () => {
    /// A token whose `exp` is `expMs`. The signature is not checked by
    /// the browser (see `expiresBefore`), so a fake header and signature
    /// are enough — which is what the `expiresBefore` tests already do.
    function jetonExpirantA(expMs: number): string {
        const charge = btoa(JSON.stringify({ exp: expMs })).replace(/=+$/, '');
        return `x.${charge}.y`;
    }

    function vaultWith(entrees: Record<string, string>): Coffre {
        const carte = new Map(Object.entries(entrees));
        return {
            getItem: (c) => carte.get(c) ?? null,
            setItem: (c, v) => void carte.set(c, v),
            removeItem: (c) => void carte.delete(c),
        };
    }

    it('a fresh token is returned WITHOUT any network call', async () => {
        const coffre = vaultWith({ [CLE_ACCES]: jetonExpirantA(100_000) });
        let appels = 0;
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => { appels += 1; return { ok: false, json: async () => ({}) }; },
            0,
            async () => { appels += 1; return undefined; },
        );
        // 🔴 THE ZERO CALLS IS THE SUBJECT OF THE TEST, AND IT STANDS ALONE:
        // `expect` stops at the first failure, so an assertion that counts
        // is never put in second position.
        expect(appels).toBe(0);
        expect(acces).toBe(jetonExpirantA(100_000));
    });

    it('a token expiring WITHIN THE MARGIN is treated as expired', async () => {
        // `exp` = 20 s, margin = 30 s, now = 0: still valid at
        // this very instant, already expired in the sense of the margin.
        const coffre = vaultWith({ [CLE_ACCES]: jetonExpirantA(20_000) });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            0,
            async () => undefined,
        );
        expect(acces).toBe('FRAIS');
    });

    it('an expired token with a refresh goes through the refresh, NOT through Pomerium', async () => {
        const coffre = vaultWith({
            [CLE_ACCES]: jetonExpirantA(0),
            [CLE_RAFRAICHISSEMENT]: 'R',
        });
        let pomerium = 0;
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => { pomerium += 1; return { ok: true, json: async () => ({ acces: 'PAR-POMERIUM' }) }; },
            10_000,
            async () => ({ acces: jetonExpirantA(999_000), rafraichissement: 'R2' }),
        );
        expect(pomerium).toBe(0);
        expect(acces).toBe(jetonExpirantA(999_000));
    });

    it('without a refresh token, Pomerium takes over and the token is SET', async () => {
        const coffre = vaultWith({ [CLE_ACCES]: jetonExpirantA(0) });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            10_000,
            async () => undefined,
        );
        expect(acces).toBe('FRAIS');
        // Set, otherwise the next reload would pay for the round trip again.
        expect(coffre.getItem(CLE_ACCES)).toBe('FRAIS');
    });

    it('both ways fail: returns undefined AND empties the store', async () => {
        const coffre = vaultWith({ [CLE_ACCES]: jetonExpirantA(0) });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: false, json: async () => ({}) }),
            10_000,
            async () => undefined,
        );
        expect(acces).toBeUndefined();
        // 🔴 THE STORE IS EMPTY, AND THAT IS THE POINT: an expired access left
        // in place would make the handshake fail later, elsewhere, on
        // a refusal nothing would link to here.
        expect(coffre.getItem(CLE_ACCES)).toBeNull();
    });

    it('an EMPTY store goes straight to Pomerium', async () => {
        const coffre = vaultWith({});
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            0,
            async () => undefined,
        );
        expect(acces).toBe('FRAIS');
    });

    // 🔴 ADDED AS A REVIEW FIX (round 1): the refresh
    // path was the FIRST production caller of
    // `rafraichirSiNecessaire`, and nothing exercised either an exception or a
    // malformed body before this round.
    it('a refresh callback that THROWS does NOT make assurerAccesFrais throw: goes through Pomerium', async () => {
        // The red would be the exception going up uncaught, instead of
        // falling back to the next step (Pomerium), and the page would stay
        // stuck on "identifying...".
        const coffre = vaultWith({
            [CLE_ACCES]: jetonExpirantA(0),
            [CLE_RAFRAICHISSEMENT]: 'R',
        });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            10_000,
            async () => { throw new Error('network unreachable'); },
        );
        expect(acces).toBe('FRAIS');
    });

    // The callback is wired as `hub/page.ts` really does it: it passes
    // the received body through `paireDeReponse` before returning it. An
    // incomplete body (`ok: true` but without `rafraichissement`, or with an empty
    // string) must therefore be rejected BY THE CALLBACK, never written as is to the
    // store by `rafraichirSiNecessaire::poser`.
    it("a MALFORMED refresh body (ok:true, without `rafraichissement`) does NOT poison the store", async () => {
        const coffre = vaultWith({
            [CLE_ACCES]: jetonExpirantA(0),
            [CLE_RAFRAICHISSEMENT]: 'R',
        });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            10_000,
            async () => paireDeReponse({ acces: 'X' }),
        );
        expect(acces).toBe('FRAIS');
        // 🔴 THE POINT OF THE TEST: the store must NEVER have seen 'X'.
        expect(coffre.getItem(CLE_ACCES)).toBe('FRAIS');
    });

    it("a MALFORMED refresh body (empty `acces`) does NOT poison the store", async () => {
        const coffre = vaultWith({
            [CLE_ACCES]: jetonExpirantA(0),
            [CLE_RAFRAICHISSEMENT]: 'R',
        });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            10_000,
            async () => paireDeReponse({ acces: '', rafraichissement: 'R2' }),
        );
        expect(acces).toBe('FRAIS');
        expect(coffre.getItem(CLE_ACCES)).toBe('FRAIS');
    });
});
