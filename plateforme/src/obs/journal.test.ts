import { describe, expect, it } from 'vitest';
import { ligne } from './journal';

describe('ligne', () => {
    it("(a) an event without a field is the event alone", () => {
        expect(ligne('frein', {})).toBe('frein');
    });

    it('(a bis) the fields follow `evenement k=v k=v`', () => {
        expect(ligne('frein', { adresse: '203.0.113.7', budget: 5 })).toBe(
            'frein adresse=203.0.113.7 budget=5',
        );
    });

    it('(b) a value carrying a space is QUOTED', () => {
        expect(ligne('sante', { motif: 'database unreachable' })).toBe('sante motif="database unreachable"');
    });

    it("(b bis) a value carrying a `=` is quoted too", () => {
        // Otherwise `k=a=b` would be unreadable: one would not know where the
        // value ends and where the next field starts.
        expect(ligne('frein', { cle: 'compte=alice' })).toBe('frein cle="compte=alice"');
    });

    it('(b ter) inner quotes are escaped', () => {
        expect(ligne('frein', { cle: 'a "thing" quoted' })).toBe('frein cle="a \\"thing\\" quoted"');
    });

    it("(c) 🔴 no value is TRUNCATED", () => {
        // A brake that truncated an address would make it ambiguous:
        // `203.0.113.7` and `203.0.113.70` would read the same, and the operator
        // could no longer recognise their proxy's address — which is the
        // ONLY remedy for the failure mode of `adresse-source.ts`.
        const longue = 'a'.repeat(500);
        expect(ligne('frein', { cle: longue })).toBe(`frein cle=${longue}`);
        expect(ligne('frein', { cle: longue })).toContain(longue);
    });

    it("(d) the order of the fields follows that of the object", () => {
        // So that two lines of the same event can be compared by eye, and
        // sorted. An object's `JSON.stringify` does not guarantee it any more,
        // but an alphabetical sort would break it.
        expect(ligne('e', { z: 1, a: 2 })).toBe('e z=1 a=2');
        expect(ligne('e', { a: 2, z: 1 })).toBe('e a=2 z=1');
    });

    it("(e) an EMPTY value stays visible, rather than disappearing", () => {
        // A field that disappeared would make one believe the service did not
        // measure it, where it measured it empty.
        expect(ligne('frein', { adresse: '' })).toBe('frein adresse=""');
    });
});
