// Extracting the AGENT bearer token, PURE.
//
// 🔴 THIS FILE IS THE TWIN OF `porteur.test.ts`, AND THE TWO ARE READ
// TOGETHER. `identite/jeton.ts` lists BOTH confusions and says they
// are both serious: a HUMAN token opening an agent path, and
// an AGENT token opening a human path. The two tokens are signed by
// the SAME secret and carry the same payload `{sub, exp}` — without the `sty` claim
// they are literally interchangeable.
//
// **If either direction is relaxed, the two identities become
// interchangeable again.** `porteur.test.ts` holds "an AGENT token is refused
// by the human reader"; this file holds the REVERSE direction, "a
// USER token is refused by the agent reader". Neither has
// value alone: they are the two halves of a single guard.

import { describe, expect, it } from 'vitest';
import { DUREE_JETON_ACCES_MS, signer } from '../identite/jeton';
import { lirePorteurAgent } from './porteur-agent';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const MS = 1_787_136_773_742;

/// The subject of an agent token is the SESSION PREFIX — it is what
/// `agents/canal.ts` signs (`jetonNeuf(prefixe)`), and never the VM
/// identifier. That name is therefore half of the assertion.
const PREFIXE = 'AAAAAAAAAAAAAAAAAAAAAA';

function entetes(
    value: string | string[] | undefined,
): Record<string, string | string[] | undefined> {
    // Node puts header names in LOWER CASE: `req.headers.authorization`
    // is the only spelling that exists on the server side.
    return value === undefined ? {} : { authorization: value };
}

function jetonAgent(sujet: string = PREFIXE): string {
    return signer(sujet, SECRET, MS, undefined, 'agent');
}

