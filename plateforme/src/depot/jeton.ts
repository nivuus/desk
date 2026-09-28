// The refresh chain: rotated on each use, hashed in the database, and
// detecting replay.
//
// 🔴 WHY SHA-256 HERE WHEREAS THE PASSWORD USES `scrypt`.
// `scrypt` is slow BY DESIGN, to make a dictionary attack on a
// low-entropy secret expensive. A randomly drawn 256-bit token
// has no dictionary: hashing here only serves so that a leak
// of the database does not make the tokens usable, and SHA-256 is enough for that.
// **The cost is named**: if one day a refresh token became
// derived from a human secret, this decision would have to be reopened.
//
// 🔴 THE REPLAY RULE, and why it requires TWO columns that spec §5
// did not have. Without `famille` or `remplace_par`, nothing links a rotated token
// to its successor: presenting an already rotated token could only revoke
// the ALREADY revoked row, and the thief who rotated first would keep their
// new token. The detection would protect nothing.
//
// ⚠️ `rejeu` and `revoque` are told apart by `remplace_par`, and this
// distinction is not cosmetic: a row revoked WITH a successor was
// legitimately rotated — presenting it again is a REPLAY, hence a
// compromise, hence the family falls. A row revoked WITHOUT a successor was
// never rotated: it is a dead token (logout, administrative
// revocation), and there is nothing more to revoke. Without it, the
// `revoque` reason would be an unreachable variant — a check that cannot
// fail, in another form.

import { createHash, randomBytes, randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';

export type MotifRafraichissement = 'inconnu' | 'expire' | 'rejeu' | 'revoque';
export type IssueRotation =
    | { ok: true; clair: string; userId: string }
    | { ok: false; motif: MotifRafraichissement };

/// 30 days. ⚠️ UNCALIBRATED: no measurement has judged it. It joins the
/// list of uncalibrated constants of the repository.
export const DUREE_RAFRAICHISSEMENT_MS = 30 * 24 * 60 * 60 * 1000;

/// 32 bytes, that is 256 bits of entropy. It is what allows SHA-256 rather
/// than `scrypt` — see the header.
const OCTETS_CLAIR = 32;

interface LigneJeton {
    id: string;
    utilisateur_id: string;
    famille: string;
    remplace_par: string | null;
    /// ⚠️ THESE TWO FIELDS WERE DECLARED `number | string`, AND IT WAS THE
    /// LOCAL SYMPTOM OF A CLASS DEFECT: `pg` returned every `BIGINT` as a
    /// string, and this module got away with it through a `Number(...)` at the point of use. The
    /// other repositories, for their part, declared `number` without converting — and
    /// `LigneAgent.vu_a` turned out to be a `string` in production during the
    /// P3 acceptance run. The defect is fixed IN THE DRIVER
    /// (`base/pilote-postgres.ts`, `setTypeParser`), so the declaration
    /// becomes true again and the local conversion goes away. What HOLDS it
    /// now is `base/pilotes.test.ts`, which tests the seven `BIGINT`
    /// columns of the schema, `jeton_rafraichissement.expire_a` included.
    expire_a: number;
    revoque_a: number | null;
}

function newPlaintext(): string {
    return randomBytes(OCTETS_CLAIR).toString('base64url');
}

/// The stored hash. The cleartext NEVER enters the database.
function empreinteDe(clair: string): string {
    return createHash('sha256').update(clair).digest('base64url');
}

async function lireParEmpreinte(p: Pilote, empreinte: string): Promise<LigneJeton | undefined> {
    const lignes = await p.interroger<LigneJeton>(
        'SELECT id, utilisateur_id, famille, remplace_par, expire_a, revoque_a FROM jeton_rafraichissement WHERE empreinte = ?',
        [empreinte],
    );
    return lignes[0];
}

async function inserer(
    p: Pilote,
    id: string,
    userId: string,
    famille: string,
    clair: string,
    maintenant: number,
): Promise<void> {
    await p.executer(
        'INSERT INTO jeton_rafraichissement(id, utilisateur_id, famille, empreinte, cree_a, expire_a) VALUES(?, ?, ?, ?, ?, ?)',
        [id, userId, famille, empreinteDe(clair), maintenant, maintenant + DUREE_RAFRAICHISSEMENT_MS],
    );
}

/// Opens a NEW family — it is the gesture of signing in. Returns the token IN
/// CLEAR, the one and only time it exists outside the browser.
export async function emettre(
    p: Pilote,
    userId: string,
    maintenant: number,
): Promise<string> {
    const clair = newPlaintext();
    await inserer(p, randomUUID(), userId, randomUUID(), clair, maintenant);
    return clair;
}

/// Revokes a whole family and returns the NUMBER of still live rows
/// it mowed down. The `revoque_a IS NULL` clause makes the call idempotent:
/// a second revocation does not move the instant of the first.
export async function revoquerFamille(
    p: Pilote,
    famille: string,
    maintenant: number,
): Promise<number> {
    const r = await p.executer(
        'UPDATE jeton_rafraichissement SET revoque_a = ? WHERE famille = ? AND revoque_a IS NULL',
        [maintenant, famille],
    );
    return r.lignes;
}

/// Rotates a token: revokes the presented one, issues a new one in the SAME
/// family, all in ONE SINGLE transaction.
///
/// ⚠️ `genererClair` is a TEST SEAM, and nothing more. It exists
/// so that a test can make the new insertion fail — on the UNIQUE index
/// of `empreinte` — and check that the transaction does roll back the revocation
/// of the old one. Without it, the partial failure path would be code never
/// run, and this repository has paid several times for fallback paths that
/// had never run. Production never passes it.
export async function tourner(
    p: Pilote,
    clair: string,
    maintenant: number,
    genererClair: () => string = newPlaintext,
): Promise<IssueRotation> {
    return p.transaction(async (tx) => {
        const ligne = await lireParEmpreinte(tx, empreinteDe(clair));
        if (!ligne) return { ok: false, motif: 'inconnu' } as const;

        if (ligne.revoque_a !== null && ligne.revoque_a !== undefined) {
            if (ligne.remplace_par === null || ligne.remplace_par === undefined) {
                // Revoked without ever having been rotated: dead token, not replay.
                return { ok: false, motif: 'revoque' } as const;
            }
            // 🔴 REPLAY: this token has ALREADY been used to obtain a new one. Either
            // the legitimate bearer is replaying, or a thief — indistinguishable,
            // and the doubt is settled on the safe side: the whole family falls,
            // including the new token the thief holds.
            await revoquerFamille(tx, ligne.famille, maintenant);
            return { ok: false, motif: 'rejeu' } as const;
        }

        if (ligne.expire_a <= maintenant) {
            return { ok: false, motif: 'expire' } as const;
        }

        const idNeuf = randomUUID();
        const clairNeuf = genererClair();
        await tx.executer(
            'UPDATE jeton_rafraichissement SET revoque_a = ?, remplace_par = ? WHERE id = ?',
            [maintenant, idNeuf, ligne.id],
        );
        await inserer(tx, idNeuf, ligne.utilisateur_id, ligne.famille, clairNeuf, maintenant);
        return { ok: true, clair: clairNeuf, userId: ligne.utilisateur_id } as const;
    });
}
