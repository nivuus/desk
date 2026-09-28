// The `vm` repository: the inventory that P4 reads, and the ONLY column the
// service owns — `vm.utilisateur_id`. (policy: allow-fr - frozen wire key or SQLite column)
//
// 🔴 THIS MODULE CREATES NO VM, and that is the meaning of the word "static" of
// spec §3.6 as reread by D1: the backend drives no hypervisor. The only
// creation path remains `admin/enroler-agent.ts`, and P4 adds no
// other. What it adds is `admin:attribuer`, which creates nothing either:
// it sets an owner on an already enrolled VM.
//
// 🔴 NO LITERAL VALUE in the SQL: everything goes in as a parameter, `null`
// included, otherwise `rendreMarqueurs` would throw on the Postgres side
// (`base/pilote.ts`). ⚠️ This half of the lint bites ONLY on the Postgres
// path — the static lint of `base/sous-ensemble.test.ts` only sweeps (policy: allow-fr - file name)
// the `.sql` files. A faulty query written here would therefore be green under
// `test:sqlite` alone.
//
// 🔴 THE CLOCK IS NOT READ HERE, and there is not even a need for it: this repository
// writes no timestamp. `vm.vue_a` exists in the schema and this module DOES NOT
// SELECT IT — see `COLUMNS`.

import type { Pilote } from '../base/pilote';

/// An inventory row: the VM, and what its enrolment says about it.
export interface LigneVm {
    id: string;
    nom: string;
    adresse: string;
    /// `null` = in the pool. ⚠️ "Belonging to nobody" is not "belonging to everybody":
    /// it is `orchestration/selection.ts` that carries this rule.
    utilisateur_id: string | null; // policy: allow-fr - frozen wire key or SQLite column
    /// `null` if the VM was never enrolled as an agent — hence the LEFT JOIN.
    prefixe_session: string | null;
    /// The last heartbeat. `null` if never seen.
    ///
    /// ✅ `number` IS TRUE ON BOTH ENGINES, and it has not always
    /// been: `pg` returned `BIGINT`s as strings until the `setTypeParser` of
    /// `base/pilote-postgres.ts` (P3 acceptance run). Since `interroger<T>` does an
    /// `as T[]`, no typing would have caught it — it is `vm.test.ts` that
    /// holds it, at the point of use, and `pilotes.test.ts` column by column.
    vu_a: number | null;
}

/// The columns are LISTED, never `SELECT *`.
///
/// 🔴 `vm.vue_a` IS DELIBERATELY MISSING FROM THEM. It exists in
/// `0001-socle.sql`, and it is an ORPHAN: P3 created `agent_enrole.vu_a` in
/// its place, and NO production code writes `vm.vue_a`. Selecting it
/// would make a successor believe it is filled in, and they would read
/// `null`s thinking they were reading a silence. It is the only cheap guard against
/// this confusion, and it is worth writing here rather than hoping for.
const COLUMNS =
    'v.id, v.nom, v.adresse, v.utilisateur_id, a.prefixe_session, a.vu_a';

/// 🔴 A `LEFT JOIN`, NEVER A `JOIN`. A VM created without enrolment — which
/// nothing forbids, the two tables being written by two distinct statements
/// of `admin/enroler-agent.ts` — WOULD VANISH from the inventory under
/// a `JOIN`, without any error saying so. An inventory that loses
/// rows silently is exactly the silent failure this repository fights.
const DEPUIS = 'FROM vm v LEFT JOIN agent_enrole a ON a.vm_id = v.id';

/// The WHOLE inventory. Filtering by user is the job of a
/// PURE module (`orchestration/selection.ts`): doing it here would put the rule out of
/// reach of a test that opens no database.
export async function lister(p: Pilote): Promise<LigneVm[]> {
    return p.interroger<LigneVm>(`SELECT ${COLUMNS} ${DEPUIS} ORDER BY v.nom`, []);
}

