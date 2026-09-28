// The session ownership registry, pure and synchronous.

import { describe, expect, it } from 'vitest';
import { ProprieteDeSession } from './propriete';

describe('ProprieteDeSession', () => {
    it('a free session has no owner', () => {
        expect(new ProprieteDeSession().proprietaire('s-1')).toBeUndefined();
    });

    it('claimed by u1, it returns u1 — and not u2', () => {
        // 🔴 The second assertion is the point: a registry that returned
        // `undefined` whatever happened would pass the first, and nobody
        // would ever be refused again.
        const r = new ProprieteDeSession();
        r.revendiquer('s-1', 'u1');
        expect(r.proprietaire('s-1')).toBe('u1');
        expect(r.proprietaire('s-1')).not.toBe('u2');
    });

    it('claiming twice by the SAME user has no effect, never an error', () => {
        // A reconnection of the same user would otherwise break their own
        // session.
        const r = new ProprieteDeSession();
        r.revendiquer('s-1', 'u1');
        expect(() => r.revendiquer('s-1', 'u1')).not.toThrow();
        expect(r.proprietaire('s-1')).toBe('u1');
    });

    it('releasing makes the session free again, and claimable by another', () => {
        // Never releasing would lose a session name for life.
        const r = new ProprieteDeSession();
        r.revendiquer('s-1', 'u1');
        r.liberer('s-1');
        expect(r.proprietaire('s-1')).toBeUndefined();
        r.revendiquer('s-1', 'u2');
        expect(r.proprietaire('s-1')).toBe('u2');
    });

    it('two sessions do not mix', () => {
        const r = new ProprieteDeSession();
        r.revendiquer('s-1', 'u1');
        r.revendiquer('s-2', 'u2');
        expect(r.proprietaire('s-1')).toBe('u1');
        expect(r.proprietaire('s-2')).toBe('u2');
        r.liberer('s-1');
        expect(r.proprietaire('s-2')).toBe('u2');
    });
});
