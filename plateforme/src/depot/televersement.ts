// The `televersement` repository: the row of a file uploaded by a user.
//
// 🔴 THE CLOCK IS A PARAMETER, never read here — rule of `depot/agent.ts`,
// `depot/session.ts`, `depot/application.ts` and of this whole repository.
//
// 🔴 NO LITERAL VALUE in the SQL: everything goes in as a parameter, `null`
// included, otherwise `rendreMarqueurs` would throw on the Postgres side.
// ⚠️ This half of the lint bites ONLY on the Postgres path — the static lint
// of `base/sous-ensemble.test.ts` only sweeps the `.sql` files. A faulty query
// written here would be green under `test:sqlite` alone.
//
// 🔴 THIS MODULE DECIDES NOTHING, AND ABOVE ALL NOT THE SLICING. The slice
// rule lives in `proto/ts/tranches.ts`, PURE and SHARED with the
// browser: two independent arithmetics would diverge one day, and the
// symptom would be a sealing that refuses without anybody knowing which of the two
// ends is wrong. Here we write what we are given.
//
// 🔴 THE SLICES ARE NOT IN THIS TABLE, and that is decision D7:
// resumption is a directory LISTING, never bookkeeping that
// could diverge from the disk. A `tranches_presentes` column would be
// exactly that second source of truth.

import { randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';

export interface LigneTeleversement {
    id: string;
    utilisateur_id: string;
    /// The name as the BROWSER announces it. It is sanitised only when it
    /// becomes a path, on the agent side — never here, where it is only data.
    nom: string;
    /// ✅ `number` IS TRUE ON BOTH ENGINES, and it has not always
    /// been: `pg` returns `BIGINT`s as strings, and this declaration would have been
    /// FALSE in production without the `setTypeParser` of
    /// `base/pilote-postgres.ts` (P3 acceptance run). Since `interroger<T>` does an
    /// `as T[]`, no typing would catch it.
    taille: number;
    /// The hash of the WHOLE file — a single value, comparable everywhere,
    /// including by a human with a `sha256sum`.
    sha256: string;
    /// Frozen at creation: the slicing must not change under the slices
    /// already uploaded.
    taille_tranche: number;
    cree_a: number;
    /// `null` = not sealed yet. ⚠️ It is NOT `0`, which would read as an
    /// epoch of 1970 — same reasoning as `application.disparue_a`.
    scelle_a: number | null;
}

export async function creer(
    p: Pilote,
    entree: {
        utilisateurId: string;
        nom: string;
        taille: number;
        sha256: string;
        tailleTranche: number;
    },
    maintenant: number,
): Promise<LigneTeleversement> {
    const ligne: LigneTeleversement = {
        id: randomUUID(),
        utilisateur_id: entree.utilisateurId,
        nom: entree.nom,
        taille: entree.taille,
        sha256: entree.sha256,
        taille_tranche: entree.tailleTranche,
        cree_a: maintenant,
        scelle_a: null,
    };
    await p.executer(
        'INSERT INTO televersement(id,utilisateur_id,nom,taille,sha256,taille_tranche,cree_a,scelle_a)'
            + ' VALUES(?,?,?,?,?,?,?,?)',
        [
            ligne.id,
            ligne.utilisateur_id,
            ligne.nom,
            ligne.taille,
            ligne.sha256,
            ligne.taille_tranche,
            ligne.cree_a,
            ligne.scelle_a,
        ],
    );
    return ligne;
}

export async function lireParId(
    p: Pilote,
    id: string,
): Promise<LigneTeleversement | undefined> {
    const lignes = await p.interroger<LigneTeleversement>(
        'SELECT id, utilisateur_id, nom, taille, sha256, taille_tranche, cree_a, scelle_a'
            + ' FROM televersement WHERE id = ?',
        [id],
    );
    return lignes[0];
}

/// Marks the sealing. **The hash has ALREADY been recomputed** by the caller:
/// this module does not judge it.
export async function sceller(p: Pilote, id: string, maintenant: number): Promise<void> {
    await p.executer('UPDATE televersement SET scelle_a = ? WHERE id = ?', [maintenant, id]);
}

/// How many UNSEALED uploads a user has in progress.
///
/// 🔴 IT IS A QUOTA, NOT A THROTTLE, and the distinction is written in
/// decision D8 of the plan: the throttle (`securite/frein.ts`) exists for the
/// PRE-AUTHENTICATED doors, where an anonymous peer guesses a secret. These routes
/// all require a valid token; what protects them is a bound on what
/// an AUTHENTICATED user can make the disk work on.
export async function compterEnCours(p: Pilote, utilisateurId: string): Promise<number> {
    const lignes = await p.interroger<{ n: number }>(
        'SELECT COUNT(*) AS n FROM televersement WHERE utilisateur_id = ? AND scelle_a IS NULL',
        [utilisateurId],
    );
    return Number(lignes[0]?.n ?? 0);
}

/// The uploads older than `avant`, for the age sweep.
///
/// ⚠️ IT ALSO RETURNS THE SEALED ONES: a sealed upload whose installation
/// succeeded no longer has any reason to occupy the disk. It is the caller that decides
/// which ones to delete — the foreign key will refuse those an installation
/// still references, and **this is intended**: the history of an installation must
/// remain readable.
export async function lirePlusVieuxQue(
    p: Pilote,
    avant: number,
): Promise<LigneTeleversement[]> {
    return p.interroger<LigneTeleversement>(
        'SELECT id, utilisateur_id, nom, taille, sha256, taille_tranche, cree_a, scelle_a'
            + ' FROM televersement WHERE cree_a < ?',
        [avant],
    );
}

export async function supprimer(p: Pilote, id: string): Promise<void> {
    await p.executer('DELETE FROM televersement WHERE id = ?', [id]);
}
