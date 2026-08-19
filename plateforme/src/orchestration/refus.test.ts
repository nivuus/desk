// Les motifs de refus, et la table de codes qui ne peut pas être incomplète.
//
// 🔴 DEUX GARDES, ET ELLES SONT VOULUES TOUTES LES DEUX. `Record<Motif,
// number>` attrape à la COMPILATION un motif ajouté sans code HTTP ; le
// premier test ci-dessous l'attrape aussi sous `vitest`, pour qui lirait le
// vert de la suite sans lire celui de `npm run typecheck`. Et `tsc` ne verrait
// PAS une clé EN TROP posée par un `as any` — le test si. C'est la doctrine de
// `base/sous-ensemble.test.ts` : chacun couvre l'angle mort de l'autre.

import { describe, expect, it } from 'vitest';
import { BACKEND_STATIQUE, CODE_HTTP, MOTIFS, refuser, type Resultat } from './refus';

describe('les motifs de refus', () => {
    it('🔴 CODE_HTTP porte EXACTEMENT les motifs de MOTIFS, ni plus ni moins', () => {
        // 🔴 La rouge : retirer une entrée de `CODE_HTTP`. Le test tombe, ET
        // `tsc` échoue — les deux ont été jouées. La rouge symétrique, celle
        // que `tsc` ne peut PAS voir, est une clé en trop posée par un cast :
        // elle a été jouée aussi.
        expect(Object.keys(CODE_HTTP).sort()).toEqual([...MOTIFS].sort());
    });

    it('🔴 `non-supporte` vaut 501, jamais 500', () => {
        // 🔴 La rouge : mettre 500. Un 500 se lit comme une PANNE du service ;
        // un backend qui avoue ne pas savoir faire n'est pas en panne, et
        // confondre les deux ferait chercher un défaut là où il n'y en a pas.
        expect(CODE_HTTP['non-supporte']).toBe(501);
        // Et les autres codes, chacun nommé plutôt que déduit.
        expect(CODE_HTTP['vm-inconnue']).toBe(404);
        expect(CODE_HTTP['vm-deja-attribuee']).toBe(409);
        expect(CODE_HTTP['utilisateur-servi']).toBe(409);
        expect(CODE_HTTP['aucune-vm']).toBe(409);
        expect(CODE_HTTP['agent-injoignable']).toBe(503);
    });

    it('🔴 `refuser` dit QUOI a été refusé, et PAR QUI', () => {
        // 🔴 La rouge : omettre `operation`. Un refus qui ne dit pas quelle
        // opération a été refusée n'informe pas — et le jour où un second
        // backend existera, un refus sans `backend` ne dirait pas qui refuse.
        expect(refuser('non-supporte', 'instantane')).toEqual({
            ok: false,
            motif: 'non-supporte',
            operation: 'instantane',
            backend: BACKEND_STATIQUE,
        });
        expect(BACKEND_STATIQUE).toBe('inventaire-statique');
    });

    it('🔴 un refus n’est JAMAIS un succès, et un succès ne porte AUCUN motif', () => {
        // 🔴 La rouge : faire rendre `{ ok: true, motif }` à `refuser`. Le type
        // l'interdit ; ce test le dit à qui lit le code d'exécution. Un refus
        // qui se lirait `ok:true` serait la panne muette exacte que la spec
        // §3.6 nomme — « un `Promise<void>` qui ne fait rien ».
        //
        // ⚠️ `it()` DISTINCT des précédents : `expect` interrompt un test à sa
        // première assertion fausse (leçon ①A/①A-bis de P2).
        const succes: Resultat = { ok: true };
        expect('motif' in succes).toBe(false);
        for (const motif of MOTIFS) {
            const r = refuser(motif, 'demarrer');
            expect(r.ok).toBe(false);
            // Le discriminant fait son travail : hors de la branche, `motif`
            // n'est même pas lisible.
            if (r.ok) throw new Error('un refus s’est déclaré succès');
            expect(r.motif).toBe(motif);
        }
    });
});
