import { describe, expect, it } from 'vitest';
import {
    ecartsEntreBlocs,
    lireBlocsBruts,
    lireBlocsDeTheme,
    tokensDeclares,
    tokensReferences,
    propertyValue,
} from './tokens';
import couleursCss from './tokens/couleurs.css?raw';
import echellesCss from './tokens/echelles.css?raw';

/**
 * 🔴 DEUX FICHIERS DEPUIS L'EXTRACTION DE LA TÂCHE 6 (25 août 2026), PLUS LE
 * VRAI `tokens.css` : `couleurs.css` porte les trois blocs de thème,
 * `echelles.css` un seul bloc `racine` sans condition. Concaténés, c'est
 * EXACTEMENT ce que `tokens.css` important les deux servait avant
 * l'extraction — voir `describe('rend ZÉRO écart …')` plus bas, qui l'éprouve
 * sur le contenu RÉEL.
 */
const tokensCss = `${couleursCss}\n${echellesCss}`;

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
    it('splits the CSS into exactly three named blocks', () => {
        const blocs = lireBlocsDeTheme(demonstration);
        expect(blocs.map((b) => b.nom)).toEqual(['racine', 'media-clair', 'attribut-clair']);
    });

    it('the « root » block is the one WITHOUT a condition, not the attribute one', () => {
        const blocs = lireBlocsDeTheme(demonstration);
        const racine = blocs.find((b) => b.nom === 'racine');
        // La palette SOMBRE est celle du bloc sans condition (spec §4.2).
        expect(racine?.tokens.get('--fond-0')).toBe('#0b0d10');
        const attribut = blocs.find((b) => b.nom === 'attribut-clair');
        expect(attribut?.tokens.get('--fond-0')).toBe('#ffffff');
    });

    it('`color-scheme` is declared in the THREE blocks (D9)', () => {
        // Ce n'est PAS un token `--*` : il ne compte ni dans l'égalité
        // d'ensembles de §7.4, ni dans les orphelins de §7.6. Il est donc
        // vérifié à part — sans quoi rien ne le garderait.
        const blocs = lireBlocsDeTheme(demonstration);
        expect(blocs.map((b) => propertyValue(b, 'color-scheme'))).toEqual([
            'dark',
            'light',
            'light',
        ]);
    });
});

describe('lireBlocsDeTheme — two occurrences OF THE SAME block MERGE', () => {
    // 🔴 LE CAS QUE L'EXTRACTION DE LA TÂCHE 6 REND RÉEL : `couleurs.css` et
    // `echelles.css` déclarent chacun leur propre `:root {}` sans condition.
    // Concaténés — c'est ce que fait tout lecteur qui a besoin des deux — le
    // texte porte DEUX occurrences physiques de « racine ». Sans fusion,
    // `Array.find` ne verrait que la PREMIÈRE et une `Map` clé par nom ne
    // garderait que la DERNIÈRE : dans les deux cas, la moitié des tokens
    // disparaîtrait EN SILENCE.
    const deuxRacines = ':root {\n    --a: 1;\n}\n:root {\n    --b: 2;\n}\n';

    it('returns ONE SINGLE « root » block, not two', () => {
        const blocs = lireBlocsDeTheme(deuxRacines);
        expect(blocs).toHaveLength(1);
        expect(blocs[0].nom).toBe('racine');
    });

    it('UNITES the tokens of both occurrences — neither the first alone, nor the last alone', () => {
        const blocs = lireBlocsDeTheme(deuxRacines);
        expect(blocs[0].tokens.get('--a')).toBe('1');
        expect(blocs[0].tokens.get('--b')).toBe('2');
    });

    it('does NOT merge a « root » block with a light block of the same text', () => {
        // La fusion doit rester bornée au NOM : les trois blocs de thème
        // doivent continuer à se distinguer même quand « racine » se
        // dédouble.
        const texte =
            deuxRacines +
            '@media (prefers-color-scheme: light) {\n' +
            '    :root:not([data-theme="sombre"]) {\n        --a: 3;\n    }\n' +
            '}\n' +
            ':root[data-theme="clair"] {\n    --a: 4;\n}\n';
        const blocs = lireBlocsDeTheme(texte);
        expect(blocs.map((b) => b.nom).sort()).toEqual([
            'attribut-clair',
            'media-clair',
            'racine',
        ]);
        const racine = blocs.find((b) => b.nom === 'racine');
        expect(racine?.tokens.get('--a')).toBe('1');
        expect(racine?.tokens.get('--b')).toBe('2');
    });

    it('`lireBlocsBruts` DOES NOT MERGE — that is its whole point', () => {
        // 🔴 CORRECTIF DE LA REVUE (round 1) : `lireBlocsDeTheme` borne
        // désormais son compte à 3 PAR CONSTRUCTION, donc AUCUNE assertion
        // sur `lireBlocsDeTheme(...).length` ne peut plus dénoncer un
        // `:root` de trop — `lireBlocsBruts` est le SEUL compte qui varie
        // encore avec le nombre d'occurrences physiques.
        const bruts = lireBlocsBruts(deuxRacines);
        expect(bruts).toHaveLength(2);
        expect(bruts.map((b) => b.nom)).toEqual(['racine', 'racine']);
        // Et la fusion qui en découle rend bien UN SEUL bloc — la même
        // propriété que les deux tests ci-dessus, vue depuis l'autre bout.
        expect(lireBlocsDeTheme(deuxRacines)).toHaveLength(1);
    });
});

