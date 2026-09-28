// The access token: an HS256 JWT written with `node:crypto` and NOTHING else.
//
// 🔴 NO DEPENDENCY, and it is a MEASURED property, not an intention.
// On 19 August 2026, on this Node (v24.9.0):
//     createHmac('sha256', secret).update(`${h}.${c}`).digest('base64url')
// returns a 43-character signature, and the payload is read back via
// `Buffer.from(…, 'base64url')`. `jsonwebtoken` would therefore bring nothing but one
// more package, and the check `adds no production dependency
// outside the allow-list` (`base/pilote.test.ts`) is the witness of this property.
//
// 🔴 THIS MODULE IS PURE: the clock is a PARAMETER, never `Date.now()` read
// here. That is the repository rule (`depot/session.ts`, `signaling/ice.ts`), and
// it is what makes expiry testable at three distinct instants instead
// of being inert.
//
// ⚠️ DELIBERATE DIVERGENCE FROM RFC 7519: `exp` is here in
// MILLISECONDS, where the standard wants seconds. The reason is that there
// should be ONLY ONE time unit in this whole package — the database writes
// `Date.now()` in milliseconds, the token pool too, and two units
// in one service get mixed up sooner or later. The token is only read by
// this service, never by a third party; the day it would be, it is this
// decision that should be reopened, not worked around. Without this paragraph,
// the next reader will think it is a bug.

// 🔴 THE TYPE CLAIM (P3): an agent token and a human token are signed
// by the SAME secret. Without a claim telling them apart, they are
// interchangeable in both directions -- see `TypeSujet` below, where the
// two confusions are named. The claim is called `sty` and lives in the
// PAYLOAD: see `CLAIM_TYPE`.

import { createHmac, timingSafeEqual } from 'node:crypto';

export type MotifJeton = 'forme' | 'algorithme' | 'signature' | 'expire';

/// What the subject of a token IS: a human, or a VM agent.
///
/// 🔴 WHY THIS CLAIM EXISTS: both tokens are signed by the SAME
/// secret (`PLATEFORME_SECRET_JETON`) and carry the same `{sub, exp}` payload.
/// Without it, they are literally INTERCHANGEABLE, in both directions and
/// both are serious:
///   - a stolen HUMAN token would open an `agent` role, hence an `ice-config`
///     on any session whose name it prefixed;
///   - an AGENT token would open a `client` role, bypassing the session
///     ownership that P2 set up (`signaling/propriete.ts`).
/// The guard therefore requires `agent` for the `agent` role, and non-`agent` for the
/// `client` role.
export type TypeSujet = 'utilisateur' | 'agent';

export type VerdictJeton =
    | { ok: true; sujet: string; type: TypeSujet }
    | { ok: false; motif: MotifJeton };

/// 10 minutes. ⚠️ NOT CALIBRATED: no measurement has judged it, it joins the
/// repository's list of uncalibrated constants. It is short because the
/// rotating refresh (`depot/jeton.ts`) carries the counterpart.
export const DUREE_JETON_ACCES_MS = 600_000;

/// 32 characters. ⚠️ NOT CALIBRATED either — it is the length of a
/// 256-bit secret in hexadecimal cut in half, not a measured threshold.
export const MIN_SECRET_LENGTH = 32;

const ALGORITHME = 'HS256';

/// The name of the type claim IN THE PAYLOAD.
///
/// ⚠️ IT IS CALLED `sty` AND NOT `typ`, AND IT IS NOT A WHIM: `typ` is
/// already used by the JWT HEADER (`{ alg: ALGORITHME, typ: 'JWT' }`,
/// below), where it designates the type of the TOKEN and is always `JWT`. Two
/// same-named fields with different meanings in the same token would mislead any
/// reviewer, and the first one to mix them up would write a check that
/// cannot fail. `sty` = "subject-type".
///
/// 🔴 IT LIVES IN THE PAYLOAD, WHICH IS SIGNED — never in the header, which is
/// NOT. Deriving behaviour from unsigned data is exactly
/// the algorithm confusion against which `alg` is compared further down.
const CLAIM_TYPE = 'sty';

/// A missing claim means `user`.
///
/// 🔴 That is what keeps IN FLIGHT the tokens issued by P2, which carry
/// none: P3 adds the ability to SAY `agent`, it does not invalidate what
/// exists. Corollary kept here: a `user` token does NOT WRITE it
/// either, which keeps the wire format identical to P2's -- a single
/// encoding, hence a single read path.
const TYPE_PAR_DEFAUT: TypeSujet = 'utilisateur';

