import { describe, expect, it } from 'vitest';
import { lireBlocsDeTheme, tokensDeclares } from './tokens';
import couleursCss from './tokens/couleurs.css?raw';
import echellesCss from './tokens/echelles.css?raw';

/**
 * THE LIST OF CASES `galerie.ts` IS SUPPOSED TO SHOW — never ITS RENDERING.
 *
 * 🔴 THIS FILE DOES NOT IMPORT `./galerie` AND WILL NEVER RUN IT.
 * `galerie.ts` touches `document.documentElement` (its very first
 * executable line), then `document.getElementById` and `getComputedStyle`, WITH
 * NO INJECTED DEPENDENCY — the opposite of the convention this repository
 * applies elsewhere to make a module that touches the DOM testable
 * (`client/src/accent-dom.ts`, `client/src/presse-papier-dom.ts`, and
 * `installThemeSelectorInDOM` in `./selecteur-theme.ts`, which each separate
 * a PURE function, with injected dependencies, from a SEAM — the only one to
 * touch the real `document`/`window`/`localStorage`). `galerie.ts` does not have
 * this seam, and `client/` has NEITHER jsdom NOR happy-dom (`client/package.json`
 * only carries `typescript`, `vite`, `vitest` — the same finding
 * `selecteur-theme.ts` makes for itself). Importing it here would crash the
 * module ON LOAD (`document is not defined`), before the slightest
 * assertion. Making this module testable belongs to a refactor this
 * task has no mandate to do; this file says so rather than forcing a
 * home-made DOM double that would prove nothing more than the text it
 * had been given.
 *
 * ⚠️ WHAT IT CAN ESTABLISH INSTEAD. `galerie.ts` declares, in its own
 * header: "the list of tokens is not written here: it is PARSED from
 * `tokens/couleurs.css` and `tokens/echelles.css`". The contract between the
 * module and `design.html` therefore lies entirely in WHAT THESE TWO
 * FILES DECLARE — freezing this contract means freezing the eight sections that
 * `rendre()` builds (read again in `galerie.ts`, never run here): the
 * explicit list of the 14 theme colours, and seven families by prefix
 * (`--video-`/`--voile-`, `--t-`, `--lh-`, `--e-`, `--r-`, `--trait`,
 * `--police-`). Each test compares an EXPECTED list, written here HARDCODED and
 * INDEPENDENT of `galerie.ts` (never re-imported from it — otherwise a
 * name removed BOTH from `galerie.ts` and from its source would stay invisible,
 * exactly the pattern of "a copy that validates its copy" this repository
 * forbids itself), to the REAL text of the two token files.
 *
 * 🔴 THE RED THIS FILE CAN PRODUCE: removing a token from
 * `tokens/couleurs.css` or `tokens/echelles.css` brings down the test of its
 * section — the primitive has disappeared from what the gallery is supposed to show,
 * and the test asks for it.
 *
 * ⚠️ WHAT IT CANNOT ESTABLISH: that `galerie.ts` still honours this
 * contract today — if its selection code diverged from the list
 * below WITHOUT THE TOKENS THEMSELVES MOVING, nothing here would
 * see it, since the module cannot be run. And nothing here carries any
 * VISUAL JUDGEMENT: that the palette is sober, that `#7aa2f7` is the right
 * blue, that the ratio 1.2 is the right one — the eight human judgements the
 * spec ⑥ §8 names, and that no sub-block has made measurable. That is
 * batch 4, not this one.
 */

const tokensCss = `${couleursCss}\n${echellesCss}`;
const TOKENS = tokensDeclares(tokensCss);

/** The declared names that start with one of the prefixes, sorted. */
function parPrefixe(...prefixes: string[]): string[] {
    return [...TOKENS].filter((n) => prefixes.some((p) => n.startsWith(p))).sort();
}