describe('tokensDeclares', () => {
    it("returns the UNION of the three blocks, not :root alone", () => {
        const css = demonstration.replace('--texte-fort: #10131a;\n}\n', '--texte-fort: #10131a;\n    --propre-au-clair: 1px;\n}\n');
        expect(tokensDeclares(css)).toEqual(
            new Set(['--fond-0', '--texte-fort', '--propre-au-clair']),
        );
    });
});

describe('tokensReferences', () => {
    it('finds `var(--a)` and `var(--b, repli)`, and IGNORES what is commented out', () => {
        const css = `
            .a { color: var(--encre); background: var(--fond, #fff); }
            /* .mort { color: var(--jamais-employe); } */
        `;
        expect(tokensReferences(css)).toEqual(new Set(['--encre', '--fond']));
    });
});

describe('ecartsEntreBlocs', () => {
    it('returns EMPTY on three blocks that declare the same set', () => {
        expect(ecartsEntreBlocs(lireBlocsDeTheme(demonstration))).toEqual([]);
    });

    it('reports a token MISSING from the media block', () => {
        const css = demonstration.replace('        --texte-fort: #10131a;\n', '');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toHaveLength(1);
        expect(ecarts[0]).toContain('media-clair');
        expect(ecarts[0]).toContain('--texte-fort');
    });

    it("reports a token missing from the ATTRIBUTE block — the OTHER direction", () => {
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
        expect(ecarts).toEqual(['attribut-clair: --texte-fort missing']);
    });

    it('reports a light token WITHOUT a counterpart in the unconditional block', () => {
        // Un thème clair qui surcharge un token qui n'existe pas en sombre est
        // une faute de frappe, pas une intention. C'est l'inclusion ② de
        // `ecartsEntreBlocs`, et rien d'autre ne la couvrait.
        const css = demonstration
            .replace(':root:not([data-theme="sombre"]) {', ':root:not([data-theme="sombre"]) {\n        --orphelin-clair: 0;')
            .replace(':root[data-theme="clair"] {', ':root[data-theme="clair"] {\n    --orphelin-clair: 0;');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual([
            'root: --orphelin-clair overridden by attribut-clair without being declared there',
            'root: --orphelin-clair overridden by media-clair without being declared there',
        ]);
    });

    it("NAMES the block and the token, never a boolean", () => {
        const css = demonstration.replace('        --fond-0: #ffffff;\n', '');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual(['media-clair: --fond-0 missing']);
    });
});

