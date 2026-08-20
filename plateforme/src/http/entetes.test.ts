import { describe, expect, it } from 'vitest';
import { ENTETES_SECURITE } from './entetes';

describe('ENTETES_SECURITE', () => {
    it('(a) porte les deux en-têtes, avec leurs valeurs exactes', () => {
        expect(ENTETES_SECURITE['X-Content-Type-Options']).toBe('nosniff');
        expect(ENTETES_SECURITE['Cache-Control']).toBe('no-store');
    });

    it("(b) 🔴 ne porte AUCUNE valeur joker — même règle que `cors.ts`", () => {
        // Éprouvée ici pour qu'un ajout futur ne l'introduise pas : un `*` sur
        // un en-tête de sécurité est presque toujours une désactivation
        // déguisée en configuration.
        for (const [cle, valeur] of Object.entries(ENTETES_SECURITE)) {
            expect(valeur, `l'en-tête ${cle} porte un joker`).not.toContain('*');
        }
    });

    it("(c) est INCONDITIONNEL : c'est un objet, jamais `undefined`", () => {
        // La différence avec `entetesCors`, qui rend `undefined` quand
        // l'origine n'est pas autorisée. Fusionner les deux modules ferait
        // dépendre la sécurité d'une configuration CORS FACULTATIVE.
        expect(ENTETES_SECURITE).toBeTypeOf('object');
        expect(Object.keys(ENTETES_SECURITE).length).toBeGreaterThan(0);
    });
});
