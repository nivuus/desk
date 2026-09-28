import { describe, expect, it } from 'vitest';
import { ENTETES_SECURITE } from './entetes';

describe('ENTETES_SECURITE', () => {
    it('(a) carries both headers, with their exact values', () => {
        expect(ENTETES_SECURITE['X-Content-Type-Options']).toBe('nosniff');
        expect(ENTETES_SECURITE['Cache-Control']).toBe('no-store');
    });

    it("(b) 🔴 carries NO wildcard value — same rule as `cors.ts`", () => {
        // Tested here so that a future addition does not introduce it: a `*` on
        // a security header is almost always a deactivation
        // disguised as configuration.
        for (const [cle, value] of Object.entries(ENTETES_SECURITE)) {
            expect(value, `the header ${cle} carries a wildcard`).not.toContain('*');
        }
    });

    it("(c) is UNCONDITIONAL: it is an object, never `undefined`", () => {
        // The difference with `entetesCors`, which returns `undefined` when
        // the origin is not allowed. Merging the two modules would make
        // security depend on an OPTIONAL CORS configuration.
        expect(ENTETES_SECURITE).toBeTypeOf('object');
        expect(Object.keys(ENTETES_SECURITE).length).toBeGreaterThan(0);
    });
});
