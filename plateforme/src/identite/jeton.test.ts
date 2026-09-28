// The HS256 JWT, without dependencies and with a clock as a parameter.
//
// 🔴 Two tests of this file forge a token BY HAND — it is the only way
// to test the most classic JWT vulnerability, algorithm
// confusion: a verifier that READS `alg` in the header and trusts it
// accepts an `alg:'none'` token without a signature. The header is not signed:
// one never derives a behaviour from unsigned data.

import { createHmac } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import {
    DUREE_JETON_ACCES_MS,
    MIN_SECRET_LENGTH,
    signer,
    verifyToken,
} from './jeton';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const T0 = 1_787_000_000_000;

function b64(value: unknown): string {
    return Buffer.from(JSON.stringify(value), 'utf8').toString('base64url');
}

/// Forges a token with the wanted header and payload. If `signature` is
/// omitted, it is computed in HS256 with the secret — a token whose ONLY
/// lie is the `alg`.
function forger(entete: unknown, charge: unknown, signature?: string): string {
    const tete = `${b64(entete)}.${b64(charge)}`;
    return `${tete}.${signature ?? createHmac('sha256', SECRET).update(tete).digest('base64url')}`;
}

describe('signer and verifierJeton', () => {
    it('re-reads the subject of a signed token', () => {
        const jeton = signer('user-42', SECRET, T0);
        expect(verifyToken(jeton, SECRET, T0))
            .toEqual({ ok: true, sujet: 'user-42', type: 'utilisateur' });
    });

    it('REFUSES alg:none, even with an empty signature', () => {
        // The classic forgery: the attacker sets `none` and removes the signature.
        const jeton = forger({ alg: 'none', typ: 'JWT' }, { sub: 'intrus', exp: T0 + 10_000 }, '');
        expect(verifyToken(jeton, SECRET, T0)).toEqual({ ok: false, motif: 'algorithme' });
    });

    it('REFUSES alg:RS256, whose HS256 signature is nevertheless VALID', () => {
        // 🔴 This token carries a correct HMAC signature: ONLY the check
        // of `alg` can refuse it. A verifier that did not compare
        // the algorithm would accept it, and this test is the only one to see it.
        const jeton = forger({ alg: 'RS256', typ: 'JWT' }, { sub: 'intrus', exp: T0 + 10_000 });
        expect(verifyToken(jeton, SECRET, T0)).toEqual({ ok: false, motif: 'algorithme' });
    });

    it('REFUSES a signature with one changed character', () => {
        const jeton = signer('user-42', SECRET, T0);
        const [tete, charge, signature] = jeton.split('.');
        const abimee = (signature[0] === 'A' ? 'B' : 'A') + signature.slice(1);
        expect(verifyToken(`${tete}.${charge}.${abimee}`, SECRET, T0))
            .toEqual({ ok: false, motif: 'signature' });
    });

    it('expires on a clock that VARIES: accepted at t0+d/2, refused at t0+d and beyond', () => {
        // 🔴 THREE distinct instants, and that is the point: a frozen clock
        // would make this test inert, which is exactly what the RED
        // column of criterion ② forbids.
        const duree = 60_000;
        const jeton = signer('user-42', SECRET, T0, duree);
        expect(verifyToken(jeton, SECRET, T0 + duree / 2)).toEqual({
            ok: true,
            sujet: 'user-42',
            type: 'utilisateur',
        });
        // The bound is STRICT: `maintenant >= exp` refuses.
        expect(verifyToken(jeton, SECRET, T0 + duree)).toEqual({ ok: false, motif: 'expire' });
        expect(verifyToken(jeton, SECRET, T0 + duree + 1)).toEqual({ ok: false, motif: 'expire' });
    });

    it('REFUSES an invalid shape without ever THROWING', () => {
        // A `JSON.parse` that throws here would make the HTTP caller answer 500,
        // where it must answer 401 — and the gap would on its own be an oracle.
        const attendu = { ok: false, motif: 'forme' };
        expect(verifyToken('deux.segments', SECRET, T0)).toEqual(attendu);
        expect(verifyToken('###.###.###', SECRET, T0)).toEqual(attendu);
        // A payload that is a number, not an object: `JSON.parse` succeeds.
        expect(verifyToken(forger({ alg: 'HS256', typ: 'JWT' }, 42), SECRET, T0)).toEqual(attendu);
        // And everything that is not even a string.
        expect(verifyToken(undefined, SECRET, T0)).toEqual(attendu);
        expect(verifyToken(null, SECRET, T0)).toEqual(attendu);
        expect(verifyToken({ jeton: 'x' }, SECRET, T0)).toEqual(attendu);
    });

    it('signer THROWS on a secret shorter than LONGUEUR_SECRET_MIN', () => {
        // Without this refusal, a platform would be deployed with a guessable
        // secret, and nothing would say so.
        expect(MIN_SECRET_LENGTH).toBe(32);
        expect(() => signer('u', 'trop-court', T0)).toThrow(/32/);
        expect(DUREE_JETON_ACCES_MS).toBeGreaterThan(0);
    });
    it('a token WITHOUT a type claim counts as « utilisateur » — P2 stays in flight', () => {
        // 🔴 The red: returning `undefined`. Every token emitted by P2 and still in
        // flight would become undecidable, which no requirement calls for — the
        // only thing P3 adds is the ability to SAY `agent`, not that
        // of invalidating what exists.
        const jeton = signer('u1', SECRET, T0);
        const v = verifyToken(jeton, SECRET, T0);
        expect(v).toEqual({ ok: true, sujet: 'u1', type: 'utilisateur' });
    });

    it('a token signed with the « agent » type is re-read as such', () => {
        const jeton = signer('RhH1x2QmTz9kLpVbNc7dAw', SECRET, T0, DUREE_JETON_ACCES_MS, 'agent');
        expect(verifyToken(jeton, SECRET, T0))
            .toEqual({ ok: true, sujet: 'RhH1x2QmTz9kLpVbNc7dAw', type: 'agent' });
    });

    it('REFUSES a FORGED type claim, with the original signature kept', () => {
        // ⚠️ THIS TEST IS WEAK, AND IT MUST BE SAID RATHER THAN DISCOVERED.
        // It ALREADY passes on the code from before P3, where no claim exists: what
        // it really tests is that any touch-up of the payload breaks the
        // signature — a property P2 already had. P3's plan announces it
        // as red; it is not, and the mutation it names ("carry
        // the claim outside the signed payload") IS NOT FEASIBLE HERE: the
        // signature covers `entete.charge`, hence the header TOO. There is
        // no unsigned position in this token in which to put a claim.
        //
        // 🔴 WHAT REALLY PINS THE LOCATION OF THE CLAIM is the next
        // test, that of the UNKNOWN type: moving the claim to the header
        // turns it red ("expected { ok: true, sujet: 'u1', …(1) } to deeply
        // equal { ok: false, motif: 'forme' }"), MEASURED. This one stays as a
        // non-regression guard, at its fair value and no more.
        const legitime = signer('u1', SECRET, T0);
        const [, , signatureDOrigine] = legitime.split('.');
        const charge = JSON.parse(
            Buffer.from(legitime.split('.')[1], 'base64url').toString('utf8'),
        );
        const promu = forger(
            { alg: 'HS256', typ: 'JWT' },
            { ...charge, sty: 'agent' },
            signatureDOrigine,
        );
        expect(verifyToken(promu, SECRET, T0)).toEqual({ ok: false, motif: 'signature' });
    });

    it('🔴 REFUSES a type of UNKNOWN value, rather than bringing it back to « utilisateur »', () => {
        // 🔴 The red: letting it through, or bringing it back to `user`. A
        // token of unknown type would become a human token -- and the day
        // a third type exists, an old service would accept it as
        // human instead of refusing it. The token is VALIDLY SIGNED here:
        // only the value of the claim is out of domain.
        const jeton = forger({ alg: 'HS256', typ: 'JWT' }, {
            sub: 'u1',
            exp: T0 + 10_000,
            sty: 'administrateur',
        });
        expect(verifyToken(jeton, SECRET, T0)).toEqual({ ok: false, motif: 'forme' });
    });
});
