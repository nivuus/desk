import { describe, expect, it, vi } from 'vitest';
import { composeObservers } from './observers';

describe('composeObservers', () => {
    it('forwards apparie and separe to every observer, in order', () => {
        const calls: string[] = [];
        const a = { apparie: (n: string) => calls.push(`a+${n}`), separe: (n: string) => calls.push(`a-${n}`) };
        const b = { apparie: (n: string) => calls.push(`b+${n}`), separe: (n: string) => calls.push(`b-${n}`) };
        const c = composeObservers(a, b);
        c.apparie('s1');
        c.separe('s1');
        expect(calls).toEqual(['a+s1', 'b+s1', 'a-s1', 'b-s1']);
    });

    it('🔴 an observer that throws does not stop the next ones, and the error is logged', () => {
        const error = vi.spyOn(console, 'error').mockImplementation(() => {});
        const next = vi.fn();
        const c = composeObservers(
            { apparie: () => { throw new Error('boom'); }, separe: () => {} },
            { apparie: next, separe: () => {} },
        );
        expect(() => c.apparie('s1')).not.toThrow();
        expect(next).toHaveBeenCalledWith('s1');
        expect(error).toHaveBeenCalledWith(expect.stringContaining('boom'));
        error.mockRestore();
    });
});
