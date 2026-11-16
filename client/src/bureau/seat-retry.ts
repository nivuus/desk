// WHAT A TAB DOES WHEN THE PLATFORM REFUSES IT THE DESKTOP'S SEAT — the rule,
// pure and tested. The wiring (socket, timer, DOM) lives in `porteur-dom.ts`.
//
// 🔴 THE REFUSAL WAS SWALLOWED, AND THE TAB GAVE UP FOR GOOD. The `client`
// role is exclusive per session; a tab refused with `role-occupe` became a
// follower in silence ("Desktop held by another tab.") and never tried again.
// That is right for the loser of an election inside ONE browser — but the tab
// that reaches this refusal is the one its browser ELECTED, and nothing in
// this browser holds the seat. What holds it lives elsewhere:
//   - another browser, another profile, another device open on the same VM;
//   - or THIS tab's previous connection, dead on its side and not yet
//     released by the platform — a reload after the laptop slept or the
//     network changed. The platform reaps such a connection within its
//     liveness deadline (`plateforme/src/http/liveness.ts`).
// In both cases no tab of this browser carried the session, the window list
// stayed empty, and no message said why.
//
// So the refusal is a STATE: it is told, with what happens next, and the seat
// is asked for again after a delay that grows — until it is granted.

import type { Ton } from '../shell';

/// The first delay. Long enough not to hammer the relay's request budget,
/// short enough that a seat freed at once is taken at once.
export const RETRY_MIN_MS = 2_000;
/// The ceiling: the order of the platform's liveness deadline, so a seat held
/// by a dead connection is retaken within about one more attempt.
export const RETRY_MAX_MS = 30_000;

/// The delay before attempt number `attempt` (0 = the first retry): doubles
/// from the floor, capped at the ceiling.
export function retryDelayMs(attempt: number): number {
    // The exponent is bounded BEFORE the power: 2 ** 1024 is `Infinity`, and
    // an `Infinity` delay would be clamped by `Math.min` — but only by luck.
    const doublings = Math.min(Math.max(attempt, 0), 16);
    return Math.min(RETRY_MIN_MS * 2 ** doublings, RETRY_MAX_MS);
}

/// What the person reads while the seat is taken.
export function seatTakenText(delayMs: number): string {
    return (
        'The desktop is held by another connection: another browser or device, ' +
        `or a connection that has just dropped. Trying again in ${Math.round(delayMs / 1000)} s.`
    );
}

export interface SeatRetryDeps {
    /// Tells the person. Called at EACH refusal, with that refusal's delay.
    show(text: string, tone: Ton): void;
    /// Runs `run` once, after `delayMs`.
    schedule(run: () => void, delayMs: number): void;
    /// Requests a FRESH token: a retry can happen long after the page loaded,
    /// and an access token lives ten minutes (`porteur.ts::DepsPromotion`).
    freshToken(): Promise<string | undefined>;
    /// Opens the control session's socket again, with that token.
    open(token: string): void;
    /// No token obtainable: say so in an actionable way instead of opening a
    /// socket doomed to another refusal.
    noToken(): void;
}

export interface SeatRetry {
    /// The platform has just refused the seat (`porteur.ts::estPlacePrise`).
    refused(): void;
}

export function createSeatRetry(deps: SeatRetryDeps): SeatRetry {
    /// Consecutive refusals. Never reset: the only path that opens a socket
    /// again after the first is this one, so the chain ends when the seat is
    /// granted and nothing counts after that.
    let attempt = 0;
    return {
        refused() {
            const delayMs = retryDelayMs(attempt);
            attempt += 1;
            deps.show(seatTakenText(delayMs), 'alerte');
            deps.schedule(() => {
                deps.freshToken().then(
                    (token) => {
                        if (token === undefined) deps.noToken();
                        else deps.open(token);
                    },
                    (cause: unknown) => {
                        // Not swallowed: logged with its cause, and the person
                        // is told what to do — the same remedy as an expired
                        // session, since no token can be had.
                        console.error('desktop seat retry: no token obtained', cause);
                        deps.noToken();
                    },
                );
            }, delayMs);
        },
    };
}
