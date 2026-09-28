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
 * 🔴 TWO FILES SINCE THE EXTRACTION OF TASK 6 (August 25th, 2026), PLUS THE
 * REAL `tokens.css`: `couleurs.css` carries the three theme blocks,
 * `echelles.css` a single unconditional `racine` block. Concatenated, it is
 * EXACTLY what `tokens.css` importing both served before the
 * extraction — see `describe('returns ZERO gaps …')` below, which exercises it
 * on the REAL content.
 */
const tokensCss = `${couleursCss}\n${echellesCss}`;

/**
 * A DEMONSTRATION CSS, never the real `tokens.css`. These tests exercise the
 * FUNCTION; that the real file complies is exercised by
 * `client/outils/blocs-de-theme.mjs`, which reads it. A parser right about a
 * file it does not read is green for nothing.
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
        // The DARK palette is that of the unconditional block (spec §4.2).
        expect(racine?.tokens.get('--fond-0')).toBe('#0b0d10');
        const attribut = blocs.find((b) => b.nom === 'attribut-clair');
        expect(attribut?.tokens.get('--fond-0')).toBe('#ffffff');
    });

    it('`color-scheme` is declared in the THREE blocks (D9)', () => {
        // It is NOT a `--*` token: it counts neither in the set equality
        // of §7.4, nor in the orphans of §7.6. It is therefore
        // checked separately — otherwise nothing would guard it.
        const blocs = lireBlocsDeTheme(demonstration);
        expect(blocs.map((b) => propertyValue(b, 'color-scheme'))).toEqual([
            'dark',
            'light',
            'light',
        ]);
    });
});

describe('lireBlocsDeTheme — two occurrences OF THE SAME block MERGE', () => {
    // 🔴 THE CASE THE EXTRACTION OF TASK 6 MAKES REAL: `couleurs.css` and
    // `echelles.css` each declare their own unconditional `:root {}`.
    // Concatenated — which is what any reader needing both does — the
    // text carries TWO physical occurrences of "racine". Without merging,
    // `Array.find` would only see the FIRST and a `Map` keyed by name would only
    // keep the LAST: in both cases, half of the tokens
    // would disappear SILENTLY.
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
        // The merge must stay bounded to the NAME: the three theme blocks
        // must keep being told apart even when "racine" is
        // doubled.
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
        // 🔴 REVIEW FIX (round 1): `lireBlocsDeTheme` now bounds
        // its count to 3 BY CONSTRUCTION, so NO assertion
        // on `lireBlocsDeTheme(...).length` can denounce an extra
        // `:root` any more — `lireBlocsBruts` is the ONLY count that still varies
        // with the number of physical occurrences.
        const bruts = lireBlocsBruts(deuxRacines);
        expect(bruts).toHaveLength(2);
        expect(bruts.map((b) => b.nom)).toEqual(['racine', 'racine']);
        // And the resulting merge does return ONE SINGLE block — the same
        // property as the two tests above, seen from the other end.
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
        // 🔴 The NATURAL defect of this check is to test inclusion in only
        // ONE direction. It would let through exactly the drift §7.4
        // exists to prevent, the light palette being declared TWICE
        // (spec §4.2).
        //
        // ⚠️ THIS TEST WAS WRITTEN WRONG A FIRST TIME, and mutation
        // revealed it: it added a token to `attribut-clair`, which brings down
        // the FIRST direction (`attribut ⊆ media`), not the second. Removing the
        // loop of the second direction left it GREEN. The only case that pins it
        // is a token present in `media-clair` and ABSENT from `attribut-clair`.
        const css = demonstration.replace('    --texte-fort: #10131a;\n}\n', '}\n');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual(['attribut-clair: --texte-fort missing']);
    });

    it('reports a light token WITHOUT a counterpart in the unconditional block', () => {
        // A light theme that overrides a token that does not exist in dark is
        // a typo, not an intention. It is inclusion ② of
        // `ecartsEntreBlocs`, and nothing else covered it.
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
    // 🔴 THE BLIND SPOT S3 CLOSES. Until now `ecartsEntreBlocs` compared
    // light ⇄ light (①) and light ⊆ racine (②), NEVER racine ⊆ light: a
    // colour declared at the root and forgotten in BOTH light blocks
    // passed without a word. S2 measured and filed it
    // (`journaux-design-s2/trou-7-4.log`): `--accent-survol` removed from both
    // light blocks returned `gaps: 0`, `exit=0`.
    //
    // ⚠️ ③ ONLY BITES ON THE ABSENCE FROM BOTH BLOCKS AT ONCE. A colour
    // present in only one is already caught by ①, and counting it twice
    // would say nothing more.

    /** Root with an intermediate colour — neither `#000` nor `#fff`. */
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
        // The exact mutation S2 played and filed.
        const css = withColour
            .replaceAll('        --accent-survol: #1d4ed8;\n', '')
            .replaceAll('    --accent-survol: #1d4ed8;\n', '');
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(css));
        expect(ecarts).toEqual([
            'light blocks: --accent-survol is a root colour without a light counterpart',
        ]);
    });

    it('reports NO gap for an OUT-OF-THEME token of the named list', () => {
        // `--voile-flottant` is a root colour absent from both
        // light blocks, and it is INTENDED: the six veils are laid over the
        // video, whose content follows no theme. Without this exemption the
        // closure would be red on a correct file — risk §11.
        expect(ecartsEntreBlocs(lireBlocsDeTheme(withColour))).toEqual([]);
    });

    it('reports NO gap for a root token that IS NOT a colour', () => {
        // 🔴 WITHOUT THIS PROPERTY the closure would return dozens of gaps
        // on the untouched tree — the typographic steps, spacing,
        // radii, durations and font stacks ONLY live in
        // `:root`, per §4.4 of the spec. It would be rejected wholesale.
        const ecarts = ecartsEntreBlocs(lireBlocsDeTheme(withColour));
        expect(ecarts.join(' ')).not.toContain('--e-3');
        expect(ecarts.join(' ')).not.toContain('--police-ui');
    });

    it('reports a NEW colour added to the root alone', () => {
        // 🔴 THE TEST THAT CATCHES VACUITY. An "is a colour" predicate
        // that returned `false` for everything would let this case pass, and the
        // whole closure would be a check that never bites.
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
        // The predicate decides on the VALUE, never on the name: a name
        // prefix is a convention a typo gets around.
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
        // 🔴 This test depends on `test: { css: true }` in `vite.config.ts`:
        // without it `?raw` returns the EMPTY string, `lireBlocsDeTheme` finds
        // no block, and the "zero gaps" assertion would pass while measuring
        // nothing. The assertion on the block count is what prevents it.
        // ⚠️ THREE LOGICAL blocks, NOT FOUR: `couleurs.css` and
        // `echelles.css` each declare an unconditional `:root {}`, and
        // it is the merge added by task 6 that brings them back to ONE SINGLE
        // "racine" block. 🔴 THIS COUNT IS NOW BOUNDED TO 3 BY
        // CONSTRUCTION — see the next test, which carries the count able
        // to denounce an extra `:root`.
        const blocs = lireBlocsDeTheme(tokensCss);
        expect(blocs).toHaveLength(3);
        expect(ecartsEntreBlocs(blocs)).toEqual([]);
    });

    it('EXACTLY FOUR PHYSICAL occurrences — the guard that one `:root` too many must turn red', () => {
        // 🔴 REVIEW FIX (round 1, August 25th, 2026). Measured: adding an extra
        // `:root { --e-4: 999rem; }` in `tokens/echelles.css` (a
        // real regression — every `--e-4` would go from 1rem to 999rem) blends
        // into the existing "racine" block WITHOUT MOVING THE LOGICAL
        // COUNT above, which stays at 3. `lireBlocsBruts`, which merges
        // nothing, is the ONLY count this regression still makes vary:
        // 4 today (root of `couleurs.css`, media-clair,
        // attribut-clair, root of `echelles.css`), 5 with the extra addition.
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
