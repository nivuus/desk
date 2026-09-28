import { describe, expect, it } from 'vitest';
import { lireBlocsDeTheme } from './tokens';
import { PAIRES, evaluer, luminanceRelative, rapportDeContraste } from './contraste';

/**
 * 🔴 A CONTRAST TEST WRITTEN WITH THE PRODUCT'S COLOURS VALIDATES THE PRODUCT
 * AGAINST ITSELF. The first four tests therefore use vectors whose
 * value is set by the WCAG 2.1 standard itself, not by our tokens.
 * That the REAL palette meets the thresholds is exercised elsewhere, by
 * `client/outils/contraste.mjs`, which reads `tokens.css`.
 */
describe('rapportDeContraste — vectors external to our palette', () => {
    it('returns 21 on black against white: the absolute maximum of the scale', () => {
        expect(rapportDeContraste('#000000', '#ffffff')).toBeCloseTo(21, 5);
    });

    it('returns 1 on a colour against itself: the absolute minimum', () => {
        expect(rapportDeContraste('#ffffff', '#ffffff')).toBeCloseTo(1, 10);
    });

    it('is symmetric — the formula orders its two terms', () => {
        // `(L + 0.05) / (l + 0.05)` with L ≥ l. Forgetting the order breaks here.
        for (const [a, b] of [
            ['#000000', '#ffffff'],
            ['#7aa2f7', '#0b0d10'],
            ['#c02b2b', '#f6f7f9'],
        ]) {
            expect(rapportDeContraste(a, b)).toBeCloseTo(rapportDeContraste(b, a), 10);
        }
    });

    it('applies the gamma correction, and not a linear average', () => {
        // 🔴 THIS IS THE VECTOR THAT DISCRIMINATES. `#808080` is halfway along the
        // channels, so a linear average would return 0.5. The true relative
        // luminance is ≈ 0.2159. The mistake is classic and returns
        // PLAUSIBLE but wrong ratios on all intermediate
        // colours — that is, on the 53 real pairs, where
        // black/white returns 21 in both cases.
        expect(luminanceRelative('#808080')).toBeCloseTo(0.2159, 4);
    });
});

describe('PAIRES', () => {
    it('counts 53 of them, and they are DECLARED pairs', () => {
        // Seven inks × three backgrounds at threshold 4.5, plus `--bord-fort` on the
        // three backgrounds at threshold 3, plus `--sur-accent` on `--accent` at
        // threshold 4.5, plus `--sur-accent` on `--accent-survol` at threshold 4.5
        // — all × 2 themes. A cartesian product would give many
        // more, and would include `--bord`.
        //
        // ⚠️ THIS COUNT IS WHAT KEEPS `PAIRES` FROM SHRINKING SILENTLY:
        // a typo that made a push disappear in
        // `pairesDuTheme` would leave `contraste.mjs` green while measuring less.
        //
        // ⚠️ THE 53RD IS IN NEITHER OF THE TWO THEMES, and that is what its
        // label says: `--sur-voile` on `--video-letterbox`, two tokens OUTSIDE
        // THE THEME, so a pair that holds identically in light and dark.
        // Counting it twice would measure the same thing twice (sub-block
        // S4, task 4).
        expect(PAIRES).toHaveLength(53);
        expect(new Set(PAIRES.map((p) => p.theme))).toEqual(
            new Set(['sombre', 'clair', 'out of theme']),
        );
    });

    it("NEVER carries `--bord`, and that is a decision", () => {
        // `--bord` gives 1.45 (dark) and 1.40 (light) on `--fond-0`: it is
        // reserved for purely decorative separators, which WCAG 1.4.11 exempts.
        // Without this test, a well-meaning successor would add it and make
        // the check red forever, hence ripe for loosening.
        // ⚠️ No command can, however, check that `--bord` was not used
        // where `--bord-fort` was needed: it is a review rule.
        expect(PAIRES.filter((p) => p.encre === '--bord' || p.fond === '--bord')).toEqual([]);
    });
});

/** Synthetic palette: no product value is copied here. */
function palette(couleurs: Record<string, [string, string]>): string {
    const sombre = Object.entries(couleurs).map(([n, [s]]) => `${n}: ${s};`).join('\n    ');
    const clair = Object.entries(couleurs).map(([n, [, c]]) => `${n}: ${c};`).join('\n    ');
    return `:root { color-scheme: dark; ${sombre} }
@media (prefers-color-scheme: light) { :root:not([data-theme="sombre"]) { color-scheme: light; ${clair} } }
:root[data-theme="clair"] { color-scheme: light; ${clair} }`;
}

// ⚠️ `--accent-survol` IS HERE BECAUSE A TOKEN THAT CANNOT BE FOUND IS A FAILURE, and
// not a silently skipped pair (see the header of `evaluer`): adding it
// to `PAIRES` without adding it to this synthetic palette brings down the test
// below on two failures of ratio 0. That is the intended behaviour, and it was
// observed RED before this line.
const ALL = [
    '--fond-0', '--fond-1', '--fond-2', '--bord-fort', '--texte-fort', '--texte',
    '--texte-faible', '--accent', '--accent-survol', '--sur-accent', '--succes',
    '--alerte', '--danger',
    // The 53rd pair (S4, task 4). `--video-letterbox` plays the role of a
    // BACKGROUND here, `--sur-voile` that of an ink: that is what they are in the
    // product — the ink of the five elements of the session window, on the
    // band `object-fit: contain` leaves.
    '--video-letterbox', '--sur-voile',
];
/** The tokens the synthetic palette treats as BACKGROUNDS. */
const FONDS_SYNTHETIQUES = (n: string) =>
    n.startsWith('--fond') || n === '--sur-accent' || n === '--video-letterbox';

describe('evaluer', () => {
    it('returns no failure on a compliant palette', () => {
        const conforme = Object.fromEntries(
            ALL.map((n) => [
                n,
                FONDS_SYNTHETIQUES(n)
                    ? (['#ffffff', '#000000'] as [string, string])
                    : (['#000000', '#ffffff'] as [string, string]),
            ]),
        );
        const result = evaluer(lireBlocsDeTheme(palette(conforme)));
        expect(result.echecs).toEqual([]);
        expect(result.verifiees).toBe(53);
        expect(result.minimum).toBeCloseTo(21, 5);
    });

    it("NAMES the theme, the ink, the background and the ratio of each failure", () => {
        // A boolean would not say what to fix.
        const fautif = Object.fromEntries(
            ALL.map((n) => [
                n,
                FONDS_SYNTHETIQUES(n)
                    ? (['#ffffff', '#000000'] as [string, string])
                    : n === '--texte-faible'
                      ? (['#eeeeee', '#111111'] as [string, string]) // dark: light on light
                      : (['#000000', '#ffffff'] as [string, string]),
            ]),
        );
        const result = evaluer(lireBlocsDeTheme(palette(fautif)));
        expect(result.echecs.length).toBeGreaterThan(0);
        const premier = result.echecs[0];
        expect(premier.paire.theme).toBe('sombre');
        expect(premier.paire.encre).toBe('--texte-faible');
        expect(premier.paire.fond).toMatch(/^--fond-[012]$/);
        expect(premier.rapport).toBeLessThan(premier.paire.seuil);
    });
});