/// Returns the row, or `undefined`. NEVER an exception on an unknown VM:
/// an exception bubbling up as a 500 would on its own be an enumeration
/// oracle. Precedent: `depot/agent.ts::lireParVm`.
export async function lireParId(p: Pilote, id: string): Promise<LigneVm | undefined> {
    const lignes = await p.interroger<LigneVm>(
        `SELECT ${COLUMNS} ${DEPUIS} WHERE v.id = ?`,
        [id],
    );
    return lignes[0];
}

/// Same, by NAME. ⚠️ `vm.nom` is NOT unique (`0001-socle.sql`): two
/// enrolments with the same name are possible, and this function then returns the
/// first row. That is acceptable for the only caller — the administration
/// command, whose operator knows the name they gave — and it would not
/// be on a route.
export async function lireParNom(p: Pilote, nom: string): Promise<LigneVm | undefined> {
    const lignes = await p.interroger<LigneVm>(
        `SELECT ${COLUMNS} ${DEPUIS} WHERE v.nom = ?`,
        [nom],
    );
    return lignes[0];
}

/// Assigns the VM IF IT IS FREE, and returns the number of rows touched.
///
/// 🔴 `AND utilisateur_id IS NULL` IS THE GUARANTEE, and it is NOT the prior (policy: allow-fr - frozen wire key or SQLite column)
/// read the caller does. The clause is RE-EVALUATED BY THE ENGINE at the
/// moment of the write: that is what makes the race safe. Measured on
/// PostgreSQL 16.15 in `READ COMMITTED`, two transactions targeting the same free
/// VM — A gets `1 row`, B BLOCKS on the row lock, then returns
/// `0 rows` after A's `COMMIT`, Postgres re-evaluating the clause on the
/// updated row. Exactly one winner, no exception, no
/// overwrite.
///
/// 🔴 WITHOUT IT, THE THEFT GOES THROUGH. Measured on BOTH engines: a bare `UPDATE vm SET
/// policy: allow-fr (frozen SQLite name) — utilisateur_id = ? WHERE id = ?` returns `1 row` on a VM already
/// assigned to somebody else. ⚠️ The partial index `vm_un_utilisateur` (policy: allow-fr - frozen wire key or SQLite column)
/// DOES NOT FORBID THIS THEFT — it makes `utilisateur_id` unique across (policy: allow-fr - frozen wire key or SQLite column)
/// rows, so it forbids a user from having TWO VMs, never a VM from
/// changing hands. The spec and `0001-socle.sql` both ascribed to that same
/// index a property it does not have (divergence E3).
///
/// ⚠️ `0` CONFLATES THREE CAUSES: unknown VM, VM already taken, reassignment to the
/// same user. It is the caller that tells them apart, by reading first —
/// and it tells them apart ONLY where that is legitimate, never on a public
/// route where it would be an oracle (E8, D8).
///
/// ⚠️ IT THROWS when the user already has another VM: that is the partial
/// index, and the text of the exception DIFFERS by engine. No caller
/// must compare it.
export async function attribuerSiLibre(
    p: Pilote,
    vmId: string,
    userId: string,
): Promise<number> {
    const r = await p.executer(
        'UPDATE vm SET utilisateur_id = ? WHERE id = ? AND utilisateur_id IS NULL',
        [userId, vmId],
    );
    return r.lignes;
}

/// Returns the VM to the pool, and returns the number of rows touched.
///
/// ⚠️ NO CLAUSE ON THE CURRENT OWNER, on purpose: the only caller
/// is the administration command, whose purpose is precisely to take
/// a VM back from somebody. The day a user could give back THEIR VM
/// themselves, this function would need an `AND utilisateur_id = ?` — (policy: allow-fr - frozen wire key or SQLite column)
/// otherwise they would give back somebody else's.
///
/// The `null` goes in as a PARAMETER like any value.
export async function detacher(p: Pilote, vmId: string): Promise<number> {
    const r = await p.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', [null, vmId]);
    return r.lignes;
}
