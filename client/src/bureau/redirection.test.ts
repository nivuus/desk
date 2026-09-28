import { describe, expect, it } from 'vitest';
import { cibleDeRedirection } from './redirection';

describe('cibleDeRedirection', () => {
    it('keeps the query string', () => {
        // 🔴 THE POINT OF THE WHOLE TASK: ALREADY installed PWAs carry
        // `shell.html?app=<id>` in their id (frozen, never updated -- see
        // hub/manifeste.ts), not in start_url since this batch, and the
        // blob: manifest they carry will never be read again. Losing
        // `?app=` would break them as surely as deleting the file.
        expect(cibleDeRedirection('?app=u-1')).toBe('/?app=u-1');
    });

    it('without a query, leads to the bare root', () => {
        expect(cibleDeRedirection('')).toBe('/');
    });

    it('keeps SEVERAL parameters', () => {
        expect(cibleDeRedirection('?app=u-1&plateforme=https%3A%2F%2Fx')).toBe(
            '/?app=u-1&plateforme=https%3A%2F%2Fx',
        );
    });
});