describe("galerie.ts — the list of cases it is meant to show (see the header)", () => {
    it('« Colours »: the fourteen tokens redeclared by the THREE theme blocks', () => {
        // STRUCTURAL definition, not a copy of `galerie.ts::COULEURS`:
        // a "theme" token is one that ALL three blocks redeclare
        // — that is the rule the header of `tokens/couleurs.css` writes
        // itself ("the LIGHT palette is written twice"). The tokens
        // outside the theme (`--voile-*`, `--sur-voile`, `--accent-fenetre`) and the
        // scales only live in the `racine` block: they never enter
        // this intersection.
        const blocs = lireBlocsDeTheme(tokensCss);
        const parBloc = new Map(blocs.map((b) => [b.nom, new Set(b.tokens.keys())]));
        const racine = parBloc.get('racine');
        const mediaClair = parBloc.get('media-clair');
        const attributClair = parBloc.get('attribut-clair');
        if (!racine || !mediaClair || !attributClair) {
            throw new Error('tokens/couleurs.css no longer carries the three expected theme blocks');
        }
        const themes = [...racine].filter((n) => mediaClair.has(n) && attributClair.has(n)).sort();

        const ATTENDUS = [
            '--accent',
            '--accent-survol',
            '--alerte',
            '--bord',
            '--bord-fort',
            '--danger',
            '--fond-0',
            '--fond-1',
            '--fond-2',
            '--succes',
            '--sur-accent',
            '--texte',
            '--texte-faible',
            '--texte-fort',
        ].sort();
        expect(themes, 'the 14 theme colours changed name or number').toEqual(ATTENDUS);
    });

    it('« Veils »: six out-of-theme tokens, `--video-` and `--voile-`', () => {
        const ATTENDUS = [
            '--video-letterbox',
            '--voile-bouton',
            '--voile-bouton-survol',
            '--voile-flottant',
            '--voile-micro-actif',
            '--voile-micro-refuse',
        ].sort();
        expect(parPrefixe('--video-', '--voile-'), 'the « veils » section of design.html').toEqual(
            ATTENDUS,
        );
    });

    it('« Typography »: seven steps, `--t-`', () => {
        const ATTENDUS = ['--t-2xl', '--t-3xl', '--t-l', '--t-m', '--t-s', '--t-xl', '--t-xs'].sort();
        expect(parPrefixe('--t-'), 'the « typography » section of design.html').toEqual(ATTENDUS);
    });

    it('« Line heights »: three, `--lh-`', () => {
        const ATTENDUS = ['--lh-large', '--lh-normal', '--lh-serre'].sort();
        expect(parPrefixe('--lh-'), 'the « line heights » section of design.html').toEqual(ATTENDUS);
    });

    it('« Spacing »: eight steps, `--e-`', () => {
        const ATTENDUS = [
            '--e-1',
            '--e-2',
            '--e-3',
            '--e-4',
            '--e-5',
            '--e-6',
            '--e-7',
            '--e-8',
        ].sort();
        expect(parPrefixe('--e-'), 'the « spacing » section of design.html').toEqual(ATTENDUS);
    });

    it('« Radii »: four, `--r-`', () => {
        const ATTENDUS = ['--r-1', '--r-2', '--r-3', '--r-plein'].sort();
        expect(parPrefixe('--r-'), 'the « radii » section of design.html').toEqual(ATTENDUS);
    });

    it('« Strokes »: two thicknesses, prefix `--trait` (no dash: also covers `--trait-focus`)', () => {
        const ATTENDUS = ['--trait', '--trait-focus'].sort();
        expect(parPrefixe('--trait'), 'the « strokes » section of design.html').toEqual(ATTENDUS);
    });

    it('« Fonts »: two system stacks, `--police-`', () => {
        const ATTENDUS = ['--police-mono', '--police-ui'].sort();
        expect(parPrefixe('--police-'), 'the « fonts » section of design.html').toEqual(ATTENDUS);
    });
});
