import { describe, expect, it } from 'vitest';
import { lireBlocsDeTheme } from './tokens';
import { PAIRES, evaluer, luminanceRelative, rapportDeContraste } from './contraste';

/**
 * 🔴 UN TEST DE CONTRASTE ÉCRIT AVEC LES COULEURS DU PRODUIT VALIDE LE PRODUIT
 * CONTRE LUI-MÊME. Les quatre premiers tests emploient donc des vecteurs dont
 * la valeur est fixée par la norme WCAG 2.1 elle-même, et non par nos tokens.
 * Que la palette RÉELLE tienne les seuils est éprouvé ailleurs, par
 * `client/outils/contraste.mjs`, qui lit `tokens.css`.
 */
describe('rapportDeContraste — vecteurs extérieurs à notre palette', () => {
    it('rend 21 sur noir contre blanc : le maximum absolu de l’échelle', () => {
        expect(rapportDeContraste('#000000', '#ffffff')).toBeCloseTo(21, 5);
    });

    it('rend 1 sur une couleur contre elle-même : le minimum absolu', () => {
        expect(rapportDeContraste('#ffffff', '#ffffff')).toBeCloseTo(1, 10);
    });

    it('est symétrique — la formule ordonne ses deux termes', () => {
        // `(L + 0.05) / (l + 0.05)` avec L ≥ l. Oublier l'ordre casse ici.
        for (const [a, b] of [
            ['#000000', '#ffffff'],
            ['#7aa2f7', '#0b0d10'],
            ['#c02b2b', '#f6f7f9'],
        ]) {
            expect(rapportDeContraste(a, b)).toBeCloseTo(rapportDeContraste(b, a), 10);
        }
    });

    it('applique la correction gamma, et non une moyenne linéaire', () => {
        // 🔴 C'EST LE VECTEUR QUI DISCRIMINE. `#808080` est à mi-course des
        // canaux, donc une moyenne linéaire rendrait 0,5. La luminance
        // relative vraie vaut ≈ 0,2159. La faute est classique et rend des
        // rapports PLAUSIBLES mais faux sur toutes les couleurs
        // intermédiaires — c'est-à-dire sur les 50 paires réelles, là où
        // noir/blanc rend 21 dans les deux cas.
        expect(luminanceRelative('#808080')).toBeCloseTo(0.2159, 4);
    });
});

describe('PAIRES', () => {
    it('en compte 50, et ce sont des paires DÉCLARÉES', () => {
        // Sept encres × trois fonds au seuil 4,5, plus `--bord-fort` sur les
        // trois fonds au seuil 3, plus `--sur-accent` sur `--accent` au
        // seuil 4,5 — le tout × 2 thèmes. Un produit cartésien en donnerait
        // bien davantage, et inclurait `--bord`.
        expect(PAIRES).toHaveLength(50);
        expect(new Set(PAIRES.map((p) => p.theme))).toEqual(new Set(['sombre', 'clair']));
    });

    it("ne porte JAMAIS `--bord`, et c'est une décision", () => {
        // `--bord` rend 1,45 (sombre) et 1,40 (clair) sur `--fond-0` : il est
        // réservé aux séparateurs purement décoratifs, que WCAG 1.4.11 exempte.
        // Sans ce test, un successeur bien intentionné l'ajouterait et rendrait
        // le contrôle rouge pour toujours, donc bon à assouplir.
        // ⚠️ Aucune commande ne peut en revanche vérifier qu'on n'a pas employé
        // `--bord` là où il fallait `--bord-fort` : c'est une règle de revue.
        expect(PAIRES.filter((p) => p.encre === '--bord' || p.fond === '--bord')).toEqual([]);
    });
});

/** Palette synthétique : aucune valeur du produit n'est recopiée ici. */
function palette(couleurs: Record<string, [string, string]>): string {
    const sombre = Object.entries(couleurs).map(([n, [s]]) => `${n}: ${s};`).join('\n    ');
    const clair = Object.entries(couleurs).map(([n, [, c]]) => `${n}: ${c};`).join('\n    ');
    return `:root { color-scheme: dark; ${sombre} }
@media (prefers-color-scheme: light) { :root:not([data-theme="sombre"]) { color-scheme: light; ${clair} } }
:root[data-theme="clair"] { color-scheme: light; ${clair} }`;
}

const TOUS = [
    '--fond-0', '--fond-1', '--fond-2', '--bord-fort', '--texte-fort', '--texte',
    '--texte-faible', '--accent', '--sur-accent', '--succes', '--alerte', '--danger',
];

describe('evaluer', () => {
    it('rend aucun échec sur une palette conforme', () => {
        const conforme = Object.fromEntries(
            TOUS.map((n) => [
                n,
                n.startsWith('--fond') || n === '--sur-accent'
                    ? (['#ffffff', '#000000'] as [string, string])
                    : (['#000000', '#ffffff'] as [string, string]),
            ]),
        );
        const resultat = evaluer(lireBlocsDeTheme(palette(conforme)));
        expect(resultat.echecs).toEqual([]);
        expect(resultat.verifiees).toBe(50);
        expect(resultat.minimum).toBeCloseTo(21, 5);
    });

    it("NOMME le thème, l'encre, le fond et le rapport de chaque échec", () => {
        // Un booléen ne dirait pas quoi corriger.
        const fautif = Object.fromEntries(
            TOUS.map((n) => [
                n,
                n.startsWith('--fond') || n === '--sur-accent'
                    ? (['#ffffff', '#000000'] as [string, string])
                    : n === '--texte-faible'
                      ? (['#eeeeee', '#111111'] as [string, string]) // sombre : clair sur clair
                      : (['#000000', '#ffffff'] as [string, string]),
            ]),
        );
        const resultat = evaluer(lireBlocsDeTheme(palette(fautif)));
        expect(resultat.echecs.length).toBeGreaterThan(0);
        const premier = resultat.echecs[0];
        expect(premier.paire.theme).toBe('sombre');
        expect(premier.paire.encre).toBe('--texte-faible');
        expect(premier.paire.fond).toMatch(/^--fond-[012]$/);
        expect(premier.rapport).toBeLessThan(premier.paire.seuil);
    });
});
