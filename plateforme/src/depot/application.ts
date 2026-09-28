// The `application` repository: read the catalogue of a VM, and apply a
// merge to it.
//
// 🔴 THE CLOCK IS A PARAMETER, never read here — same rule as
// `depot/agent.ts`, `depot/session.ts` and `depot/utilisateur.ts`, and it is what
// makes `application.test.ts` able to assert an EXACT epoch.
//
// 🔴 NO LITERAL VALUE in the SQL: everything goes in as a parameter, `null`
// included, otherwise `rendreMarqueurs` would throw on the Postgres side
// (`base/pilote.ts`). ⚠️ This half of the lint bites ONLY on the Postgres
// path — the static lint of `base/sous-ensemble.test.ts` only sweeps the
// `.sql` files. A faulty query written here would therefore be green under `test:sqlite`
// alone.
//
// 🔴 THIS MODULE DECIDES NOTHING. What to insert, update, mark
// gone or bring back is decided by `apps/catalogue.ts`, which is PURE. Here
// we write, and nothing else: a rule living in this layer would
// only be testable by a test that goes through an SQL engine.
//
// 🔴 A ROW IS NEVER DELETED. `disparue_a` is set and the row
// stays: an application installed on the browser side carries the identifier of its
// row, and a `DELETE` followed by a reinsertion on reappearance would give it
// another one. There is, in this whole module, no `DELETE`.

import { randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';
import type { Connue, Fusion } from '../apps/catalogue';
import type { SourceMax } from '../../../proto/ts/plateforme';

export interface LigneApplication {
    id: string;
    vm_id: string;
    nom: string;
    /// The path of the `.lnk` ITSELF, and it is what gets launched.
    chemin: string;
    /// The last reconciliation that saw this application.
    ///
    /// ✅ `number` IS TRUE ON BOTH ENGINES, and it has not always
    /// been: `pg` returns `BIGINT`s as strings, and this declaration would have been
    /// FALSE in production without the `setTypeParser` of
    /// `base/pilote-postgres.ts` (P3 acceptance run). Since `interroger<T>` does an
    /// `as T[]`, no typing would catch it — it is `pilotes.test.ts` that
    /// holds it, column by column, and `application.test.ts` at the point
    /// of use.
    vue_a: number;
    /// The hash of the triplet `(cible, arguments, repertoire)` — the identity in
    /// the agent's sense, unique per VM (`application_cle`).
    cle: string;
    cible: string;
    /// RAW and case-sensitive. Empty = `''`, never NULL.
    arguments: string;
    repertoire: string;
    /// The FIRST sighting. ⚠️ WRITTEN HERE AND READ BY NOBODY before sub-block
    /// G3: it exists because a NOT NULL column can no longer be
    /// added once the table is populated (see `0004-applications.sql`).
    apparue_a: number;
    /// `null` = live. ⚠️ It is NOT `0`: zero would read as an epoch
    /// of 1970, and the two states are distinct — same reasoning as
    /// `agent_enrole.vu_a`.
    disparue_a: number | null;
    /// The explicit gesture that hides an entry. ⚠️ NO WRITER IN G1.
    masquee_a: number | null;
    /// The SHA-256 hash of the PNG, in lowercase hexadecimal. `null` =
    /// the extraction failed, and that is NOT an error.
    icone: string | null;
    /// 🔴 `null` = `SourceMax.NonMesuree`, NEVER `0` NOR `256`. The column is
    /// `INTEGER`: it cannot carry the word `non-mesuree`. See
    /// the three-case invariant of `0005-icones.sql`, and the fourth
    /// combination that is FORBIDDEN there.
    source_max_px: number | null;
    /// The dominant colour of the icon, as `#rrggbb`, or `null`.
    ///
    /// ⚠️ `null` MEANS "NO ACCENT", NEVER "NOT MEASURED YET": an
    /// icon too pale, too dark or too transparent has no dominant colour,
    /// and the pure rule of the agent returns `None` by construction. The manifest
    /// then OMITS `theme_color` rather than making one up.
    accent: string | null;
}

/// Rebuilds the wire `SourceMax` from the two columns.
///
/// 🔴 WRITTEN ONLY ONCE, HERE, AND THIS IS DELIBERATE: two rebuilds
/// would diverge the day one of them decided that `null` means `0`. It is
/// criterion ④ all the way down the chain — `NonMesuree` is NEVER returned
/// as a number.
export function sourceMaxDepuis(px: number | null): SourceMax {
    return px === null ? 'non-mesuree' : { pixels: px };
}

/// The reverse: what is written in the column for a wire `SourceMax`.
export function pxDepuisSourceMax(source: SourceMax): number | null {
    return source === 'non-mesuree' ? null : source.pixels;
}

/// The columns are LISTED, never `SELECT *`: a column added one
/// day would not show up by itself in a type that does not declare it.
const COLONNES =
    'id, vm_id, nom, chemin, vue_a, cle, cible, arguments, repertoire, apparue_a, disparue_a,'
    + ' masquee_a, icone, source_max_px, accent';

/// Rewrites the associations of an application: we erase, we put back.
///
/// 🔴 ERASE THEN PUT BACK, AND NOT RECONCILE. These are a few extensions per
/// application, the agent returns them **sorted and deduplicated**, and a diff would cost
/// more to write and to review than the replacement. Above all: a diff that got it
/// wrong would leave a STALE association, that is, a `file_handler`
/// that would open a file with the wrong application — a defect visible to
/// the user and invisible in the data.
///
/// ⚠️ THE `DELETE` IS ON A LINK TABLE, AND NOT ON `application` —
/// of which the file writes, twice, that it knows NONE. A link
/// row has no history to preserve: it describes a current state.
async function ecrireAssociations(
    tx: { executer(sql: string, parametres?: unknown[]): Promise<unknown> },
    applicationId: string,
    extensions: readonly string[],
): Promise<void> {
    await tx.executer('DELETE FROM application_association WHERE application_id = ?', [
        applicationId,
    ]);
    for (const extension of extensions) {
        await tx.executer(
            'INSERT INTO application_association(application_id, extension) VALUES(?, ?)',
            [applicationId, extension],
        );
    }
}

/// The associations of several applications, in ONE query.
///
/// 🔴 ONE QUERY, AND NOT ONE PER APPLICATION. The VM corpus carries 156
/// applications; querying them one by one would make 156 round trips per
/// display of the hub. ⚠️ The markers are **generated from the number
/// of identifiers**, never concatenated from their values — `rendreMarqueurs`
/// refuses any SQL carrying an apostrophe anyway, and that is what makes
/// the rule "every value goes in as a parameter" mechanical rather than
/// documentary.
export async function associationsDe(
    p: Pilote,
    ids: readonly string[],
): Promise<Map<string, string[]>> {
    const par = new Map<string, string[]>();
    if (ids.length === 0) return par;
    const marqueurs = ids.map(() => '?').join(', ');
    const lignes = await p.interroger<{ application_id: string; extension: string }>(
        `SELECT application_id, extension FROM application_association`
            + ` WHERE application_id IN (${marqueurs}) ORDER BY application_id, extension`,
        [...ids],
    );
    for (const l of lignes) {
        const deja = par.get(l.application_id);
        if (deja === undefined) par.set(l.application_id, [l.extension]);
        else deja.push(l.extension);
    }
    return par;
}

/// The DISPLAYABLE catalogue of a VM: neither the gone ones, nor the hidden ones.
///
/// 🔴 IT IS NOT THE MERGE READER, and the two cannot be the
/// same — see `lireConnues`.
export async function lireParVm(p: Pilote, vmId: string): Promise<LigneApplication[]> {
    return p.interroger<LigneApplication>(
        `SELECT ${COLONNES} FROM application`
            + ' WHERE vm_id = ? AND disparue_a IS NULL AND masquee_a IS NULL'
            + ' ORDER BY nom',
        [vmId],
    );
}

/// Returns the row, or `undefined`. NEVER an exception on an unknown
/// identifier: an exception bubbling up as a 500 would on its own be an
/// oracle. Precedents: `depot/agent.ts::lireParVm`, `depot/vm.ts::lireParId`.
///
/// ⚠️ IT FILTERS NEITHER THE GONE NOR THE HIDDEN ONES, on purpose: its caller
/// is the launch route, which must be able to tell "unknown" from
/// "known but no longer there", and its other caller is the test that checks
/// that a disappearance did not erase the row.
export async function lireParId(p: Pilote, id: string): Promise<LigneApplication | undefined> {
    const lignes = await p.interroger<LigneApplication>(
        `SELECT ${COLONNES} FROM application WHERE id = ?`,
        [id],
    );
    return lignes[0];
}

/// What the merge needs to know, and NOTHING MORE.
///
/// 🔴 IT ALSO RETURNS THE GONE ONES, and that is what sets it apart from
/// `lireParVm`. A merge that did not see them would REINSERT them on their
/// return, with a new identifier — that is, exactly the loss of
/// identifier this module exists to prevent.
export async function lireConnues(p: Pilote, vmId: string): Promise<Connue[]> {
    return p.interroger<Connue>(
        'SELECT id, cle, disparue_a FROM application WHERE vm_id = ?',
        [vmId],
    );
}

/// Applies a merge, IN FULL OR NOT AT ALL.
///
/// 🔴 THE TRANSACTION IS NOT DECORATIVE. A half-written catalogue is
/// indistinguishable from a correct one at the next round: the next
/// reconciliation would take it for the state of the VM, and the missing rows would
/// only come back at the next full send — or never, if the agent no longer
/// emits one. It is the class of silent failure this whole repository
/// is written against, and it is the same reasoning as that of `migrations.ts`.
///
/// 🔴 THE IDENTIFIER IS GENERATED HERE, never received from the agent. The key is
/// the hash of a triplet of Windows paths: two VMs carrying the same
/// application produce the SAME key, and an identifier derived from it
/// would collide from one VM to another.
export async function appliquer(
    p: Pilote,
    vmId: string,
    fusion: Fusion,
    maintenant: number,
): Promise<void> {
    await p.transaction(async (tx) => {
        for (const app of fusion.aInserer) {
            // 🔴 THE IDENTIFIER IS NAMED BEFORE THE INSERT, because the
            // associations need it: `randomUUID()` written inline
            // would return a value that could no longer be referred to.
            const identifiant = randomUUID();
            await tx.executer(
                // 🔴 FOURTEEN MARKERS FOR FOURTEEN COLUMNS. A miscounted `INSERT`
                // THROWS on BOTH engines — it is the cheapest guard in the
                // file, and it is free.
                // 🔴 FIFTEEN MARKERS FOR FIFTEEN COLUMNS — fourteen until
                // G5, which adds `accent`. A miscounted `INSERT` THROWS on
                // BOTH engines: it is the cheapest guard in the file, and
                // it is free.
                `INSERT INTO application(${COLONNES})`
                    + ' VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)',
                [
                    identifiant,
                    vmId,
                    app.nom,
                    app.chemin,
                    maintenant,
                    app.cle,
                    app.cible,
                    app.arguments,
                    app.repertoire,
                    // `apparue_a` and `vue_a` are born equal: it is the same
                    // sighting. They diverge at the next update, and that is
                    // the whole point of the first one.
                    maintenant,
                    null,
                    null,
                    app.icone,
                    pxDepuisSourceMax(app.source_max),
                    app.accent,
                ],
            );
            await ecrireAssociations(tx, identifiant, app.associations);
        }

        for (const { id, app } of fusion.aMettreAJour) {
            // ⚠️ `apparue_a` IS NOT IN THIS `SET`. Advancing it would make it a
            // duplicate of `vue_a`, and the installation verdict that will read it one
            // day would never again see an appearance.
            //
            // ⚠️ Nor is `cle`, and for a different reason: it is through
            // it that the row was found, and it is the identity. Rewriting
            // it would have no effect in the best case, and
            // would violate `application_cle` in the worst.
            // ⚠️ `icone` AND `source_max_px` ARE IN THIS `SET`, unlike
            // `cle` and `apparue_a`. An icon CHANGES when the application
            // updates, and that is the nominal case, not the exception:
            // omitting them would mean a new icon never reaches the database,
            // and the only symptom would be a stale image that nothing
            // would explain.
            await tx.executer(
                // ⚠️ `accent` IS IN THIS `SET`, AND FOR THE SAME REASON AS
                // `icone`: it DERIVES from it. An icon that changes changes its
                // accent, and omitting it would leave a stale colour that nothing
                // would explain.
                'UPDATE application SET nom = ?, chemin = ?, cible = ?, arguments = ?,'
                    + ' repertoire = ?, vue_a = ?, icone = ?, source_max_px = ?, accent = ?'
                    + ' WHERE id = ?',
                [
                    app.nom,
                    app.chemin,
                    app.cible,
                    app.arguments,
                    app.repertoire,
                    maintenant,
                    app.icone,
                    pxDepuisSourceMax(app.source_max),
                    app.accent,
                    id,
                ],
            );
            await ecrireAssociations(tx, id, app.associations);
        }

        for (const id of fusion.aMarquerDisparues) {
            // ⚠️ NO `DELETE`, here or anywhere else in this file.
            await tx.executer('UPDATE application SET disparue_a = ? WHERE id = ?', [maintenant, id]);
        }

        for (const id of fusion.aRessusciter) {
            // The `null` goes in as a PARAMETER like any value. It is not
            // `rendreMarqueurs` that requires it — `NULL` is a keyword, not a
            // string literal, and a hardcoded `SET disparue_a = NULL`
            // would pass. It is the general rule of the repository, whose usage
            // `depot/vm.ts::detacher` established: a single form
            // of writing, so that no reader has to wonder which
            // of the two they are looking at.
            await tx.executer('UPDATE application SET disparue_a = ? WHERE id = ?', [null, id]);
        }
    });
}