const TYPES_CONNUS: readonly TypeSujet[] = ['utilisateur', 'agent'];

function encoder(value: unknown): string {
    return Buffer.from(JSON.stringify(value), 'utf8').toString('base64url');
}

function signature(tete: string, secret: string): string {
    return createHmac('sha256', secret).update(tete).digest('base64url');
}

/// Signs a token for `sujet`. THROWS on a secret that is too short: a service that
/// started with a guessable secret would not be authenticated at all.
export function signer(
    sujet: string,
    secret: string,
    maintenant: number,
    dureeMs: number = DUREE_JETON_ACCES_MS,
    type: TypeSujet = TYPE_PAR_DEFAUT,
): string {
    if (secret.length < MIN_SECRET_LENGTH) {
        throw new Error(
            `signing secret too short: ${secret.length} characters, at least ${MIN_SECRET_LENGTH} are required`,
        );
    }
    const tete = `${encoder({ alg: ALGORITHME, typ: 'JWT' })}.${encoder({
        sub: sujet,
        // In MILLISECONDS — see the divergence declared at the top of the file.
        exp: maintenant + dureeMs,
        // The claim is only WRITTEN if it says something other than the default: see
        // `TYPE_PAR_DEFAUT`. An explicit `user` and a P2 token are
        // thus the SAME token, byte for byte.
        ...(type === TYPE_PAR_DEFAUT ? {} : { [CLAIM_TYPE]: type }),
    })}`;
    return `${tete}.${signature(tete, secret)}`;
}

function estObjet(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/// Verifies a token. ALWAYS returns a verdict, never an exception: the
/// `jeton` comes from the network, and a throwing `JSON.parse` would make the answer
/// 500 where it must be 401.
export function verifyToken(jeton: unknown, secret: string, maintenant: number): VerdictJeton {
    if (typeof jeton !== 'string') return { ok: false, motif: 'forme' };

    const morceaux = jeton.split('.');
    if (morceaux.length !== 3) return { ok: false, motif: 'forme' };
    const [enteteB64, chargeB64, signatureRecue] = morceaux;

    let entete: unknown;
    let charge: unknown;
    try {
        entete = JSON.parse(Buffer.from(enteteB64, 'base64url').toString('utf8'));
        charge = JSON.parse(Buffer.from(chargeB64, 'base64url').toString('utf8'));
    } catch {
        return { ok: false, motif: 'forme' };
    }
    if (!estObjet(entete) || !estObjet(charge)) return { ok: false, motif: 'forme' };

    // 🔴 The `alg` is COMPARED, never used to CHOOSE an algorithm.
    // The header is not signed: deriving behaviour from unsigned
    // data is the algorithm confusion, and that is how an `alg:'none'`
    // without a signature gets accepted. The check comes BEFORE the
    // signature one, so that the returned reason names the real cause.
    if (entete.alg !== ALGORITHME) return { ok: false, motif: 'algorithme' };

    const attendue = Buffer.from(signature(`${enteteB64}.${chargeB64}`, secret), 'utf8');
    const recue = Buffer.from(signatureRecue, 'utf8');
    // LENGTHS first: measured, `timingSafeEqual` THROWS
    // `Input buffers must have the same byte length` when they differ.
    if (attendue.length !== recue.length) return { ok: false, motif: 'signature' };
    if (!timingSafeEqual(attendue, recue)) return { ok: false, motif: 'signature' };

    const { sub, exp } = charge;
    if (typeof sub !== 'string' || sub.length === 0) return { ok: false, motif: 'forme' };
    if (typeof exp !== 'number' || !Number.isFinite(exp)) return { ok: false, motif: 'forme' };

    // 🔴 AN UNKNOWN TYPE IS REFUSED, never mapped to the default. Mapping it to
    // `user` would let a token of a THIRD type, issued some day by
    // a future version of the service, be accepted as human by an
    // old version -- a silent privilege change checked
    // in the wrong direction. ABSENCE, on the other hand, does mean the default: it is the
    // P2 format, and it is a known state, not an unknown one.
    const brut = charge[CLAIM_TYPE];
    let type: TypeSujet;
    if (brut === undefined) {
        type = TYPE_PAR_DEFAUT;
    } else if (typeof brut === 'string' && (TYPES_CONNUS as readonly string[]).includes(brut)) {
        type = brut as TypeSujet;
    } else {
        return { ok: false, motif: 'forme' };
    }

    // A SHARP bound, written so that the test can besiege it from both sides.
    if (maintenant >= exp) return { ok: false, motif: 'expire' };

    return { ok: true, sujet: sub, type };
}
