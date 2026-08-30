import { describe, expect, it } from 'vitest';
import { sessionIdDepuisParametres } from './session-id';

describe('sessionIdDepuisParametres', () => {
    it("rend l'identifiant porté par le paramètre `session`", () => {
        expect(sessionIdDepuisParametres(new URLSearchParams('session=abc'))).toBe('abc');
    });

    // 🔴 LA ROUGE DE LA CORRECTION DU 30 AOÛT 2026. Avant l'extraction,
    // `main.ts` portait `params.get('session') ?? 'demo'` : ce test rougit
    // si cette forme revient un jour, sous quelque nom que ce soit — une
    // absence de paramètre doit rendre `undefined`, jamais une valeur
    // inventée. Vérifié rouge en substituant temporairement le corps de
    // `sessionIdDepuisParametres` par l'ancienne ligne (voir le rapport de
    // ce lot pour la sortie réelle) : `expect(undefined).toBe('demo')`
    // échoue comme attendu.
    it("n'invente PAS de session 'demo' quand le paramètre est absent", () => {
        expect(sessionIdDepuisParametres(new URLSearchParams())).toBeUndefined();
    });
});
