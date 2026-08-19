// La fraîcheur d'un agent : `prete` ou `injoignable`, décidé sur `vu_a`.
//
// 🔴 LA ROUGE CENTRALE DE CE FICHIER EST LA TRANSITION, pas les deux états
// pris séparément. La spec §4 P3 ④ l'écrit littéralement : « un seuil qui
// n'est jamais atteint dans le test ne prouve rien : le test DOIT voir la
// transition ». Deux tests qui poseraient chacun leur instant et leur `vu_a`
// pourraient tous deux passer sur une implémentation qui ne lit pas l'horloge
// du tout — c'est le piège que le critère ② de P2 a nommé pour les jetons, et
// qu'une horloge FIGÉE rejouerait ici.
//
// ⚠️ L'HORLOGE EST UN PARAMÈTRE, jamais lue dans le module : c'est ce qui rend
// les instants assertables sur des valeurs exactes, et c'est la règle du dépôt
// (`depot/session.ts`, `identite/jeton.ts`, `signaling/ice.ts`).
//
// ⚠️ LES INSTANTS SONT DE VRAIES ÉPOQUES EN MILLISECONDES, jamais de petits
// nombres commodes. C'est la leçon de `base/harnais.ts` : un `1_000` tient
// dans un entier de 4 octets, `Date.now()` non — et ici il vaut de surcroît
// que le test exerce l'ordre de grandeur que le service manipule réellement.

import { describe, expect, it } from 'vitest';
import { SEUIL_INJOIGNABLE_MS, etatDe } from './fraicheur';

/// Une époque réelle : 19 août 2026, à la milliseconde près.
const T0 = 1_787_000_000_000;

describe('la fraîcheur d’un agent', () => {
    it('une VM qui n’a JAMAIS battu est injoignable', () => {
        // 🔴 `null` n'est pas `0` : `depot/agent.ts` le dit déjà de la colonne.
        // Rendre `prete` ici annoncerait prête une VM dont personne n'a jamais
        // eu la moindre nouvelle — exactement l'inverse de ce que la colonne
        // signifie.
        expect(etatDe(null, T0)).toBe('injoignable');
    });

    it('une VM vue à l’instant même est prête', () => {
        expect(etatDe(T0, T0)).toBe('prete');
    });

    it('🔴 une VM vue il y a SEUIL + 1 ms est injoignable', () => {
        // Sans comparaison au seuil, une implémentation qui rendrait `prete`
        // dès que `vu_a` n'est pas `null` passerait les deux tests ci-dessus.
        expect(etatDe(T0, T0 + SEUIL_INJOIGNABLE_MS + 1)).toBe('injoignable');
    });

    it('🔴 LA TRANSITION est observée : même `vu_a`, deux instants, et la borne est assiégée des DEUX côtés', () => {
        // 🔴 C'est la rouge littérale du critère ④ de la spec. Le MÊME `vu_a`
        // est jugé à deux instants, et les deux instants encadrent la borne à
        // une milliseconde près : une horloge figée — ou ignorée — rendrait
        // deux fois la même valeur et ce test tomberait.
        const vuA = T0;
        expect(etatDe(vuA, vuA + SEUIL_INJOIGNABLE_MS)).toBe('prete');
        expect(etatDe(vuA, vuA + SEUIL_INJOIGNABLE_MS + 1)).toBe('injoignable');
    });
});
