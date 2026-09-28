// Read `Authorization: Bearer <token>`, and refuse the USER token.
//
// 🔴 THIS MODULE IS THE EXACT MIRROR OF `porteur.ts`, AND THE TWO ARE READ
// TOGETHER. `identite/jeton.ts` lists the TWO confusions and says they
// are both serious — both tokens are signed by the SAME
// secret (`PLATEFORME_SECRET_JETON`) and carry the same payload `{sub, exp}`:
//   - a HUMAN token that would open an agent path; that is what this
//     module closes;
//   - an AGENT token that would open a human path; that is what
//     `porteur.ts` closes.
// If one of the two loosens, the two identities become
// INTERCHANGEABLE again: it is not two guards, it is one, written in two
// halves. **Never touch one without rereading the other.**
//
// 🔴 THIS MODULE IS PURE: no database, no socket, no clock read — `maintenant` is
// a PARAMETER, as everywhere in this repository.
//
// 🔴 IT YIELDS THE SESSION PREFIX, NEVER THE VM ID, and that is not
// a naming detail: `agents/canal.ts` signs `signer(prefixe, …,
// 'agent')`, and its header says that « a channel that signed the VM
// id instead of the prefix would issue perfectly valid tokens that NOTHING
// would accept ». The subject of an agent token IS the prefix. The VM is resolved
// afterwards by `depot/agent.ts::lireParPrefixe` — **this module does not touch the
// database**, it has no right to and no need to.
//
// ⚠️ WHY THE HEADER SPLITTING IS COPIED FROM `porteur.ts` RATHER THAN
// SHARED. The two modules differ only by the type line and by what
// they yield; everything before is identical. The common factor
// would naturally live in a third module, and that is what will have to be
// done the day a THIRD header reader shows up. It is not
// done here because the G3 sub-block does not open `porteur.ts`, and a
// half extraction — the new module calling the old one, the old one unchanged —
// would cost an indirection without closing anything. **Declared rather than endured**:
// any fix to the splitting is made IN BOTH FILES.

import { verifierJeton } from '../identite/jeton';

export type MotifPorteurAgent =
    | 'jeton-absent'
    | 'jeton-invalide'
    | 'jeton-expire'
    | 'jeton-utilisateur';

export type VerdictPorteurAgent =
    | { ok: true; prefixe: string }
    | { ok: false; motif: MotifPorteurAgent; code: 401 | 403 };

/// The scheme, compared character by character.
const SCHEMA = 'Bearer';

/// Reads the header and yields the SESSION PREFIX, or a refusal carrying its HTTP
/// code.
///
/// ⚠️ DELIBERATE DIVERGENCE FROM RFC 7235 §2.1, taken from `porteur.ts` word
/// for word and for the same reason: the standard makes the scheme name
/// CASE-INSENSITIVE, and this module compares it STRICTLY — `bearer` and
/// `BEARER` are refused. The only emitter is our own agent
/// (`agent/src/apps/`, which writes `Bearer`), and a strict comparison is
/// decidable where a loose comparison opens a family of spellings that
/// nobody has listed. **The two halves of the guard must diverge on
/// the TYPE and on nothing else**: a case tolerated on one side and refused on
/// the other would be a difference nobody decided.
///
/// ⚠️ ALL THE REASONS OF `verifierJeton` EXCEPT `expire` FOLD INTO
/// `jeton-invalide`: `forme`, `algorithme` and `signature` tell apart
/// ways of being wrong that the requester has no use for, and from which an
/// attacker, for their part, would learn how far along they are. `expire` is kept because it
/// is ACTIONABLE — it tells the agent to ask the `/agent` channel for a token again
/// rather than enrol again.
export function lirePorteurAgent(
    entetes: Record<string, string | string[] | undefined>,
    secret: string,
    maintenant: number,
): VerdictPorteurAgent {
    const brut = entetes.authorization;

    // 🔴 A REPEATED HEADER IS REFUSED, never disambiguated — it is an ambiguous
    // request, not a request to interpret, and picking one would be making
    // a decision an attacker exploits as soon as two layers do not make
    // the same one.
    //
    // ⚠️ THIS PATH IS UNREACHABLE FROM A REAL HTTP REQUEST, and that is
    // MEASURED, not assumed: on this Node (v24.9.0), two
    // `Authorization` headers on the same request yield
    // `typeof req.headers.authorization === 'string'` and
    // `Array.isArray(...) === false` — the parser keeps the FIRST and drops the
    // second, `authorization` being on its discard list. The guard stays
    // because this module is PURE and nothing forces its caller to be the
    // Node HTTP layer; it is simply not the line of defence
    // one would think.
    if (Array.isArray(brut)) return { ok: false, motif: 'jeton-invalide', code: 401 };
    if (brut === undefined || brut === '') {
        return { ok: false, motif: 'jeton-absent', code: 401 };
    }

    // Split on spaces, and EXACTLY two pieces: a third
    // piece is a value we cannot read, not a token to truncate.
    const morceaux = brut.split(' ');
    if (morceaux.length !== 2 || morceaux[0] !== SCHEMA || morceaux[1] === '') {
        return { ok: false, motif: 'jeton-invalide', code: 401 };
    }

    const verdict = verifierJeton(morceaux[1], secret, maintenant);
    if (!verdict.ok) {
        return verdict.motif === 'expire'
            ? { ok: false, motif: 'jeton-expire', code: 401 }
            : { ok: false, motif: 'jeton-invalide', code: 401 };
    }

    // 🔴 403 AND NOT 401, exactly like the `jeton-agent` of `porteur.ts`: the
    // token is VALID, it simply is not an agent one. A 401
    // would invite signing in again, which would change nothing — and would hide
    // that the request was made with the wrong identity.
    if (verdict.type !== 'agent') {
        return { ok: false, motif: 'jeton-utilisateur', code: 403 };
    }

    // The subject of an agent token IS the session prefix — see the header.
    return { ok: true, prefixe: verdict.sujet };
}