describe('lirePorteurAgent', () => {
    it('header ABSENT → jeton-absent, 401', () => {
        // 🔴 The red: returning `ok:true` with an empty prefix. The upload
        // route would become public, in the name of a VM that does not
        // exist — and `depot/agent.ts::lireParPrefixe('')` would return nothing, so
        // the refusal would fall much further on, under a reason that does not state the
        // cause.
        expect(lirePorteurAgent(entetes(undefined), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-absent',
            code: 401,
        });
        // An empty value is not a token either.
        expect(lirePorteurAgent(entetes(''), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-absent',
            code: 401,
        });
    });

    it('`Bearer <valid agent token>` → the PREFIX, never a VM id', () => {
        // 🔴 WHAT IS RETURNED IS THE TOKEN'S SUBJECT, AND THE SUBJECT IS THE
        // PREFIX. `agents/canal.ts` writes `signer(prefixe, …, 'agent')` and
        // its header says that "a channel that signed the VM identifier instead
        // of the prefix would deliver perfectly valid tokens that NOTHING
        // would accept". A module that returned a `vmId` here would be a
        // silent end-to-end failure: the VM is resolved LATER, by
        // `depot/agent.ts::lireParPrefixe`.
        expect(lirePorteurAgent(entetes(`Bearer ${jetonAgent()}`), SECRET, MS)).toEqual({
            ok: true,
            prefixe: PREFIXE,
        });
    });

    it('🔴 `Bearer <USER token>` → jeton-utilisateur, 403', () => {
        // 🔴 THE RED, AND IT GOES AS A PAIR WITH THAT OF `porteur.test.ts`:
        // accepting the `user` type here. A human would then drop
        // installers and download those of a VM whose agent they are
        // not, with a token the service itself delivered to them. The
        // symmetry is the point: `porteur.ts` refuses `agent`, this module
        // refuses `user`, and relaxing EITHER OF THE TWO is enough to make the
        // two identities interchangeable (`identite/jeton.ts`).
        //
        // ⚠️ 403 AND NOT 401: the token is VALID, it simply is not
        // an agent's. A 401 would invite reconnecting for nothing.
        const jetonHumain = signer('u-ada', SECRET, MS);
        expect(lirePorteurAgent(entetes(`Bearer ${jetonHumain}`), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-utilisateur',
            code: 403,
        });
    });

    it('🔴 a token WITHOUT a type claim is a user token, so refused', () => {
        // 🔴 THE ABSENCE OF THE CLAIM MEANS `user` (`identite/jeton.ts`,
        // `TYPE_PAR_DEFAUT`) — it is the format of P2's tokens, still in
        // flight. This test holds that the agent reader does read the DEFAULT and does not
        // settle for `verdict.type !== 'user'`: a module that
        // tested the absence of the claim as "not human" would accept any
        // P2 token.
        //
        // ⚠️ It is not redundant with the previous one: `signer` DOES NOT WRITE
        // the claim for a `user`, so that the two tokens are the
        // same byte for byte — it is precisely what makes the assertion
        // solid and the comment necessary, otherwise a reader will believe it
        // a copy.
        const sansClaim = signer('u-ada', SECRET, MS, undefined, 'utilisateur');
        expect(sansClaim.split('.').length).toBe(3);
        expect(lirePorteurAgent(entetes(`Bearer ${sansClaim}`), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-utilisateur',
            code: 403,
        });
    });

    it('🔴 EXPIRED token → jeton-expire, and the clock VARIES', () => {
        // 🔴 The red: freezing the clock. The test would become inert — there
        // would be only one observable instant and the threshold would never be
        // crossed. The bound of `identite/jeton.ts` is STRICT (`maintenant >=
        // exp`) precisely so that it can be besieged from both sides.
        const jeton = jetonAgent();
        const exp = MS + DUREE_JETON_ACCES_MS;
        // One millisecond BEFORE: still valid.
        expect(lirePorteurAgent(entetes(`Bearer ${jeton}`), SECRET, exp - 1)).toEqual({
            ok: true,
            prefixe: PREFIXE,
        });
        // At the EXACT bound: expired.
        expect(lirePorteurAgent(entetes(`Bearer ${jeton}`), SECRET, exp)).toEqual({
            ok: false,
            motif: 'jeton-expire',
            code: 401,
        });
    });

    it('🔴 a scheme other than `Bearer` → jeton-invalide', () => {
        // 🔴 The red: accepting any scheme. The case is compared
        // STRICTLY, a divergence from RFC 7235 declared in
        // `porteur-agent.ts` — and it MUST be the same as that of
        // `porteur.ts`: the two halves of the guard diverge on the TYPE and
        // on nothing else.
        const jeton = jetonAgent();
        for (const brut of [
            `Basic ${jeton}`,
            `bearer ${jeton}`,
            `BEARER ${jeton}`,
            jeton,
            `Bearer`,
            `Bearer ${jeton} too-much`,
        ]) {
            const v = lirePorteurAgent(entetes(brut), SECRET, MS);
            expect(v.ok).toBe(false);
            if (v.ok) return;
            expect(v.motif).toBe('jeton-invalide');
            expect(v.code).toBe(401);
        }
        // A wrong signature is invalid the same way — never 500.
        expect(lirePorteurAgent(entetes(`Bearer ${jeton}x`), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-invalide',
            code: 401,
        });
    });

    it('🔴 a REPEATED header (string[]) → jeton-invalide', () => {
        // 🔴 The red: taking `entetes.authorization[0]` silently. Two
        // authorization headers make an AMBIGUOUS request, not a request to
        // interpret.
        //
        // ⚠️ THIS PATH IS UNREACHABLE FROM A REAL HTTP REQUEST, and
        // it is MEASURED: on Node v24.9.0, two `Authorization` headers
        // yield a STRING — the parser keeps the first and discards the second.
        // The test therefore holds a property of the PURE module, not a defence of
        // the route; saying so prevents a successor from reading it as the proof
        // that `PUT /icone/:sha256` is protected from this case.
        expect(
            lirePorteurAgent(
                entetes([`Bearer ${jetonAgent()}`, `Bearer ${signer('u-ada', SECRET, MS)}`]),
                SECRET,
                MS,
            ),
        ).toEqual({ ok: false, motif: 'jeton-invalide', code: 401 });
        // Even a SINGLE-element array: the shape is ambiguous, not the
        // value. Node only produces an array if it saw several headers.
        expect(lirePorteurAgent(entetes([`Bearer ${jetonAgent()}`]), SECRET, MS).ok).toBe(false);
    });
});
