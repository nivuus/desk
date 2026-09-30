// The orchestrator that knows how to wake the VM.
//
// It WRAPS `inventaireStatique` rather than rewriting it: the inventory, the
// freshness and the refusals of the other verbs stay the ones that the tests of
// `inventaire-statique.test.ts` hold. It only replaces two verbs: `start`
// (requests the wake) and `etat` (says `demarrage` during a wake).

import type { Pilote } from '../base/pilote';
import type { EtatVm, Orchestrateur } from './interface';
import { inventaireStatique } from './inventaire-statique';
import { BACKEND_HOTE, refuser, type Outcome } from './refus';
import type { Wake } from './wake';

export function hostOrchestrator(base: Pilote, now: () => number, wake: Wake): Orchestrateur {
    const statique = inventaireStatique(base, now);
    return {
        ...statique,

        async etat(vm: string): Promise<EtatVm> {
            const etat = await statique.etat(vm);
            // Only a VM we cannot see can be "starting": a live VM is `prete`,
            // whatever the wake memory says.
            return etat === 'injoignable' && wake.pending(vm) ? 'demarrage' : etat;
        },

        async start(vm: string): Promise<Outcome> {
            // An unknown VM is refused BEFORE the channel is opened: the host
            // socket must never be called for a VM this service does not
            // inventory.
            const known = (await statique.lister()).some((v) => v.id === vm);
            if (!known) return refuser('vm-inconnue', 'demarrer', BACKEND_HOTE);
            return wake.request(vm);
        },
    };
}
