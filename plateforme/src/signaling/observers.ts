// The relay accepts only ONE session observer: this composer chains several
// without any of them depending on the others.
//
// 🔴 `apparie` and `separe` are called from the synchronous `message` handler
// of a `ws` socket, where an exception takes the whole Node process down (see
// `relais.ts::ObservateurDeSession`). An observer that throws is therefore
// logged, never propagated, and does not deprive the next ones of the event.

import type { ObservateurDeSession } from './relais';

export function composeObservers(...observers: ObservateurDeSession[]): ObservateurDeSession {
    // The arguments are forwarded exactly as received: an absent `userId` stays
    // absent instead of becoming an explicit `undefined`.
    const dispatch = <M extends 'apparie' | 'separe'>(
        method: M,
        ...args: Parameters<ObservateurDeSession[M]>
    ): void => {
        for (const observer of observers) {
            try {
                (observer[method] as (...a: unknown[]) => void)(...args);
            } catch (cause) {
                console.error(`session observer ${method} failed for ${args[0]}: ${String(cause)}`);
            }
        }
    };
    return {
        apparie: (...args) => dispatch('apparie', ...args),
        separe: (...args) => dispatch('separe', ...args),
    };
}
