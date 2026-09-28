// The `installation` repository: the request, its progress, and its outcome.
//
// 🔴 THE CLOCK IS A PARAMETER, never read here. 🔴 NO LITERAL VALUE
// in the SQL. Same rules as its four neighbours, and for the same reasons.
//
// 🔴 THIS MODULE DECIDES NO OUTCOME. The verdict is computed by
// `agent/src/apps/installation/verdict.rs`, PURE, on the VM: it is the one that
// knows the count of applications that appeared during the window, and it alone.
// Here we write what the agent reports.
//
// 🔴 AND THE EXIT CODE IS NOT INTERPRETED HERE EITHER. `msiexec` returns
// 3010 for a success that asks for a restart, and many installers
// return 0 after a cancellation: a `reussie BOOLEAN` column derived from the
// code would be wrong in both directions. The code is REPORTED, next to
// the outcome.

import { randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';

/// The state of the ROW, distinct from the outcome.
///
/// 🔴 IT IS WHAT GOVERNS RE-EMISSION: the platform stops pushing
/// the order as soon as the state is no longer `en_attente`. It is the FIRST of the two
/// belts against a double execution; the second is the marker on the
/// disk of the VM, and it protects against the case where the first has lost its database.
export type EtatInstallation = 'en_attente' | 'en_cours' | 'terminee';

export interface LigneInstallation {
    id: string;
    vm_id: string;
    televersement_id: string;
    demandee_a: number;
    etat: EtatInstallation;
    /// `transfert` | `execution` | `reconciliation`, or the empty string as long as
    /// nothing has started. ⚠️ `empreinte` is NOT a phase of this channel: it
    /// takes place in the browser.
    phase: string;
    octets_faits: number;
    /// Is zero in the `execution` phase, where there is nothing to total.
    octets_total: number;
    ecoule_ms: number;
    /// 🔴 `null` = THE CODE COULD NOT BE COLLECTED, and that is a fact distinct
    /// from any integer code. A `-1` sentinel would conflate them.
    code_sortie: number | null;
    issue: string | null;
    motif: string | null;
    journal: string | null;
    /// ⚠️ An integer and not a boolean: SQLite has no boolean type, and
    /// it is the convention of the rest of the schema.
    journal_tronque: number;
    terminee_a: number | null;
    maj_a: number;
}

export async function creer(
    p: Pilote,
    entree: { vmId: string; televersementId: string },
    maintenant: number,
): Promise<LigneInstallation> {
    const ligne: LigneInstallation = {
        id: randomUUID(),
        vm_id: entree.vmId,
        televersement_id: entree.televersementId,
        demandee_a: maintenant,
        etat: 'en_attente',
        phase: '',
        octets_faits: 0,
        octets_total: 0,
        ecoule_ms: 0,
        code_sortie: null,
        issue: null,
        motif: null,
        journal: null,
        journal_tronque: 0,
        terminee_a: null,
        maj_a: maintenant,
    };
    await p.executer(
        'INSERT INTO installation(id,vm_id,televersement_id,demandee_a,etat,phase,octets_faits,'
            + 'octets_total,ecoule_ms,code_sortie,issue,motif,journal,journal_tronque,terminee_a,maj_a)'
            + ' VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)',
        [
            ligne.id, ligne.vm_id, ligne.televersement_id, ligne.demandee_a, ligne.etat,
            ligne.phase, ligne.octets_faits, ligne.octets_total, ligne.ecoule_ms,
            ligne.code_sortie, ligne.issue, ligne.motif, ligne.journal,
            ligne.journal_tronque, ligne.terminee_a, ligne.maj_a,
        ],
    );
    return ligne;
}

const COLONNES =
    'id, vm_id, televersement_id, demandee_a, etat, phase, octets_faits, octets_total,'
    + ' ecoule_ms, code_sortie, issue, motif, journal, journal_tronque, terminee_a, maj_a';

export async function lireParId(
    p: Pilote,
    id: string,
): Promise<LigneInstallation | undefined> {
    const lignes = await p.interroger<LigneInstallation>(
        `SELECT ${COLONNES} FROM installation WHERE id = ?`,
        [id],
    );
    return lignes[0];
}

/// The installations a VM has yet to receive.
///
/// 🔴 IT IS THE QUERY OF THE RE-EMISSION ON ENROLMENT, and the index
/// `installation_par_vm_et_etat` exists for it. A WebSocket `push` has
/// NO delivery guarantee: without this re-emission, an order sent during
/// an outage would be lost WITHOUT END. It is the same safety net as the
/// `complet = true` of the catalogue, and the G1 acceptance run saw it work on the
/// real path.
export async function lireEnAttentePourVm(
    p: Pilote,
    vmId: string,
): Promise<LigneInstallation[]> {
    return p.interroger<LigneInstallation>(
        `SELECT ${COLONNES} FROM installation WHERE vm_id = ? AND etat = ? ORDER BY demandee_a`,
        [vmId, 'en_attente'],
    );
}

/// Records a progress reported by the agent.
///
/// ⚠️ IT MOVES THE STATE TO `en_cours`, AND THAT IS WHAT STOPS THE
/// RE-EMISSION. Without this move, the platform would replay the order at the next
/// enrolment while the installer is already running — and the agent would have to
/// defend itself alone, through its disk marker.
export async function avancer(
    p: Pilote,
    id: string,
    avancement: { phase: string; octetsFaits: number; octetsTotal: number; ecouleMs: number },
    maintenant: number,
): Promise<void> {
    await p.executer(
        'UPDATE installation SET etat = ?, phase = ?, octets_faits = ?, octets_total = ?,'
            + ' ecoule_ms = ?, maj_a = ? WHERE id = ? AND etat <> ?',
        [
            'en_cours', avancement.phase, avancement.octetsFaits, avancement.octetsTotal,
            avancement.ecouleMs, maintenant, id, 'terminee',
        ],
    );
}

/// ⚠️ `AND etat <> 'terminee'` ON BOTH WRITES, and it is not a
/// stylistic precaution: the platform RE-EMITS, so an agent can report
/// twice. Without this guard, a late progress would overwrite an outcome already
/// set and a finished installation would become "in progress" again — that is,
/// the re-emission would start over.
export async function terminer(
    p: Pilote,
    id: string,
    issue: {
        issue: string;
        motif: string | null;
        codeSortie: number | null;
        journal: string;
        journalTronque: boolean;
    },
    maintenant: number,
): Promise<void> {
    await p.executer(
        'UPDATE installation SET etat = ?, issue = ?, motif = ?, code_sortie = ?, journal = ?,'
            + ' journal_tronque = ?, terminee_a = ?, maj_a = ? WHERE id = ? AND etat <> ?',
        [
            'terminee', issue.issue, issue.motif, issue.codeSortie, issue.journal,
            issue.journalTronque ? 1 : 0, maintenant, maintenant, id, 'terminee',
        ],
    );
}
