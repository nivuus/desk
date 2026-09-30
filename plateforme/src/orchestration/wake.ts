// The memory of requested wakes.
//
// 🔴 IT LIVES IN A LONG-LIVED OBJECT, not in the orchestrator: the routes built
// `inventaireStatique` on every request, and state held there would be lost on
// the next request.

import type { ReponseHote } from './host-channel';
import { BACKEND_HOTE, refuser, type Outcome } from './refus';

/// How long a requested wake counts as "pending".
/// ⚠️ NOT CALIBRATED: it is the `MAX_WAIT_SECONDS` of `handle-vm-start.sh`
/// (180 s for the IP), without the agent's bootstrap. Beyond it the state goes
/// back to `injoignable`: a VM that never starts does not stay "starting".
export const WAKE_PENDING_MAX_MS = 180_000;

export class Wake {
    private readonly requestedAt = new Map<string, number>();
    private readonly inFlight = new Map<string, Promise<Outcome>>();

    constructor(
        private readonly send: (verb: 'wake') => Promise<ReponseHote>,
        private readonly now: () => number,
    ) {}

    /// Was a wake requested less than `WAKE_PENDING_MAX_MS` ago?
    pending(vmId: string): boolean {
        const at = this.requestedAt.get(vmId);
        if (at === undefined) return false;
        if (this.now() - at > WAKE_PENDING_MAX_MS) {
            this.requestedAt.delete(vmId);
            return false;
        }
        return true;
    }

    /// Requests the wake, unless it is already pending (or in flight).
    /// ⚠️ `ok:true` says "the request went out or is already pending", never
    /// "the VM is ready": the agent says that, by reconnecting.
    request(vmId: string): Promise<Outcome> {
        if (this.pending(vmId)) return Promise.resolve({ ok: true });
        const existing = this.inFlight.get(vmId);
        if (existing !== undefined) return existing;

        const request = this.send('wake')
            .then((reply): Outcome => {
                if (!reply.ok) {
                    console.warn(`wake refused for vm ${vmId}: ${reply.cause}`);
                    return refuser('hote-inaccessible', 'demarrer', BACKEND_HOTE);
                }
                this.requestedAt.set(vmId, this.now());
                return { ok: true };
            })
            .finally(() => this.inFlight.delete(vmId));
        this.inFlight.set(vmId, request);
        return request;
    }
}
