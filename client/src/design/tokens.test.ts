import { describe, expect, it } from 'vitest';
import {
    ecartsEntreBlocs,
    lireBlocsDeTheme,
    tokensDeclares,
    tokensReferences,
    valeurDePropriete,
} from './tokens';
import tokensCss from './tokens.css?raw';

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

describe('ecartsEntreBlocs — inclusion ③, les COULEURS de la racine', () => {
    // 🔴 L'ANGLE MORT QUE S3 FERME. Jusqu'ici `ecartsEntreBlocs` comparait
    // clair ⇄ clair (①) et clair ⊆ racine (②), JAMAIS racine ⊆ clair : une
    // couleur déclarée à la racine et oubliée dans les DEUX blocs clairs
    // passait sans un mot. S2 l'a mesuré et versé
    // (`journaux-design-s2/trou-7-4.log`) : `--accent-survol` retiré des deux
    // blocs clairs rendait `écarts : 0`, `exit=0`.
    //
    // ⚠️ ③ NE MORD QUE SUR L'ABSENCE DES DEUX BLOCS À LA FOIS. Une couleur
    // présente dans un seul est déjà attrapée par ①, et la faire compter deux
    // fois ne dirait rien de plus.

    /** Racine avec une couleur intermédiaire — ni `#000` ni `#fff`. */
    const avecCouleur = `
:root {
    --fond-0: #0b0d10;
    --accent-survol: #3b82f6;
    --e-3: 12px;
    --police-ui: system-ui, sans-serif;
    --voile-flottant: rgb(0 0 0 / 0.72);
}

@media (prefers-color-scheme: light) {
    :root:not([data-theme="sombre"]) {
        --fond-0: #ffffff;
        --accent-survol: #1d4ed8;
    }
}

:root[data-theme="clair"] {
    --fond-0: #ffffff;
    --accent-survol: #1d4ed8;
}
`;

    it('rend VIDE quand toutes les couleurs de la racine sont dans les deux blocs clairs', () => {
        expect(ecartsEntreBlocs(lireBlocsDeTheme(avecCouleur))).toEqual([]);
    });

    it('signale une couleur de la racine absente des DEUX blocs clairs', () => {
        // La mutation exacte que S2 a jouée et versée.
        const css = avecCouleur
            .replaceAll('        --accent-survol: #1d4ed8;\n', '')
            .replaceAll('    --accent-survol: #1d4ed8;\n', '');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual([
            'blocs clairs : --accent-survol est une couleur de la racine sans contrepartie claire',
        ]);
    });

    it('ne signale AUCUN écart pour un token HORS THÈME de la liste nommée', () => {
        // `--voile-flottant` est une couleur de la racine absente des deux
        // blocs clairs, et c'est VOULU : les six voiles sont posés sur la
        // vidéo, dont le contenu ne suit aucun thème. Sans cette exemption la
        // fermeture serait rouge sur un fichier correct — le risque §11.
        expect(ecartsEntreBlocs(lireBlocsDeTheme(avecCouleur))).toEqual([]);
    });

    it('ne signale AUCUN écart pour un token de la racine qui N\'EST PAS une couleur', () => {
        // 🔴 SANS CETTE PROPRIÉTÉ la fermeture rendrait des dizaines d'écarts
        // sur l'arbre intact — les crans typographiques, l'espacement, les
        // rayons, les durées et les piles de polices ne vivent QUE dans
        // `:root`, par le §4.4 de la spec. Elle serait rejetée en bloc.
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(avecCouleur));
        expect(ecarts.join(' ')).not.toContain('--e-3');
        expect(ecarts.join(' ')).not.toContain('--police-ui');
    });

    it('signale une couleur NEUVE ajoutée à la seule racine', () => {
        // 🔴 LE TEST QUI ATTRAPE LA VACUITÉ. Un prédicat « est une couleur »
        // qui rendrait `false` pour tout laisserait ce cas passer, et la
        // fermeture entière serait un contrôle qui ne mord jamais.
        const css = avecCouleur.replace(
            '    --fond-0: #0b0d10;',
            '    --fond-0: #0b0d10;\n    --bord-neuf: #4b5563;',
        );
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual([
            'blocs clairs : --bord-neuf est une couleur de la racine sans contrepartie claire',
        ]);
    });

    it('reconnaît une couleur écrite en rgb() et en hsl(), pas seulement en #', () => {
        // Le prédicat décide sur la VALEUR, jamais sur le nom : un préfixe de
        // nom est une convention qu'une faute de frappe contourne.
        const css = avecCouleur.replace(
            '    --fond-0: #0b0d10;',
            '    --fond-0: #0b0d10;\n    --a-rgb: rgb(12 34 56);\n    --a-hsl: hsl(210 40% 30%);',
        );
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual([
            'blocs clairs : --a-hsl est une couleur de la racine sans contrepartie claire',
            'blocs clairs : --a-rgb est une couleur de la racine sans contrepartie claire',
        ]);
    });

    it('rend ZÉRO écart sur le VRAI tokens.css', () => {
        // 🔴 Ce test dépend de `test: { css: true }` dans `vite.config.ts` :
        // sans lui `?raw` rend la chaîne VIDE, `lireBlocsDeTheme` ne trouve
        // aucun bloc, et l'assertion « zéro écart » passerait en ne mesurant
        // rien. L'assertion sur le compte de blocs est ce qui l'empêche.
        const blocs = lireBlocsDeTheme(tokensCss);
        expect(blocs).toHaveLength(3);
        expect(ecartsEntreBlocs(blocs)).toEqual([]);
    });
});
