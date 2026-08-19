import { describe, expect, it } from 'vitest';
import {
    ecartsEntreBlocs,
    lireBlocsDeTheme,
    tokensDeclares,
    tokensReferences,
    valeurDePropriete,
} from './tokens';

/**
 * Un CSS de DÉMONSTRATION, jamais le vrai `tokens.css`. Ces tests éprouvent la
 * FONCTION ; que le vrai fichier soit conforme est éprouvé par
 * `client/outils/blocs-de-theme.mjs`, qui le lit. Un parseur juste sur un
 * fichier qu'il ne lit pas est vert pour rien.
 */
const demonstration = `
:root {
    color-scheme: dark;
    --fond-0: #0b0d10;
    --texte-fort: #e6e8eb;
}

@media (prefers-color-scheme: light) {
    :root:not([data-theme="sombre"]) {
        color-scheme: light;
        --fond-0: #ffffff;
        --texte-fort: #10131a;
    }
}

:root[data-theme="clair"] {
    color-scheme: light;
    --fond-0: #ffffff;
    --texte-fort: #10131a;
}
`;

describe('lireBlocsDeTheme', () => {
    it('découpe le CSS en exactement trois blocs nommés', () => {
        const blocs = lireBlocsDeTheme(demonstration);
        expect(blocs.map((b) => b.nom)).toEqual(['racine', 'media-clair', 'attribut-clair']);
    });

    it('le bloc « racine » est celui SANS condition, pas celui à attribut', () => {
        const blocs = lireBlocsDeTheme(demonstration);
        const racine = blocs.find((b) => b.nom === 'racine');
        // La palette SOMBRE est celle du bloc sans condition (spec §4.2).
        expect(racine?.tokens.get('--fond-0')).toBe('#0b0d10');
        const attribut = blocs.find((b) => b.nom === 'attribut-clair');
        expect(attribut?.tokens.get('--fond-0')).toBe('#ffffff');
    });

    it('`color-scheme` est déclaré dans les TROIS blocs (D9)', () => {
        // Ce n'est PAS un token `--*` : il ne compte ni dans l'égalité
        // d'ensembles de §7.4, ni dans les orphelins de §7.6. Il est donc
        // vérifié à part — sans quoi rien ne le garderait.
        const blocs = lireBlocsDeTheme(demonstration);
        expect(blocs.map((b) => valeurDePropriete(b, 'color-scheme'))).toEqual([
            'dark',
            'light',
            'light',
        ]);
    });
});

describe('tokensDeclares', () => {
    it("rend l'UNION des trois blocs, pas le seul :root", () => {
        const css = demonstration.replace('--texte-fort: #10131a;\n}\n', '--texte-fort: #10131a;\n    --propre-au-clair: 1px;\n}\n');
        expect(tokensDeclares(css)).toEqual(
            new Set(['--fond-0', '--texte-fort', '--propre-au-clair']),
        );
    });
});

describe('tokensReferences', () => {
    it('trouve `var(--a)` et `var(--b, repli)`, et IGNORE ce qui est commenté', () => {
        const css = `
            .a { color: var(--encre); background: var(--fond, #fff); }
            /* .mort { color: var(--jamais-employe); } */
        `;
        expect(tokensReferences(css)).toEqual(new Set(['--encre', '--fond']));
    });
});

describe('ecartsEntreBlocs', () => {
    it('rend VIDE sur trois blocs qui déclarent le même ensemble', () => {
        expect(ecartsEntreBlocs(lireBlocsDeTheme(demonstration))).toEqual([]);
    });

    it('signale un token MANQUANT dans le bloc média', () => {
        const css = demonstration.replace('        --texte-fort: #10131a;\n', '');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toHaveLength(1);
        expect(ecarts[0]).toContain('media-clair');
        expect(ecarts[0]).toContain('--texte-fort');
    });

    it("signale un token manquant dans le bloc ATTRIBUT — l'AUTRE sens", () => {
        // 🔴 Le défaut NATUREL de ce contrôle est de ne tester l'inclusion que
        // dans UN sens. Il laisserait passer exactement la dérive que §7.4
        // existe pour empêcher, la palette claire étant déclarée DEUX fois
        // (spec §4.2).
        //
        // ⚠️ CE TEST A ÉTÉ ÉCRIT FAUX UNE PREMIÈRE FOIS, et la mutation l'a
        // révélé : il ajoutait un token à `attribut-clair`, ce qui fait tomber
        // le PREMIER sens (`attribut ⊆ media`), pas le second. Retirer la
        // boucle du second sens le laissait VERT. Le seul cas qui l'épingle
        // est un token présent dans `media-clair` et ABSENT d'`attribut-clair`.
        const css = demonstration.replace('    --texte-fort: #10131a;\n}\n', '}\n');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual(['attribut-clair : --texte-fort manquant']);
    });

    it('signale un token clair SANS contrepartie dans le bloc sans condition', () => {
        // Un thème clair qui surcharge un token qui n'existe pas en sombre est
        // une faute de frappe, pas une intention. C'est l'inclusion ② de
        // `ecartsEntreBlocs`, et rien d'autre ne la couvrait.
        const css = demonstration
            .replace(':root:not([data-theme="sombre"]) {', ':root:not([data-theme="sombre"]) {\n        --orphelin-clair: 0;')
            .replace(':root[data-theme="clair"] {', ':root[data-theme="clair"] {\n    --orphelin-clair: 0;');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual([
            'racine : --orphelin-clair surchargé par attribut-clair sans y être déclaré',
            'racine : --orphelin-clair surchargé par media-clair sans y être déclaré',
        ]);
    });

    it("NOMME le bloc et le token, jamais un booléen", () => {
        const css = demonstration.replace('        --fond-0: #ffffff;\n', '');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual(['media-clair : --fond-0 manquant']);
    });
});
