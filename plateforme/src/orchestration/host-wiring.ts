// The host wiring: one place decides whether the service knows how to wake the
// VM. `serveur.ts` only receives the result.

import type { Pilote } from '../base/pilote';
import { createAppActivity, type AppActivity } from '../signaling/app-activity';
import { envoyerAuCanal } from './host-channel';
import { hostOrchestrator } from './host-orchestrator';
import type { Orchestrateur } from './interface';
import { inventaireStatique } from './inventaire-statique';
import { Wake } from './wake';

export interface CablageHote {
    orchestrateur: Orchestrateur;
    /// `undefined` when the channel is disabled: nothing to report to the host.
    activity: AppActivity | undefined;
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
        return { orchestrateur: inventaireStatique(base, now), activity: undefined };
    }
    const wake = new Wake((verb) => envoyerAuCanal(socketPath, verb), now);
    return {
        orchestrateur: hostOrchestrator(base, now, wake),
        activity: createAppActivity((verb) => envoyerAuCanal(socketPath, verb)),
    };
}
