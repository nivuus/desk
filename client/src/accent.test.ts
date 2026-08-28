/// <reference types="vite/client" />
/**
 * Tests de `conformer` — le SEUL REMPART du produit contre une couleur
 * illisible reçue du serveur.
 *
 * 🔴 **§7.1 et §7.2 ne voient RIEN de ce chemin, et c'est STRUCTUREL** (E10 du
 * plan) : §7.2 est syntaxique et le déclare lui-même ; §7.1 compare une liste
 * de paires écrite à la main dans `design/contraste.ts`, jamais un produit
 * cartésien, et `client/outils/contraste.mjs` exclut nommément les couleurs
 * composées à l'exécution. **Ces tests sont le seul juge, et leurs rouges ont
 * été VUES.**
 *
 * 🔴 **AUCUNE COULEUR N'EST ÉCRITE EN LITTÉRAL ICI, ET CE N'EST PAS UN
 * ORNEMENT — c'est une correction.** La première rédaction en portait
 * vingt-et-une, et elle a fait passer §7.2 au ROUGE **dans le commit `46e3aa8`,
 * sans que je le voie** : l'exclusion de ce contrôle est posée AU PLUS ÉTROIT
 * (`client/src/design/*.test.ts` seulement), et ces tests-ci ne sont pas du
 * socle. La widener appartiendrait à ⑥, clos, et le sous-bloc A1 s'interdit
 * d'écrire dans `client/src/design/` comme dans `client/outils/` (D-A1-14).
 *
 * ⚠️ **La voie facile aurait été de ranger les fixtures dans un `.json`**, que
 * §7.2 ne balaie pas — c'est-à-dire de satisfaire un contrôle en le VIDANT, ce
 * que ce dépôt refuse. Tout est donc **LU DANS `tokens/couleurs.css`**
 * (`tokens.css` avant l'extraction de la tâche 6, 25 août 2026 — ce fichier
 * ne lit que des COULEURS, jamais une échelle), ce qui est strictement plus
 * fort qu'un littéral : « un contrôle qui a sa propre copie des valeurs
 * valide sa copie » (spec ⑥ §7.1).
 */

import { describe, expect, it } from 'vitest';
import { lireBlocsDeTheme } from './design/tokens';
import tokensCss from './design/tokens/couleurs.css?raw';
import { conformer } from './accent';

const blocs = lireBlocsDeTheme(tokensCss);
const sombre = blocs.find((b) => b.nom === 'racine')!.tokens;

const jeton = (nom: string): string => {
    const v = sombre.get(nom);
    if (!v) throw new Error(`token absent de tokens/couleurs.css : ${nom}`);
    return v;
};

const FONDS = ['--fond-0', '--fond-1', '--fond-2'].map(jeton);
const ACCENT_DU_THEME = jeton('--accent');
/// `--bord` : **1,447 / 1,336 / 1,215** contre les trois fonds sombres — sous
/// le seuil de 3 sur les TROIS. Mesuré par `rapportDeContraste` lui-même le
/// 21 août 2026, jamais estimé.
const ILLISIBLE = jeton('--bord');
/// `--succes` : **8,867 / 8,186 / 7,446** — lisible sur les trois, et
/// DIFFÉRENT de `--accent`, ce qui est indispensable : un fixture égal au repli
/// ne permettrait pas de distinguer « rendu tel quel » de « refusé ».
const LISIBLE = jeton('--succes');

