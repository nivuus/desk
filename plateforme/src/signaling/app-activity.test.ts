import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BUSY_PERIOD_MS, createAppActivity } from './app-activity';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
});

function mount() {
    const send = vi.fn(async (_v: 'busy') => ({ ok: true }) as const);
    return { send, activity: createAppActivity(send) };
}

describe('createAppActivity', () => {
    it('a window opens: `busy` is sent right away, then every BUSY_PERIOD_MS', () => {
        const { send, activity } = mount();
        activity.apparie('PFX:w-1');
        expect(send).toHaveBeenCalledTimes(1);
        expect(send).toHaveBeenCalledWith('busy');
        vi.advanceTimersByTime(BUSY_PERIOD_MS);
        expect(send).toHaveBeenCalledTimes(2);
        vi.advanceTimersByTime(BUSY_PERIOD_MS);
        expect(send).toHaveBeenCalledTimes(3);
        activity.stop();
    });

    it('🔴 the `bureau` control session and the `fichiers` bridge do NOT count', () => {
        const { send, activity } = mount();
        activity.apparie('PFX:bureau');
        activity.apparie('PFX:fichiers');
        vi.advanceTimersByTime(BUSY_PERIOD_MS * 3);
        expect(send).not.toHaveBeenCalled();
        expect(activity.windows()).toBe(0);
    });

    it('🔴 names that look like `w-1` without being one do not count', () => {
        const { send, activity } = mount();
        for (const name of ['PFX:w-', 'PFX:w-1x', 'PFX:xw-1', 'PFX:w-1:z']) activity.apparie(name);
        expect(send).not.toHaveBeenCalled();
        expect(activity.windows()).toBe(0);
    });

    it('an unprefixed `w-1` counts (bare session)', () => {
        const { send, activity } = mount();
        activity.apparie('w-1');
        expect(send).toHaveBeenCalledTimes(1);
        activity.stop();
    });

    it('the last window closed stops the heartbeats', () => {
        const { send, activity } = mount();
        activity.apparie('PFX:w-1');
        activity.apparie('PFX:w-2');
        activity.separe('PFX:w-1');
        vi.advanceTimersByTime(BUSY_PERIOD_MS);
        expect(send).toHaveBeenCalledTimes(2); // one window left: still beating
        activity.separe('PFX:w-2');
        vi.advanceTimersByTime(BUSY_PERIOD_MS * 3);
        expect(send).toHaveBeenCalledTimes(2); // nothing more
        expect(activity.windows()).toBe(0);
    });

    it('🔴 a repeated `apparie` and an unknown `separe` do not skew the count', () => {
        const { send, activity } = mount();
        activity.apparie('PFX:w-1');
        activity.apparie('PFX:w-1');
        activity.separe('PFX:w-9'); // never paired
        expect(activity.windows()).toBe(1);
        activity.separe('PFX:w-1');
        expect(activity.windows()).toBe(0);
        vi.advanceTimersByTime(BUSY_PERIOD_MS * 3);
        expect(send).toHaveBeenCalledTimes(1);
    });

    it('🔴 a send failure is logged as a warning and the heartbeats continue', async () => {
        const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const send = vi.fn(async (_v: 'busy') => ({ ok: false, cause: 'ENOENT' }) as const);
        const activity = createAppActivity(send);
        activity.apparie('PFX:w-1');
        await vi.advanceTimersByTimeAsync(BUSY_PERIOD_MS);
        expect(send).toHaveBeenCalledTimes(2);
        expect(warn).toHaveBeenCalledWith(expect.stringContaining('ENOENT'));
        activity.stop();
    });

    it('`stop` stops everything, even with windows open', () => {
        const { send, activity } = mount();
        activity.apparie('PFX:w-1');
        activity.stop();
        vi.advanceTimersByTime(BUSY_PERIOD_MS * 3);
        expect(send).toHaveBeenCalledTimes(1);
    });

    it('🔴 `stop` is terminal: a pairing after it neither counts nor re-arms the timer', () => {
        const { send, activity } = mount();
        activity.apparie('PFX:w-1');
        expect(send).toHaveBeenCalledTimes(1);
        activity.stop();
        activity.apparie('PFX:w-2');
        vi.advanceTimersByTime(BUSY_PERIOD_MS * 3);
        expect(send).toHaveBeenCalledTimes(1);
        expect(activity.windows()).toBe(1);
    });
});
