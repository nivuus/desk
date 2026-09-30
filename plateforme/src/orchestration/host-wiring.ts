// The host wiring: one place decides whether the service knows how to wake the
// VM. `serveur.ts` only receives the result.

import type { Pilote } from '../base/pilote';
import { envoyerAuCanal } from './host-channel';
import { hostOrchestrator } from './host-orchestrator';
import type { Orchestrateur } from './interface';
import { inventaireStatique } from './inventaire-statique';
import { Wake } from './wake';

export interface CablageHote {
    orchestrateur: Orchestrateur;
}

/// An absent `socketPath` means the channel is disabled: we keep the static
/// inventory, whose `start` refuses `non-supporte` (the behaviour from before
/// this work).
export function hostWiring(
    base: Pilote,
    now: () => number,
    socketPath: string | undefined,
): CablageHote {
    if (socketPath === undefined) {
        return { orchestrateur: inventaireStatique(base, now) };
    }
    const wake = new Wake((verb) => envoyerAuCanal(socketPath, verb), now);
    return { orchestrateur: hostOrchestrator(base, now, wake) };
}
