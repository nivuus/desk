import { describe, expect, it } from 'vitest';
import { sessionIdFromParams } from './session-id';

describe('sessionIdDepuisParametres', () => {
    it("returns the identifier carried by the `session` parameter", () => {
        expect(sessionIdFromParams(new URLSearchParams('session=abc'))).toBe('abc');
    });

    // 🔴 LA ROUGE DE LA CORRECTION DU 30 AOÛT 2026. Avant l'extraction,
    // `main.ts` portait `params.get('session') ?? 'demo'` : ce test rougit
    // si cette forme revient un jour, sous quelque nom que ce soit — une
    // absence de paramètre doit rendre `undefined`, jamais une valeur
    // inventée. Vérifié rouge en substituant temporairement le corps de
    // `sessionIdFromParams` par l'ancienne ligne (voir le rapport de
    // ce lot pour la sortie réelle) : `expect(undefined).toBe('demo')`
    // échoue comme attendu.
    it("does NOT invent a 'demo' session when the parameter is absent", () => {
        expect(sessionIdFromParams(new URLSearchParams())).toBeUndefined();
    });
});