describe('conformer', () => {
    it('🔴 une couleur ILLISIBLE est REFUSÉE, et le thème reprend la main', () => {
        // ROUGE : l'arbre intact avant que `conformer` n'existe ; puis, par
        // mutation, ne plus juger la lisibilité.
        // 🔴 C'EST LA ROUGE QUE LA SPEC EXIGE (critère ③).
        expect(conformer(ILLISIBLE, FONDS, ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
    });

    it('une couleur LISIBLE est rendue telle quelle', () => {
        // ROUGE : rendre toujours `accentDuTheme` ⟹ le mécanisme entier serait
        // inerte. SANS CE TEST, le précédent serait tenu par une fonction qui
        // refuse tout — c'est-à-dire par un produit mort.
        expect(conformer(LISIBLE, FONDS, ACCENT_DU_THEME)).toBe(LISIBLE);
    });

    it('une couleur lisible sur DEUX fonds sur trois est REFUSÉE', () => {
        // ROUGE : `.some()` au lieu de `.every()`.
        // Le troisième fond EST la couleur candidate : le rapport y vaut
        // exactement 1,000, quand les deux premiers valent 8,867 et 8,186.
        // Le cas n'est pas artificiel — une surface dont le fond est justement
        // la teinte de l'application est un cas réel.
        expect(conformer(LISIBLE, [FONDS[0], FONDS[1], LISIBLE], ACCENT_DU_THEME))
            .toBe(ACCENT_DU_THEME);
    });

    it('une forme NON HEXADÉCIMALE est REFUSÉE, sans lever', () => {
        // ROUGE : retirer l'étape « forme » ⟹ `rapportDeContraste` LÈVE (E9).
        // ⚠️ L'assertion est « rend `accentDuTheme` », PAS « ne lève pas » : un
        // test qui n'attendrait qu'une absence d'exception serait satisfait par
        // un `catch`, que ce module s'interdit.
        //
        // Les deux premières formes sont DÉRIVÉES d'un vrai token, et c'est ce
        // qui les rend intéressantes : `luminanceRelative` les ACCEPTE (`#rgb`
        // et `#rrggbbaa` sont dans sa liste), et `conformer` les refuse quand
        // même — parce que le protocole dit `#rrggbb`, et rien d'autre.
        const formes = [
            LISIBLE.slice(0, 4), // trois chiffres
            `${LISIBLE}00`, // huit chiffres
            'red',
            'var(--accent)',
            '',
            '   ',
            'pas une couleur',
        ];
        for (const forme of formes) {
            expect(conformer(forme, FONDS, ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
        }
    });

    it('une casse MAJUSCULE est acceptée après normalisation', () => {
        // ROUGE : comparer sans minusculer ⟹ la MÊME couleur, écrite en
        // majuscules, serait refusée pour un motif de forme.
        expect(conformer(`  ${LISIBLE.toUpperCase()}  `, FONDS, ACCENT_DU_THEME)).toBe(LISIBLE);
    });

    it('la couleur rendue n\'est JAMAIS corrigée', () => {
        // ROUGE : éclaircir la couleur refusée au lieu de la refuser (D10
        // point 3 : « éclaircir ou assombrir la couleur d'une application
        // produirait une teinte que personne n'a choisie »).
        // Ni la couleur acceptée ni le repli ne sont retouchés d'un bit.
        expect(conformer(LISIBLE, FONDS, ACCENT_DU_THEME)).toBe(LISIBLE);
        expect(conformer(ILLISIBLE, FONDS, ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
    });

    it('un fond mal formé fait REFUSER, il ne fait pas lever', () => {
        // ROUGE : ne pas contrôler la forme des FONDS ⟹ `rapportDeContraste`
        // lève sur le fond, et le message de canal de données tue la session.
        // Le cas est RÉEL : `getComputedStyle` rend la CHAÎNE VIDE pour un
        // token absent — donc pour toute page dont le socle n'est pas lié.
        expect(conformer(LISIBLE, [FONDS[0], '', FONDS[2]], ACCENT_DU_THEME))
            .toBe(ACCENT_DU_THEME);
    });

    it('une liste de fonds VIDE fait REFUSER', () => {
        // ROUGE : rendre la candidate quand il n'y a rien à juger ⟹ une page
        // dont les tokens ne sont pas encore posés accepterait n'importe quoi.
        expect(conformer(LISIBLE, [], ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
    });
});
