// CORS, et le refus par défaut.
//
// 🔴 La valeur `*` n'est JAMAIS produite, et ce n'est pas une intention : c'est
// une assertion, balayée sur toutes les issues de tous les cas de ce fichier.

import { describe, expect, it } from 'vitest';
import { entetesCors } from './cors';

const AUTORISEE = 'http://127.0.0.1:5173';

describe('entetesCors', () => {
    it('n’émet AUCUN en-tête quand aucune origine n’est autorisée', () => {
        // Le défaut est le refus, jamais l'ouverture : sans
        // `PLATEFORME_ORIGINE_CLIENT`, le navigateur refuse de lire la réponse,
        // ce que l'opérateur voit immédiatement.
        expect(entetesCors(AUTORISEE, undefined)).toBeUndefined();
        expect(entetesCors(undefined, undefined)).toBeUndefined();
    });

    it('n’émet rien à qui ne demande pas — une requête non navigateur', () => {
        // Renvoyer l'origine autorisée à un appelant qui n'a pas d'`Origin`
        // n'a aucun sens et divulgue la configuration.
        expect(entetesCors(undefined, AUTORISEE)).toBeUndefined();
    });

    it('REFUSE une origine différente, sans préfixe ni inclusion', () => {
        // Une comparaison par `startsWith` accepterait
        // `http://127.0.0.1:5173.attaquant.test`.
        expect(entetesCors('http://mechant.test', AUTORISEE)).toBeUndefined();
        expect(entetesCors('http://127.0.0.1:5173.mechant.test', AUTORISEE)).toBeUndefined();
        expect(entetesCors('http://127.0.0.1:517', AUTORISEE)).toBeUndefined();
    });

    it('émet l’origine ET Vary: Origin quand elle correspond exactement', () => {
        const entetes = entetesCors(AUTORISEE, AUTORISEE);
        expect(entetes).toBeDefined();
        expect(entetes!['Access-Control-Allow-Origin']).toBe(AUTORISEE);
        // 🔴 Sans `Vary`, un cache intermédiaire servirait la réponse d'une
        // origine à une autre.
        expect(entetes!['Vary']).toBe('Origin');
    });

    it('ne produit JAMAIS la valeur `*`, quelle que soit l’entrée', () => {
        const entrees: Array<[string | undefined, string | undefined]> = [
            ['*', '*'],
            ['*', AUTORISEE],
            [AUTORISEE, '*'],
            [AUTORISEE, AUTORISEE],
            [undefined, '*'],
            ['null', 'null'],
        ];
        for (const [demandee, autorisee] of entrees) {
            const entetes = entetesCors(demandee, autorisee);
            if (entetes) expect(entetes['Access-Control-Allow-Origin']).not.toBe('*');
        }
    });
});
