import { describe, expect, it, vi } from 'vitest';
import { Wake, WAKE_PENDING_MAX_MS } from './wake';

function factory(reply: { ok: true } | { ok: false; cause: string } = { ok: true }) {
    let instant = 1_000_000;
    const send = vi.fn(async (_verb: 'wake') => reply);
    const wake = new Wake(send, () => instant);
    return { wake, send, advance: (ms: number) => void (instant += ms) };
}

describe('Wake', () => {
    it('does not claim a wake is pending before one was requested', () => {
        expect(factory().wake.pending('v1')).toBe(false);
    });

    it('requests the wake once, then marks it pending', async () => {
        const { wake, send } = factory();
        expect(await wake.request('v1')).toEqual({ ok: true });
        expect(send).toHaveBeenCalledTimes(1);
        expect(send).toHaveBeenCalledWith('wake');
        expect(wake.pending('v1')).toBe(true);
    });

    it('🔴 two simultaneous launches on a stopped VM send a single `wake`', async () => {
        const { wake, send } = factory();
        await Promise.all([wake.request('v1'), wake.request('v1')]);
        await wake.request('v1');
        expect(send).toHaveBeenCalledTimes(1);
    });

    it('one wake per VM: another VM has its own', async () => {
        const { wake, send } = factory();
        await wake.request('v1');
        await wake.request('v2');
        expect(send).toHaveBeenCalledTimes(2);
    });

    it('🔴 the bound: pending up to WAKE_PENDING_MAX_MS, not after, and a new wake is then requested', async () => {
        const { wake, send, advance } = factory();
        await wake.request('v1');
        advance(WAKE_PENDING_MAX_MS);
        expect(wake.pending('v1')).toBe(true);
        advance(1);
        expect(wake.pending('v1')).toBe(false);
        await wake.request('v1');
        expect(send).toHaveBeenCalledTimes(2);
    });

    it('🔴 a channel failure is a typed, logged refusal and marks nothing pending', async () => {
        const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const { wake } = factory({ ok: false, cause: 'ENOENT' });
        expect(await wake.request('v1')).toEqual({
            ok: false,
            motif: 'hote-inaccessible',
            operation: 'demarrer',
            backend: 'hote',
        });
        expect(wake.pending('v1')).toBe(false);
        expect(warn).toHaveBeenCalledWith(expect.stringContaining('ENOENT'));
        warn.mockRestore();
    });
});
