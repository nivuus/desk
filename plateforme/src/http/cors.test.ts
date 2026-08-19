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

    it('🔴 annonce GET, POST, OPTIONS — `GET /vm` en a besoin', () => {
        // 🔴 La rouge : laisser `POST, OPTIONS`. La requête préalable de
        // `GET /vm` recevrait alors une liste de méthodes qui ne contient pas
        // la sienne, et le navigateur refuserait la vraie requête — SANS
        // qu'aucun test Node ne le voie, puisque les tests parlent en `fetch`
        // Node, qui n'applique pas la politique d'origine (voir l'en-tête de
        // `cors.ts`).
        //
        // ⚠️ Le sous-bloc G1 a besoin de la MÊME modification : elle est
        // identique et idempotente, et la seconde branche arrivée la trouvera
        // faite.
        const entetes = entetesCors(AUTORISEE, AUTORISEE);
        expect(entetes!['Access-Control-Allow-Methods']).toBe('GET, POST, OPTIONS');
    });

    it('🔴 autorise l’en-tête `authorization` — sans quoi AUCUNE route de P4 n’est atteignable', () => {
        // 🔴 DÉFAUT DU PLAN, RELEVÉ ET NON RECOPIÉ. La tâche 8 ne prescrivait
        // que `GET` dans `Access-Control-Allow-Methods`. Or les deux routes de
        // P4 exigent `Authorization: Bearer` (`http/porteur.ts`), et un en-tête
        // `Authorization` rend la requête NON SIMPLE : le navigateur envoie une
        // requête préalable portant `Access-Control-Request-Headers:
        // authorization`, à laquelle un serveur qui ne répond que
        // `content-type` oppose un refus. Les deux routes seraient donc
        // INATTEIGNABLES depuis le navigateur, et P4 livrerait une surface HTTP
        // que son propre client ne peut pas appeler.
        //
        // ⚠️ AUCUN TEST NODE NE POUVAIT LE VOIR — c'est exactement ce que
        // l'en-tête de `cors.ts` annonce de lui-même : « sans quoi le navigateur
        // refuse de lire la réponse, sans qu'aucun test côté serveur ne le
        // voie ». La garde est donc cette assertion, et rien d'autre.
        //
        // 🔴 La rouge : laisser `content-type` seul.
        const entetes = entetesCors(AUTORISEE, AUTORISEE);
        const permis = entetes!['Access-Control-Allow-Headers'].split(', ');
        expect(permis).toContain('authorization');
        // `content-type` reste : `POST /auth/connexion` en a besoin, et le
        // retirer casserait P2 sans qu'aucune ligne de P4 ne le demande.
        expect(permis).toContain('content-type');
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
