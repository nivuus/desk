// Password hashing, in a format that carries its own algorithm.
//
// 🔴 WHY THE FORMAT CARRIES ITS PARAMETERS: `scrypt$N$r$p$sel$empreinte`.
// The day these parameters are calibrated — or replaced by Argon2id —, a
// rehash at the next sign-in will be enough, WITHOUT data migration
// (spec §3.5). A bare hash would not tell what it was
// produced with, and the whole database would have to be thrown away at once.
//
// Measured on 19 August 2026 on this Node (v24.9.0):
//     N=16384 r=8 p=1 : OK 29 ms (plan reading), 31 ms (reading from
//         the implementation, same machine — the gap is measurement variance)
//     N=32768 r=8 p=1 : REFUSED -> Invalid scrypt params:
//         error:030000AC:digital envelope routines::memory limit exceeded
// The limit is that of `maxmem` (32 MiB by default), crossed as soon as
// 128·N·r exceeds it. The message does NOT mention N. Hardening the parameter
// therefore requires raising `maxmem` explicitly — a step NOT taken here, for lack
// of a measurement justifying it. These parameters ARE NOT CALIBRATED: the 29 ms
// are a measurement, not a goal reached. They join the repository's already
// long list (BPP_MIN, FACTEUR_FOCUS, PART_DORMANTE_BPS, HYSTERESIS,
// TAILLE_MAX_SORTIE, DUREE_SECONDES).

import { randomBytes, scrypt, timingSafeEqual } from 'node:crypto';

export interface ParametresScrypt {
    N: number;
    r: number;
    p: number;
}

export const PARAMETRES_COURANTS: ParametresScrypt = { N: 16384, r: 8, p: 1 };

/// 16 bytes of salt, 32 bytes of hash: the usual sizes, and the ones
/// the tests assert — a shorter salt would weaken the protection
/// against precomputed tables without saving anything useful.
const OCTETS_SEL = 16;
const OCTETS_EMPREINTE = 32;

const ALGO = 'scrypt';

function deriver(
    motDePasse: string,
    sel: Buffer,
    params: ParametresScrypt,
): Promise<Buffer> {
    return new Promise((resolve, rejeter) => {
        scrypt(motDePasse, sel, OCTETS_EMPREINTE, params, (erreur, cle) => {
            if (erreur) rejeter(erreur);
            else resolve(cle);
        });
    });
}

/// Returns `scrypt$N$r$p$sel$empreinte`, salt and hash in base64url.
export async function hacher(
    motDePasse: string,
    params: ParametresScrypt = PARAMETRES_COURANTS,
): Promise<string> {
    const sel = randomBytes(OCTETS_SEL);
    const empreinte = await deriver(motDePasse, sel, params);
    return [
        ALGO,
        params.N,
        params.r,
        params.p,
        sel.toString('base64url'),
        empreinte.toString('base64url'),
    ].join('$');
}

/// Decomposes an encoding. THROWS on an unreadable shape: a caller that
/// received a half-filled object would produce a refusal whose cause would be
/// invisible.
export function analyser(encode: string): {
    algo: string;
    params: ParametresScrypt;
    sel: Buffer;
    empreinte: Buffer;
} {
    const morceaux = encode.split('$');
    if (morceaux.length !== 6) {
        throw new Error(
            `empreinte de mot de passe malformée : ${morceaux.length} segments au lieu de 6`,
        );
    }
    const [algo, n, r, p, sel, empreinte] = morceaux;
    return {
        algo,
        params: { N: Number(n), r: Number(r), p: Number(p) },
        sel: Buffer.from(sel, 'base64url'),
        empreinte: Buffer.from(empreinte, 'base64url'),
    };
}

/// `false` on a wrong password OR a malformed hash.
///
/// 🔴 THROWS on an unknown algorithm, and that is deliberate: a silent
/// `false` there would be indistinguishable from a bad password, and nobody
/// could diagnose a database written by a future version of the service.
export async function verifier(motDePasse: string, encode: string): Promise<boolean> {
    let analyse: ReturnType<typeof analyser>;
    try {
        analyse = analyser(encode);
    } catch {
        // An unreadable shape is a refusal, not an exception: the HTTP
        // caller must answer 401 and not 500.
        return false;
    }

    if (analyse.algo !== ALGO) {
        throw new Error(
            `algorithme de hachage inconnu : ${analyse.algo} — ce service ne sait vérifier que ${ALGO}`,
        );
    }

    const { N, r, p } = analyse.params;
    if (!Number.isInteger(N) || !Number.isInteger(r) || !Number.isInteger(p)) {
        return false;
    }

    let calculee: Buffer;
    try {
        calculee = await deriver(motDePasse, analyse.sel, analyse.params);
    } catch {
        // Parameters the library refuses (see the reading at the top)
        // yield a refusal, never an exception that would bubble up as a 500.
        return false;
    }

    // 🔴 LENGTHS FIRST. Measured: `timingSafeEqual` THROWS
    // `Input buffers must have the same byte length` on lengths that
    // differ. A truncated hash in the database would therefore make the
    // verification throw instead of returning `false`.
    if (calculee.length !== analyse.empreinte.length) return false;
    return timingSafeEqual(calculee, analyse.empreinte);
}

/// True if the hash was produced with weaker parameters than the
/// current ones — in which case the next sign-in must replace it.
export function doitEtreRehache(encode: string): boolean {
    let analyse: ReturnType<typeof analyser>;
    try {
        analyse = analyser(encode);
    } catch {
        // Unreadable: to be rewritten, certainly.
        return true;
    }
    if (analyse.algo !== ALGO) return true;
    return (
        analyse.params.N < PARAMETRES_COURANTS.N ||
        analyse.params.r < PARAMETRES_COURANTS.r ||
        analyse.params.p < PARAMETRES_COURANTS.p
    );
}
