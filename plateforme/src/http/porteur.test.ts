// Extracting the bearer token, PURE.
//
// 🔴 BOTH DIRECTIONS OF THE CONFUSION ARE NAMED, ONLY ONE IS TESTED HERE.
// `identite/jeton.ts` lists both and says they are both
// serious: a HUMAN token opening an `agent` role, and an AGENT token opening
// a human role. The first direction is closed by `identite/garde.ts` and tested
// by P3; it is the SECOND this file holds, because it is the only one
// P4 opens — an HTTP route that accepted an agent token would show it
// a human's inventory.

import { describe, expect, it } from 'vitest';
import { DUREE_JETON_ACCES_MS, signer } from '../identite/jeton';
import { lirePorteur } from './porteur';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const MS = 1_787_136_773_742;

function entetes(value: string | string[] | undefined): Record<string, string | string[] | undefined> {
    // Node puts header names in LOWER CASE: `req.headers.authorization`
    // is the only spelling that exists on the server side.
    return value === undefined ? {} : { authorization: value };
}

describe('lirePorteur', () => {
    it('header ABSENT → jeton-absent, 401', () => {
        // 🔴 The red: returning `ok:true` with an empty subject. Every route
        // would become public, in the name of a user who does not exist.
        expect(lirePorteur(entetes(undefined), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-absent',
            code: 401,
        });
        // An empty value is not a token either.
        expect(lirePorteur(entetes(''), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-absent',
            code: 401,
        });
    });

    it('`Bearer <valid user token>` → the subject', () => {
        const jeton = signer('u-ada', SECRET, MS);
        expect(lirePorteur(entetes(`Bearer ${jeton}`), SECRET, MS)).toEqual({
            ok: true,
            userId: 'u-ada',
        });
    });

    it('🔴 `Bearer <jeton d’AGENT>` → jeton-agent, 403', () => {
        // 🔴 The red: accepting the `agent` type. The two tokens are signed
        // by the SAME secret and carry the same payload `{sub, exp}`: without the
        // `sty` claim, they are literally interchangeable
        // (`identite/jeton.ts`). An agent would see a human's inventory.
        //
        // ⚠️ 403 AND NOT 401: the token is VALID, it simply is not
        // a human's. A 401 would invite reconnecting, which would
        // change nothing.
        const jetonAgent = signer('PREFIXEdelaVM', SECRET, MS, undefined, 'agent');
        expect(lirePorteur(entetes(`Bearer ${jetonAgent}`), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-agent',
            code: 403,
        });
    });

    it('🔴 EXPIRED token → jeton-expire, and the clock VARIES', () => {
        // 🔴 The red: freezing the clock. The test would become inert — there
        // would be only one observable instant and the threshold would never be
        // crossed. The bound of `identite/jeton.ts` is STRICT (`maintenant >=
        // exp`) precisely so that it can be besieged from both sides.
        const jeton = signer('u-ada', SECRET, MS);
        const exp = MS + DUREE_JETON_ACCES_MS;
        // One millisecond BEFORE: still valid.
        expect(lirePorteur(entetes(`Bearer ${jeton}`), SECRET, exp - 1)).toEqual({
            ok: true,
            userId: 'u-ada',
        });
        // At the EXACT bound: expired.
        expect(lirePorteur(entetes(`Bearer ${jeton}`), SECRET, exp)).toEqual({
            ok: false,
            motif: 'jeton-expire',
            code: 401,
        });
    });

    it('🔴 a scheme other than `Bearer` → jeton-invalide', () => {
        // 🔴 The red: accepting any scheme. And the case is
        // compared STRICTLY — see the comment of `porteur.ts`, which
        // declares the divergence from RFC 7235 rather than suffering it. The
        // check must be explicit in one direction or the other; here it
        // is in the strict direction.
        const jeton = signer('u-ada', SECRET, MS);
        for (const brut of [
            `Basic ${jeton}`,
            `bearer ${jeton}`,
            `BEARER ${jeton}`,
            jeton,
            `Bearer`,
            `Bearer ${jeton} too-much`,
        ]) {
            const v = lirePorteur(entetes(brut), SECRET, MS);
            expect(v.ok).toBe(false);
            if (v.ok) return;
            expect(v.motif).toBe('jeton-invalide');
            expect(v.code).toBe(401);
        }
        // A wrong signature is invalid the same way — never 500.
        expect(lirePorteur(entetes(`Bearer ${jeton}x`), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-invalide',
            code: 401,
        });
    });

    it('🔴 a REPEATED header (string[]) → jeton-invalide', () => {
        // 🔴 The red: taking `entetes.authorization[0]` silently. Two
        // authorization headers make an AMBIGUOUS request, not a request to
        // interpret — and choosing one of the two is exactly the kind of
        // decision an attacker exploits when two layers do not choose
        // the same one.
        const bon = signer('u-ada', SECRET, MS);
        const agent = signer('PREFIXEdelaVM', SECRET, MS, undefined, 'agent');
        expect(lirePorteur(entetes([`Bearer ${bon}`, `Bearer ${agent}`]), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-invalide',
            code: 401,
        });
        // Even a SINGLE-element array: the shape is ambiguous, not the
        // value. Node only produces an array if it saw several headers.
        expect(lirePorteur(entetes([`Bearer ${bon}`]), SECRET, MS).ok).toBe(false);
    });
});
