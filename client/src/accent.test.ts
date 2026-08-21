/**
 * Tests de `conformer` — le SEUL REMPART du produit contre une couleur
 * illisible reçue du serveur.
 *
 * 🔴 **§7.1 et §7.2 ne voient RIEN de ce chemin, et c'est STRUCTUREL** (E10 du
 * plan) : §7.2 est syntaxique et le déclare ; §7.1 compare une liste de paires
 * écrite à la main dans `design/contraste.ts`, jamais un produit cartésien, et
 * `client/outils/contraste.mjs` exclut nommément les couleurs composées à
 * l'exécution. **Ces tests sont le seul juge, et leurs rouges ont été VUES.**
 */

import { describe, expect, it } from 'vitest';
import { conformer } from './accent';

// Les trois fonds du thème SOMBRE, repris VERBATIM de `design/tokens.css`
// (`--fond-0`, `--fond-1`, `--fond-2`). Les recopier ici est une duplication
// assumée : un test qui lirait le fichier mesurerait le fichier, pas la règle.
const FONDS_SOMBRE = ['#0b0d10', '#14171c', '#1c2027'] as const;
// `--accent` du bloc sombre.
const ACCENT_DU_THEME = '#7aa2f7';

describe('conformer', () => {
    it('🔴 un gris ILLISIBLE est REFUSÉ, et le thème reprend la main', () => {
        // ROUGE : l'arbre intact avant que `conformer` n'existe ; puis, par
        // mutation, rendre `candidate` sans juger la lisibilité.
        // `#1e2229` contre `--fond-0` (#0b0d10) vaut 1,219:1 — mesuré par
        // `rapportDeContraste` lui-même, pas estimé.
        // 🔴 C'EST LA ROUGE QUE LA SPEC EXIGE (critère ③).
        expect(conformer('#1e2229', FONDS_SOMBRE, ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
    });

    it('une couleur LISIBLE est rendue telle quelle', () => {
        // ROUGE : rendre toujours `accentDuTheme` ⟹ le mécanisme entier serait
        // inerte. SANS CE TEST, le précédent serait tenu par une fonction qui
        // refuse tout — c'est-à-dire par un produit mort.
        expect(conformer('#fa8c16', FONDS_SOMBRE, ACCENT_DU_THEME)).toBe('#fa8c16');
    });

    it('une couleur lisible sur DEUX fonds sur trois est REFUSÉE', () => {
        // ROUGE : `.some()` au lieu de `.every()` — ici, sortir de la boucle au
        // premier fond qui passe.
        // `#5060a0` vaut 3,263 / 3,013 / 2,740 contre les trois fonds : il passe
        // le seuil de 3 sur les DEUX premiers et le rate sur le troisième.
        expect(conformer('#5060a0', FONDS_SOMBRE, ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
    });

    it('une forme NON HEXADÉCIMALE est REFUSÉE, sans lever', () => {
        // ROUGE : retirer l'étape « forme » ⟹ `rapportDeContraste` LÈVE (E9).
        // ⚠️ L'assertion est « rend `accentDuTheme` », PAS « ne lève pas » : un
        // test qui n'attendrait qu'une absence d'exception serait satisfait par
        // un `catch`, que ce module s'interdit.
        for (const forme of ['rgb(1,2,3)', 'red', '#abc', '#abcdef00', '', '  ', '#12345g', 'var(--accent)']) {
            expect(conformer(forme, FONDS_SOMBRE, ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
        }
    });

    it('une casse MAJUSCULE est acceptée après normalisation', () => {
        // ROUGE : comparer sans minusculer ⟹ `#FA8C16` serait refusé pour un
        // motif de forme, alors que c'est la MÊME couleur.
        expect(conformer('  #FA8C16  ', FONDS_SOMBRE, ACCENT_DU_THEME)).toBe('#fa8c16');
    });

    it('la couleur rendue n\'est JAMAIS corrigée', () => {
        // ROUGE : éclaircir la couleur refusée au lieu de la refuser (D10
        // point 3 : « éclaircir ou assombrir la couleur d'une application
        // produirait une teinte que personne n'a choisie »).
        // Ni la couleur acceptée ni le repli ne sont retouchés d'un bit.
        expect(conformer('#fa8c16', FONDS_SOMBRE, ACCENT_DU_THEME)).toBe('#fa8c16');
        expect(conformer('#1e2229', FONDS_SOMBRE, ACCENT_DU_THEME)).toBe('#7aa2f7');
    });

    it('un fond mal formé fait REFUSER, il ne fait pas lever', () => {
        // ROUGE : ne pas contrôler la forme des FONDS ⟹ `rapportDeContraste`
        // lève sur le fond, et le message de canal de données tue la session.
        // Le cas est réel : `getComputedStyle` rend la chaîne VIDE pour un
        // token absent, ce qui est exactement ce que la rouge du critère ②
        // provoque en posant le token sur `document.body`.
        expect(conformer('#fa8c16', ['#0b0d10', '', '#1c2027'], ACCENT_DU_THEME))
            .toBe(ACCENT_DU_THEME);
    });

    it('une liste de fonds VIDE fait REFUSER', () => {
        // ROUGE : rendre `candidate` quand il n'y a rien à juger ⟹ une page
        // dont les tokens ne sont pas encore posés accepterait n'importe quoi.
        expect(conformer('#fa8c16', [], ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
    });
});
