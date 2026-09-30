// What the platform tells the host: "an app window is open".
//
// 🔴 ONLY `w-<N>` SESSIONS COUNT. The `bureau` control session and the
// `fichiers` bridge are paired as soon as the hub is open on a running VM:
// counting them would let a forgotten hub tab block the VM shutdown forever.
//
// 🔴 A SET OF NAMES, NOT A COUNTER: a repeated `apparie` or an unknown
// `separe` can neither skew the count nor make it negative.

import { decouper } from '../agents/prefixe';
import type { ReponseHote } from '../orchestration/host-channel';
import type { ObservateurDeSession } from './relais';

/// Period between `busy` signals. ⚠️ MUST STAY EQUAL to `APP_HEARTBEAT_S` in
/// `console/host/vm-idle-shutdown.sh` (`installer` repository): the host
/// considers a window open as long as the last `busy` is less than three
/// periods old.
export const BUSY_PERIOD_MS = 60_000;

const WINDOW_NAME = /^w-\d+$/;

export interface AppActivity extends ObservateurDeSession {
    /// Stops the heartbeats (service shutdown).
    stop(): void;
    /// The number of open windows, for tests and diagnostics.
    windows(): number;
}

export function createAppActivity(send: (verb: 'busy') => Promise<ReponseHote>): AppActivity {
    const open = new Set<string>();
    let timer: NodeJS.Timeout | undefined;

    const beat = (): void => {
        void send('busy').then((response) => {
            if (!response.ok) console.warn(`busy signal failed: ${response.cause}`);
        });
    };
    const arm = (): void => {
        if (timer !== undefined) return;
        beat();
        timer = setInterval(beat, BUSY_PERIOD_MS);
        // A heartbeat timer must never keep the process alive.
        timer.unref();
    };
    const disarm = (): void => {
        if (timer === undefined) return;
        clearInterval(timer);
        timer = undefined;
    };

    return {
        apparie(sessionName) {
            if (!WINDOW_NAME.test(decouper(sessionName).nom)) return;
            open.add(sessionName);
            arm();
        },
        separe(sessionName) {
            open.delete(sessionName);
            if (open.size === 0) disarm();
        },
        stop: disarm,
        windows: () => open.size,
    };
}
