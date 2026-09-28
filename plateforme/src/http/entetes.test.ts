import { describe, expect, it } from 'vitest';
import { ENTETES_SECURITE } from './entetes';

describe('ENTETES_SECURITE', () => {
    it('(a) carries both headers, with their exact values', () => {
        expect(ENTETES_SECURITE['X-Content-Type-Options']).toBe('nosniff');
        expect(ENTETES_SECURITE['Cache-Control']).toBe('no-store');
    });

    it("(b) 🔴 carries NO wildcard value — same rule as `cors.ts`", () => {
        // Éprouvée ici pour qu'un ajout futur ne l'introduise pas : un `*` sur
        // un en-tête de sécurité est presque toujours une désactivation
        // déguisée en configuration.
        for (const [cle, value] of Object.entries(ENTETES_SECURITE)) {
            expect(value, `the header ${cle} carries a wildcard`).not.toContain('*');
        }
    });

    it("(c) is UNCONDITIONAL: it is an object, never `undefined`", () => {
        // La différence avec `entetesCors`, qui rend `undefined` quand
        // l'origine n'est pas autorisée. Fusionner les deux modules ferait
        // dépendre la sécurité d'une configuration CORS FACULTATIVE.
        expect(ENTETES_SECURITE).toBeTypeOf('object');
        expect(Object.keys(ENTETES_SECURITE).length).toBeGreaterThan(0);
    });
});