describe('ecartsEntreBlocs — inclusion ③, the root COLOURS', () => {
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
    const withColour = `
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

    it('returns EMPTY when all the root colours are in both light blocks', () => {
        expect(ecartsEntreBlocs(lireBlocsDeTheme(withColour))).toEqual([]);
    });

    it('reports a root colour missing from BOTH light blocks', () => {
        // La mutation exacte que S2 a jouée et versée.
        const css = withColour
            .replaceAll('        --accent-survol: #1d4ed8;\n', '')
            .replaceAll('    --accent-survol: #1d4ed8;\n', '');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual([
            'light blocks: --accent-survol is a root colour without a light counterpart',
        ]);
    });

    it('reports NO gap for an OUT-OF-THEME token of the named list', () => {
        // `--voile-flottant` est une couleur de la racine absente des deux
        // blocs clairs, et c'est VOULU : les six voiles sont posés sur la
        // vidéo, dont le contenu ne suit aucun thème. Sans cette exemption la
        // fermeture serait rouge sur un fichier correct — le risque §11.
        expect(ecartsEntreBlocs(lireBlocsDeTheme(withColour))).toEqual([]);
    });

    it('reports NO gap for a root token that IS NOT a colour', () => {
        // 🔴 SANS CETTE PROPRIÉTÉ la fermeture rendrait des dizaines d'écarts
        // sur l'arbre intact — les crans typographiques, l'espacement, les
        // rayons, les durées et les piles de polices ne vivent QUE dans
        // `:root`, par le §4.4 de la spec. Elle serait rejetée en bloc.
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(withColour));
        expect(ecarts.join(' ')).not.toContain('--e-3');
        expect(ecarts.join(' ')).not.toContain('--police-ui');
    });

    it('reports a NEW colour added to the root alone', () => {
        // 🔴 LE TEST QUI ATTRAPE LA VACUITÉ. Un prédicat « est une couleur »
        // qui rendrait `false` pour tout laisserait ce cas passer, et la
        // fermeture entière serait un contrôle qui ne mord jamais.
        const css = withColour.replace(
            '    --fond-0: #0b0d10;',
            '    --fond-0: #0b0d10;\n    --bord-neuf: #4b5563;',
        );
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual([
            'light blocks: --bord-neuf is a root colour without a light counterpart',
        ]);
    });

    it('recognises a colour written in rgb() and in hsl(), not only in #', () => {
        // Le prédicat décide sur la VALEUR, jamais sur le nom : un préfixe de
        // nom est une convention qu'une faute de frappe contourne.
        const css = withColour.replace(
            '    --fond-0: #0b0d10;',
            '    --fond-0: #0b0d10;\n    --a-rgb: rgb(12 34 56);\n    --a-hsl: hsl(210 40% 30%);',
        );
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual([
            'light blocks: --a-hsl is a root colour without a light counterpart',
            'light blocks: --a-rgb is a root colour without a light counterpart',
        ]);
    });

    it('returns ZERO gaps on the REAL tokens/couleurs.css and tokens/echelles.css', () => {
        // 🔴 Ce test dépend de `test: { css: true }` dans `vite.config.ts` :
        // sans lui `?raw` rend la chaîne VIDE, `lireBlocsDeTheme` ne trouve
        // aucun bloc, et l'assertion « zéro écart » passerait en ne mesurant
        // rien. L'assertion sur le compte de blocs est ce qui l'empêche.
        // ⚠️ TROIS blocs LOGIQUES, PAS QUATRE : `couleurs.css` et
        // `echelles.css` déclarent chacun un `:root {}` sans condition, et
        // c'est la fusion ajoutée par la tâche 6 qui les ramène à UN SEUL
        // bloc « racine ». 🔴 CE COMPTE-CI EST DÉSORMAIS BORNÉ À 3 PAR
        // CONSTRUCTION — voir le test suivant, qui porte le compte capable
        // de dénoncer un `:root` de trop.
        const blocs = lireBlocsDeTheme(tokensCss);
        expect(blocs).toHaveLength(3);
        expect(ecartsEntreBlocs(blocs)).toEqual([]);
    });

    it('EXACTLY FOUR PHYSICAL occurrences — the guard that one `:root` too many must turn red', () => {
        // 🔴 CORRECTIF DE LA REVUE (round 1, 25 août 2026). Mesuré : ajouter
        // `:root { --e-4: 999rem; }` en trop dans `tokens/echelles.css` (une
        // régression réelle — tout `--e-4` passerait de 1rem à 999rem) se
        // fond dans le bloc « racine » existant SANS FAIRE BOUGER LE COMPTE
        // LOGIQUE ci-dessus, qui reste à 3. `lireBlocsBruts`, qui ne fusionne
        // rien, est le SEUL compte que cette régression fait encore varier :
        // 4 aujourd'hui (racine de `couleurs.css`, media-clair,
        // attribut-clair, racine de `echelles.css`), 5 avec l'ajout en trop.
        const bruts = lireBlocsBruts(tokensCss);
        expect(bruts).toHaveLength(4);
        expect(bruts.map((b) => b.nom)).toEqual([
            'racine',
            'media-clair',
            'attribut-clair',
            'racine',
        ]);
    });
});
