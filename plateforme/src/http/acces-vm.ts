// Whether a user may act on a VM: the ONE ownership rule of the human routes
// that name a VM (`routes-applications.ts`, `routes-installation.ts`).
//
// The rule used to live in one copy per route, and the copies agreed with
// each other but not with `GET /vm` (`orchestration/selection.ts::vmsDe`).
// It lives here so that every route gives the same verdict for the same VM.

import type { Pilote } from '../base/pilote';
import { lireParId as lireVm } from '../depot/vm';

/// `inconnue` and `etrangere` stay distinct INSIDE the service: they feed the
/// log line. The routes answer both with the same `404 vm-inconnue`.
export type VerdictAcces = 'ok' | 'inconnue' | 'etrangere';

/// The trade-off of the indistinguishable refusal: the only place that says
/// WHICH case happened. `cas=` is a field the operator greps for, and it must
/// never reach the HTTP response.
function refuser(cas: string, vmId: string, userId: string): void {
    console.warn(
        `access to VM ${vmId} refused for user ${userId}: cas=${cas} — the HTTP `
            + 'response itself is the same 404 « vm-inconnue » in every case '
            + '(decision of the repository owner: no enumeration oracle).',
    );
}

export async function accesVm(base: Pilote, vmId: string, userId: string): Promise<VerdictAcces> {
    const vm = await lireVm(base, vmId);
    if (vm === undefined) {
        refuser('inconnue', vmId, userId);
        return 'inconnue';
    }
    // An UNASSIGNED VM (after a detach, a failed assignment, an orphan row)
    // belongs to NOBODY, hence is foreign to everyone: the verdict `GET /vm`
    // already gives. Granting it would let any authenticated account list,
    // launch on, or push an installer to a machine that is not theirs.
    if (vm.utilisateur_id === null) { // policy: allow-fr - frozen wire key or SQLite column
        refuser('unassigned', vmId, userId);
        return 'etrangere';
    }
    if (vm.utilisateur_id !== userId) { // policy: allow-fr - frozen wire key or SQLite column
        refuser('etrangere', vmId, userId);
        return 'etrangere';
    }
    return 'ok';
}
