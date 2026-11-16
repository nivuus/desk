import { describe, expect, it } from 'vitest';
import type { Ton } from '../shell';
import { RETRY_MAX_MS, RETRY_MIN_MS, createSeatRetry, retryDelayMs } from './seat-retry';

/// A harness that records what the person is shown and holds the scheduled
/// retries, so the test runs them when it decides — no timer, no DOM.
function harness(token: () => Promise<string | undefined>) {
    const shown: { text: string; tone: Ton }[] = [];
    const scheduled: { run: () => void; delayMs: number }[] = [];
    const opened: string[] = [];
    let noToken = 0;
    const retry = createSeatRetry({
        show: (text, tone) => void shown.push({ text, tone }),
        schedule: (run, delayMs) => void scheduled.push({ run, delayMs }),
        freshToken: token,
        open: (jeton) => void opened.push(jeton),
        noToken: () => void noToken++,
    });
    return { retry, shown, scheduled, opened, noToken: () => noToken };
}

/// Lets the promise chain of a scheduled retry settle.
const settle = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

describe('retryDelayMs', () => {
    it('starts at the floor, grows, and reaches the ceiling without passing it', () => {
        expect(retryDelayMs(0)).toBe(RETRY_MIN_MS);
        let previous = retryDelayMs(0);
        for (let attempt = 1; attempt < 40; attempt++) {
            const delay = retryDelayMs(attempt);
            expect(delay).toBeGreaterThanOrEqual(previous);
            expect(delay).toBeLessThanOrEqual(RETRY_MAX_MS);
            previous = delay;
        }
        expect(retryDelayMs(1)).toBeGreaterThan(RETRY_MIN_MS);
        expect(previous).toBe(RETRY_MAX_MS);
    });
});

describe('a refused seat', () => {
    it('is TOLD to the person, with the delay before the next attempt', () => {
        const h = harness(async () => 'fresh');
        h.retry.refused();

        expect(h.shown).toHaveLength(1);
        expect(h.shown[0]!.tone).toBe('alerte');
        expect(h.shown[0]!.text).toContain(`${RETRY_MIN_MS / 1000} s`);
        // Nothing is opened before the delay has run.
        expect(h.opened).toEqual([]);
        expect(h.scheduled.map((s) => s.delayMs)).toEqual([RETRY_MIN_MS]);
    });

    it('is retried with a FRESH token once the delay has run', async () => {
        let issued = 0;
        const h = harness(async () => `token-${++issued}`);
        h.retry.refused();
        h.scheduled[0]!.run();
        await settle();
        expect(h.opened).toEqual(['token-1']);

        // Refused again: the next attempt asks for a token again.
        h.retry.refused();
        h.scheduled[1]!.run();
        await settle();
        expect(h.opened).toEqual(['token-1', 'token-2']);
    });

    it('backs off from one refusal to the next', () => {
        const h = harness(async () => 'fresh');
        for (let i = 0; i < 8; i++) h.retry.refused();

        const delays = h.scheduled.map((s) => s.delayMs);
        expect(delays[0]).toBe(RETRY_MIN_MS);
        expect(delays[1]).toBeGreaterThan(delays[0]!);
        expect(delays[7]).toBe(RETRY_MAX_MS);
        // Each refusal says ITS delay, never the first one's.
        expect(h.shown[7]!.text).toContain(`${RETRY_MAX_MS / 1000} s`);
    });

    it('without a token, says so and opens nothing', async () => {
        const h = harness(async () => undefined);
        h.retry.refused();
        h.scheduled[0]!.run();
        await settle();

        expect(h.opened).toEqual([]);
        expect(h.noToken()).toBe(1);
    });

    it('a token request that fails is not a silent end either', async () => {
        const h = harness(async () => {
            throw new Error('network down');
        });
        h.retry.refused();
        h.scheduled[0]!.run();
        await settle();

        expect(h.opened).toEqual([]);
        expect(h.noToken()).toBe(1);
    });
});
