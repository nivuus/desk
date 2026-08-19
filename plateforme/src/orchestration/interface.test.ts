// Les deux listes blanches d'opérations, éprouvées sur leurs VALEURS
// D'EXÉCUTION et non sur leurs seuls types.
//
// 🔴 POURQUOI LES DEUX GARDES. `satisfies readonly Operation[]` est une garde
// de COMPILATION : elle interdit d'écrire un verbe qui n'existe pas. Elle
// n'interdit PAS d'en oublier un. C'est le test d'union ci-dessous qui
// l'interdit, et il tourne sous `vitest` — la doctrine de
// `base/sous-ensemble.test.ts` (« chacun couvre l'angle mort de l'autre »)
// appliquée ici pour la seconde fois.

import { describe, expect, it } from 'vitest';
import { etatDe } from '../agents/fraicheur';
import {
    OPERATIONS,
    OPERATIONS_HORS_HTTP,
    OPERATIONS_HTTP,
    type EtatVm,
} from './interface';

describe('les listes blanches d’opérations', () => {
    it('🔴 leur UNION, triée, est exactement OPERATIONS triée', () => {
        // 🔴 La rouge : retirer `arreter` des deux listes. L'union cesse
        // d'égaler `OPERATIONS`, et le test tombe. C'est le contrôle qui
        // interdit d'ajouter un verbe en l'oubliant : un verbe ajouté à
        // `OPERATIONS` sans place dans l'une des deux listes n'a plus de
        // domicile, et personne ne le verrait autrement.
        const union = [...OPERATIONS_HTTP, ...OPERATIONS_HORS_HTTP].sort();
        expect(union).toEqual([...OPERATIONS].sort());
    });

    it('🔴 leur INTERSECTION est vide', () => {
        // 🔴 La rouge : mettre `etat` dans les deux. `it()` DISTINCT du
        // précédent : `expect` interrompt un test à sa première assertion
        // fausse, et l'union resterait juste en doublons près — la leçon
        // ①A/①A-bis de P2, où une seconde assertion n'était éprouvée par rien.
        const http = new Set<string>(OPERATIONS_HTTP);
        const communes = OPERATIONS_HORS_HTTP.filter((o) => http.has(o));
        expect(communes).toEqual([]);
    });

    it('🔴 `attribuer` n’est PAS une opération HTTP', () => {
        // 🔴 La rouge : l'ajouter à `OPERATIONS_HTTP`. L'attribution d'une VM
        // deviendrait atteignable par tout utilisateur authentifié — il
        // n'existe aucun rôle d'administration dans ce service (D8), et la
        // route ne pourrait donc rien exiger de plus qu'un jeton ordinaire.
        expect((OPERATIONS_HTTP as readonly string[]).includes('attribuer')).toBe(false);
        expect((OPERATIONS_HORS_HTTP as readonly string[]).includes('attribuer')).toBe(true);
    });

    it('EtatVm est exactement ce que `etatDe` produit : `prete` et `injoignable`', () => {
        // 🔴 La rouge : remplacer le réexport par l'union à quatre membres de
        // la spec §3.6 (`arretee`, `demarrage`, `prete`, `injoignable`).
        //
        // ⚠️ ET ELLE EST DOUBLE, PARCE QU'AUCUNE DES DEUX MOITIÉS NE SUFFIT.
        // Sous la mutation seule, `Record<EtatVm, number>` perd deux clés :
        // `npm run typecheck` ÉCHOUE, `vitest` reste vert (esbuild ne
        // typecheck pas). Si l'auteur de la mutation ajoute alors les deux
        // clés manquantes pour faire taire `tsc`, c'est `Object.keys`
        // ci-dessous qui tombe sous `vitest`. Les DEUX rouges ont été jouées
        // et relevées ; sans la seconde, ce test serait un contrôle incapable
        // d'échouer là où on le lit — voir E2.
        const CODES: Record<EtatVm, number> = { prete: 0, injoignable: 1 };
        expect(Object.keys(CODES).sort()).toEqual(['injoignable', 'prete']);

        const MS = 1_787_136_773_742;
        const prete: EtatVm = etatDe(MS, MS);
        const injoignable: EtatVm = etatDe(null, MS);
        expect(prete).toBe('prete');
        expect(injoignable).toBe('injoignable');
    });
});
