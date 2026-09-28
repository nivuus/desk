// Lire `Authorization: Bearer <jeton>`, et refuser le jeton d'agent.
//
// 🔴 THIS MODULE IS PURE: no database, no socket, no clock read — `maintenant` is
// a PARAMETER, as everywhere in this repository.
//
// 🔴 WHY IT EXISTS, AND WHY `identite/garde.ts` COULD NOT
// SERVE. The guard has the signature `verifier(poignee: { role; session;
// jeton? })`: it is cut for a WebSocket HANDSHAKE, it carries
// the session membership registry, and it is wired exclusively to
// the relay. Nothing, anywhere in this service, read the
// `Authorization` header before P4. The precedent of a direct consumer of
// `verifierJeton` outside the guard is `agents/canal.ts`.
//
// 🔴 REFUSING THE `agent` TYPE IS NOT DECORATIVE. Both tokens are signed
// by the SAME secret and carry the same payload: `identite/jeton.ts` lists
// the two confusions and says they are both serious. The direction
// « a human token opens an agent role » is closed by the guard and tested
// by P3; it is the other direction that this module closes, and it is the only one P4
// opens.

import { verifierJeton } from '../identite/jeton';

export type MotifPorteur = 'jeton-absent' | 'jeton-invalide' | 'jeton-expire' | 'jeton-agent';

export type VerdictPorteur =
    | { ok: true; utilisateurId: string }
    | { ok: false; motif: MotifPorteur; code: 401 | 403 };

/// The scheme, compared character by character.
const SCHEMA = 'Bearer';

/// Reads the header and yields the subject, or a refusal carrying its HTTP code.
///
/// ⚠️ DELIBERATE DIVERGENCE FROM RFC 7235 §2.1, declared rather than endured:
/// the standard makes the scheme name CASE-INSENSITIVE, and this module
/// compares it STRICTLY — `bearer` and `BEARER` are refused. The reason is that
/// the only emitter is our own client (`client/src/connexion.ts`), which
/// writes `Bearer`, and that a strict comparison is decidable where a
/// loose comparison opens a family of spellings nobody has
/// listed. The day a third-party client talks to this API, it is this
/// decision that will need reopening, not bypassing. The check is
/// EXPLICIT in one direction; do not make it implicit in the other.
///
/// ⚠️ ALL THE REASONS OF `verifierJeton` EXCEPT `expire` FOLD INTO
/// `jeton-invalide`, and that is deliberate: `forme`, `algorithme` and `signature`
/// tell apart ways of being wrong that the requester has no use for, and
/// from which an attacker, for their part, would learn how far along they are. `expire` is kept
/// because it is ACTIONABLE — it says to refresh rather than to
/// sign in again.
export function lirePorteur(
    entetes: Record<string, string | string[] | undefined>,
    secret: string,
    maintenant: number,
): VerdictPorteur {
    const brut = entetes.authorization;

    // 🔴 A REPEATED HEADER IS REFUSED, never disambiguated. Node yields an
    // array when it has seen several headers with the same name; picking one
    // would be making a decision an attacker exploits as soon as two
    // layers do not make the same one. It is an ambiguous request, not a
    // request to interpret.
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

    // 🔴 403 AND NOT 401: the token is VALID, it simply is not a
    // human one. A 401 would invite signing in again, which would change
    // nothing — and would hide that the request was made with the wrong
    // identity.
    if (verdict.type !== 'utilisateur') {
        return { ok: false, motif: 'jeton-agent', code: 403 };
    }

    return { ok: true, utilisateurId: verdict.sujet };
}
