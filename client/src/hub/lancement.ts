// Launching an application, waiting for the VM if it is starting.
//
// 🔴 THE CLIENT READS `etat`, NEVER A MESSAGE: the `demarrage` value is
// technical, hence independent of the language.
//
// 🔴 ONLY `503` + `etat: demarrage` IS REPLAYED. In that case `registre.lancer`
// sent NO order (no agent socket): replaying cannot duplicate anything.
// `504 delai` — the order WENT OUT and nobody answered — is never replayed:
// the application may already be running.

import { lancerApplication, type DepsCatalogue, type Issue } from './catalogue';

/// Interval between two attempts while the VM starts. ⚠️ NOT CALIBRATED.
export const LAUNCH_RETRY_MS = 3_000;

/// Maximum wait for a starting VM. ⚠️ TO KEEP EQUAL to `MAX_WAIT_SECONDS` of
/// `handle-vm-start.sh` (180 s, `installer` repository) and to
/// `WAKE_PENDING_MAX_MS` of `plateforme/src/orchestration/wake.ts`.
export const LAUNCH_STARTUP_MAX_MS = 180_000;

/// The technical value the platform puts in `etat` during a wake.
const ETAT_DEMARRAGE = 'demarrage';

export interface LaunchClock {
    now(): number;
    sleep(ms: number): Promise<void>;
}

export type LaunchResult = { kind: 'done'; issue: Issue<null> } | { kind: 'vm-timeout' };

function enDemarrage(issue: Issue<null>): boolean {
    return (
        issue.etat === 'refus' &&
        issue.refus.source === 'service' &&
        issue.refus.statut === 503 &&
        issue.refus.etat === ETAT_DEMARRAGE
    );
}

export async function launchWhenReady(
    id: string,
    deps: DepsCatalogue,
    clock: LaunchClock,
    onStarting: () => void,
): Promise<LaunchResult> {
    const debut = clock.now();
    let annonce = false;
    for (;;) {
        const issue = await lancerApplication(id, deps);
        if (!enDemarrage(issue)) return { kind: 'done', issue };
        if (!annonce) {
            annonce = true;
            onStarting();
        }
        if (clock.now() - debut >= LAUNCH_STARTUP_MAX_MS) return { kind: 'vm-timeout' };
        await clock.sleep(LAUNCH_RETRY_MS);
    }
}
