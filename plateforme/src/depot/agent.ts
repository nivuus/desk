// The `agent_enrole` repository: enrol a VM, read it back by its identifier or
// by its prefix, and mark that we just heard it beat.
//
// 🔴 THE CLOCK IS A PARAMETER, never read here — same rule as
// `depot/session.ts`, `depot/utilisateur.ts` and `signaling/ice.ts`, and it is
// what makes `agent.test.ts` able to assert an EXACT epoch.
//
// 🔴 NO LITERAL VALUE in the SQL: everything goes in as a parameter, otherwise
// `rendreMarqueurs` would throw on the Postgres side (`base/pilote.ts`).
//
// ⚠️ THIS MODULE KNOWS NOTHING ABOUT HASHING: it receives and returns an opaque
// hash, exactly like `depot/utilisateur.ts`. That is what will allow
// hardening `scrypt` without reopening it — the format carries its own parameters
// (`identite/mot-de-passe.ts`).
//
// ⚠️ IT KNOWS NOTHING ABOUT FRESHNESS EITHER: it returns `vu_a` as it is,
// `null` included. Deciding `prete` / `injoignable` is the job of a
// PURE module, with its clock as a parameter.

import type { Pilote } from '../base/pilote';

export interface LigneAgent {
    vm_id: string;
    empreinte_secret: string;
    prefixe_session: string;
    /// `null` as long as the agent has never beaten. ⚠️ It is NOT `0`: zero would
    /// read as a 1970 epoch, hence as an agent unreachable for
    /// fifty-six years, and the two states are distinct.
    ///
    /// ✅ `number` IS TRUE ON BOTH ENGINES, and it has not always
    /// been: `pg` returns `BIGINT`s as strings, and this declaration was
    /// FALSE in production until `base/pilote-postgres.ts` set its
    /// `setTypeParser` (P3 acceptance run). Since `interroger<T>` does an `as T[]`,
    /// no typing would have caught it — it is `pilotes.test.ts` that
    /// holds it, column by column.
    vu_a: number | null;
}

const COLONNES = 'vm_id, empreinte_secret, prefixe_session, vu_a';

/// Enrols a VM.
///
/// A prefix already taken makes it THROW, through the UNIQUE index of `0003-agents.sql` —
/// never a silent return: here the caller is the administrator, and hiding
/// the failure from them would make them believe in an enrolment that does not exist. Same
/// reasoning as `creerUtilisateur` on the email.
export async function enroler(
    p: Pilote,
    vmId: string,
    empreinte: string,
    prefixe: string,
): Promise<void> {
    await p.executer(
        `INSERT INTO agent_enrole(${COLONNES}) VALUES(?, ?, ?, ?)`,
        [vmId, empreinte, prefixe, null],
    );
}

/// Returns the row, or `undefined`. NEVER an exception on an unknown VM:
/// the channel must answer the SAME refusal as for a wrong secret, and an
/// exception that bubbled up as an internal error would on its own be an
/// enumeration oracle — the caller would learn by trial and error which VMs
/// exist. Precedent: `depot/utilisateur.ts::lireParEmail`.
export async function lireParVm(p: Pilote, vmId: string): Promise<LigneAgent | undefined> {
    const lignes = await p.interroger<LigneAgent>(
        `SELECT ${COLONNES} FROM agent_enrole WHERE vm_id = ?`,
        [vmId],
    );
    return lignes[0];
}

/// Returns the row whose prefix is this one, or `undefined`.
///
/// It is the ONLY key available to whoever reads a session name: `P:bureau` does not
/// carry the identifier of the VM, it carries its prefix. The uniqueness of the
/// column is what makes this return unambiguous.
export async function lireParPrefixe(
    p: Pilote,
    prefixe: string,
): Promise<LigneAgent | undefined> {
    const lignes = await p.interroger<LigneAgent>(
        `SELECT ${COLONNES} FROM agent_enrole WHERE prefixe_session = ?`,
        [prefixe],
    );
    return lignes[0];
}

/// Advances `vu_a`. The value is OVERWRITTEN, never accumulated: it is an instant,
/// not a counter.
///
/// An unknown VM writes nothing and does not throw — the `UPDATE` touches zero rows.
/// This is deliberate: this path is the heartbeat one, and it must
/// never be a reason to bring the channel down.
export async function marquerVu(p: Pilote, vmId: string, maintenant: number): Promise<void> {
    await p.executer('UPDATE agent_enrole SET vu_a = ? WHERE vm_id = ?', [maintenant, vmId]);
}

/// Replaces the hash of the enrolment secret of a VM, and NOTHING ELSE.
///
/// 🔴 `prefixe_session` IS NOT TOUCHED, AND THAT IS THE POINT OF THIS FUNCTION.
/// The prefix makes up the name of the LIVE sessions of this VM
/// (`agents/prefixe.ts`, spec §3.4): rotating it together with the
/// secret would cut every ongoing session. **Rotating the secret is not
/// rotating the identity**, and the two do not have the same urgency — a secret
/// is replaced the day it leaks, an identity is never replaced in
/// a hurry.
///
/// Returns the number of rows touched. An unknown VM touches ZERO and does NOT
/// throw: that is what lets `admin/enroler-agent.ts` return a
/// REASONED refusal rather than an exception. ⚠️ The oracle reasoning that holds for
/// `lireParVm` does NOT apply here — the caller is the administrator, not a
/// stranger at the end of a channel, and hiding from them that they picked the wrong VM would
/// make them believe in a rotation that did not happen.
///
/// ⚠️ WHAT IT DOES NOT DO: revoke the agent tokens ALREADY handed out, which
/// stay valid until they expire. Same property as the human
/// tokens (spec §3.5), and it bounds the window to `DUREE_JETON_ACCES_MS`.
export async function remplacerEmpreinte(
    p: Pilote,
    vmId: string,
    empreinte: string,
): Promise<number> {
    const r = await p.executer(
        'UPDATE agent_enrole SET empreinte_secret = ? WHERE vm_id = ?',
        [empreinte, vmId],
    );
    return r.lignes;
}
