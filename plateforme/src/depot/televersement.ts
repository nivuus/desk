// The `televersement` repository: the row of a file uploaded by a user.
//
// 🔴 THE CLOCK IS A PARAMETER, never read here — rule of `depot/agent.ts`,
// `depot/session.ts`, `depot/application.ts` and of this whole repository.
//
// 🔴 NO LITERAL VALUE in the SQL: everything goes in as a parameter, `null`
// included, otherwise `rendreMarqueurs` would throw on the Postgres side.
// ⚠️ This half of the lint bites ONLY on the Postgres path — the static lint
// of `base/sous-ensemble.test.ts` only sweeps the `.sql` files. A faulty query (policy: allow-fr - file name)
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
    utilisateur_id: string; // policy: allow-fr - frozen wire key or SQLite column
    /// The name as the BROWSER announces it. It is sanitised only when it
    /// becomes a path, on the agent side — never here, where it is only data.
    nom: string;
    /// ✅ `number` IS TRUE ON BOTH ENGINES, and it has not always
    /// been: `pg` returns `BIGINT`s as strings, and this declaration would have been
    /// FALSE in production without the `setTypeParser` of
    /// `base/pilote-postgres.ts` (P3 acceptance run). Since `interroger<T>` does an
    /// `as T[]`, no typing would catch it.
    taille: number; // policy: allow-fr - frozen wire key or SQLite column
    /// The hash of the WHOLE file — a single value, comparable everywhere,
    /// including by a human with a `sha256sum`.
    sha256: string;
    /// Frozen at creation: the slicing must not change under the slices
    /// already uploaded.
    taille_tranche: number; // policy: allow-fr - frozen wire key or SQLite column
    cree_a: number;
    /// `null` = not sealed yet. ⚠️ It is NOT `0`, which would read as an
    /// epoch of 1970 — same reasoning as `application.disparue_a`.
    scelle_a: number | null;
    /// `null` = the chunks are on disk. A date = the cleanup released them,
    /// every installation of this upload being finished (migration 0008). The
    /// row stays: the installation history names this file.
    contenu_purge_a: number | null;
}

const COLONNES =
    'id, utilisateur_id, nom, taille, sha256, taille_tranche, cree_a, scelle_a, contenu_purge_a';

export async function create(
    p: Pilote,
    entree: {
        userId: string;
        nom: string;
        taille: number; // policy: allow-fr - frozen wire key or SQLite column
        sha256: string;
        chunkSize: number;
    },
    maintenant: number,
): Promise<LigneTeleversement> {
    const ligne: LigneTeleversement = {
        id: randomUUID(),
        utilisateur_id: entree.userId, // policy: allow-fr - frozen wire key or SQLite column
        nom: entree.nom,
        taille: entree.taille, // policy: allow-fr - frozen wire key or SQLite column
        sha256: entree.sha256,
        taille_tranche: entree.chunkSize, // policy: allow-fr - frozen wire key or SQLite column
        cree_a: maintenant,
        scelle_a: null,
        contenu_purge_a: null,
    };
    await p.executer(
        'INSERT INTO televersement(id,utilisateur_id,nom,taille,sha256,taille_tranche,cree_a,scelle_a)'
            + ' VALUES(?,?,?,?,?,?,?,?)',
        [
            ligne.id,
            ligne.utilisateur_id, // policy: allow-fr - frozen wire key or SQLite column
            ligne.nom,
            ligne.taille, // policy: allow-fr - frozen wire key or SQLite column
            ligne.sha256,
            ligne.taille_tranche, // policy: allow-fr - frozen wire key or SQLite column
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
        `SELECT ${COLONNES} FROM televersement WHERE id = ?`,
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
export async function compterEnCours(p: Pilote, userId: string): Promise<number> {
    const lignes = await p.interroger<{ n: number }>(
        'SELECT COUNT(*) AS n FROM televersement WHERE utilisateur_id = ? AND scelle_a IS NULL',
        [userId],
    );
    return Number(lignes[0]?.n ?? 0);
}

/// How many bytes a user's uploads hold on disk: every upload whose chunks
/// have not been released, sealed or not. It is what `routes-televersement.ts`
/// bounds — a COUNT of uploads in progress does not protect a disk, since a
/// sealed upload leaves that count and keeps its bytes.
export async function octetsDetenus(p: Pilote, userId: string): Promise<number> {
    const lignes = await p.interroger<{ n: number | string | null }>(
        'SELECT SUM(taille) AS n FROM televersement WHERE utilisateur_id = ? AND contenu_purge_a IS NULL',
        [userId],
    );
    // ⚠️ `Number`: `SUM` over BIGINT is NUMERIC on Postgres, which `pg` returns
    // as a string — the INT8 parser of `pilote-postgres.ts` does not cover it.
    return Number(lignes[0]?.n ?? 0);
}

/// The uploads older than `before` whose chunks are still held, for the age sweep.
///
/// ⚠️ IT ALSO RETURNS THE SEALED ONES: a sealed upload whose installations are
/// finished no longer has any reason to occupy the disk. `purger` decides
/// what becomes of each one.
export async function lirePlusVieuxQue(
    p: Pilote,
    before: number,
): Promise<LigneTeleversement[]> {
    return p.interroger<LigneTeleversement>(
        `SELECT ${COLONNES} FROM televersement WHERE cree_a < ? AND contenu_purge_a IS NULL`,
        [before],
    );
}

export async function remove(p: Pilote, id: string): Promise<void> {
    await p.executer('DELETE FROM televersement WHERE id = ?', [id]);
}

/// What `purger` did to an upload.
/// - `supprime`: no installation ever named it — the row goes, and its chunks.
/// - `contenu-libere`: every installation that named it is finished — the row
///   stays for their history, the chunks go (`contenu_purge_a` is set).
/// - `retenu`: an installation still needs the bytes — nothing changes.
export type IssuePurge = 'supprime' | 'contenu-libere' | 'retenu';

/// Releases what an old upload no longer needs.
///
/// 🔴 EACH DECISION IS ONE STATEMENT, its condition inside it: reading the
/// installations first and writing afterwards would let an order placed in
/// between lose its bytes. 🔴 THIS REPLACES A FOREIGN-KEY REFUSAL THAT WAS
/// SWALLOWED AS "EXPECTED": that refusal was the defect — it kept every
/// ordered installer on disk forever, finished or not.
export async function purger(p: Pilote, id: string, maintenant: number): Promise<IssuePurge> {
    const supprime = await p.executer(
        'DELETE FROM televersement WHERE id = ?'
            + ' AND NOT EXISTS (SELECT 1 FROM installation WHERE televersement_id = ?)',
        [id, id],
    );
    if (supprime.lignes > 0) return 'supprime';
    const libere = await p.executer(
        'UPDATE televersement SET contenu_purge_a = ? WHERE id = ? AND contenu_purge_a IS NULL'
            + ' AND NOT EXISTS (SELECT 1 FROM installation WHERE televersement_id = ? AND etat <> ?)',
        [maintenant, id, id, 'terminee'],
    );
    return libere.lignes > 0 ? 'contenu-libere' : 'retenu';
}
