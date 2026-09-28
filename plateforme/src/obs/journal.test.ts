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
        // Sans quoi `k=a=b` serait illisible : on ne saurait pas où finit la
        // valeur et où commence le champ suivant.
        expect(ligne('frein', { cle: 'compte=alice' })).toBe('frein cle="compte=alice"');
    });

    it('(b ter) inner quotes are escaped', () => {
        expect(ligne('frein', { cle: 'a "thing" quoted' })).toBe('frein cle="a \\"thing\\" quoted"');
    });

    it("(c) 🔴 no value is TRUNCATED", () => {
        // Un frein qui tronquerait une adresse la rendrait ambiguë :
        // `203.0.113.7` et `203.0.113.70` se liraient pareil, et l'exploitant
        // ne pourrait plus reconnaître l'adresse de son proxy — qui est le
        // SEUL remède au mode de défaillance de `adresse-source.ts`.
        const longue = 'a'.repeat(500);
        expect(ligne('frein', { cle: longue })).toBe(`frein cle=${longue}`);
        expect(ligne('frein', { cle: longue })).toContain(longue);
    });

    it("(d) the order of the fields follows that of the object", () => {
        // Pour que deux lignes du même évènement se comparent à l'œil, et se
        // trient. Un `JSON.stringify` d'objet ne le garantit pas davantage,
        // mais un tri alphabétique le romprait.
        expect(ligne('e', { z: 1, a: 2 })).toBe('e z=1 a=2');
        expect(ligne('e', { a: 2, z: 1 })).toBe('e a=2 z=1');
    });

    it("(e) an EMPTY value stays visible, rather than disappearing", () => {
        // Un champ qui disparaîtrait ferait croire que le service ne l'a pas
        // mesuré, là où il l'a mesuré vide.
        expect(ligne('frein', { adresse: '' })).toBe('frein adresse=""');
    });
});
