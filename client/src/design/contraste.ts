/**
 * WCAG 2.1 contrast — PURE: no DOM, no `fs`, no path.
 *
 * ⚠️ ERASABLE TypeScript, this module being imported by a `.mjs`: no
 * `enum`, no `namespace`. See the header of `tokens.ts` for the measurement.
 *
 * The values ALWAYS come from `tokens.css`, parsed by `tokens.ts`. This
 * module knows NO colour: it knows token NAMES. That is the
 * design point of §7.1 — "a check that has its own copy of the
 * values validates its copy".
 */

import type { BlocDeTheme } from './tokens';

export interface Paire {
    theme: string;
    encre: string;
    fond: string;
    seuil: number;
}

export interface Echec {
    paire: Paire;
    rapport: number;
}

/** A linearised sRGB channel. It is here that gamma correction lives. */
function canalLineaire(octet: number): number {
    const c = octet / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/**
 * WCAG 2.1 relative luminance.
 *
 * 🔴 THE CLASSIC MISTAKE IS TO AVERAGE THE CHANNELS LINEARLY. It returns
 * plausible and wrong ratios: `#808080` would be 0.5 instead of ≈ 0.2159, and
 * black/white would return 21 in both cases — so the discriminating vector
 * is an INTERMEDIATE colour, never the extremes.
 */
export function luminanceRelative(couleur: string): number {
    const brut = couleur.trim().replace(/^#/, '');
    const hex =
        brut.length === 3 || brut.length === 4
            ? brut.slice(0, 3).split('').map((c) => c + c).join('')
            : brut.slice(0, 6);
    if (!/^[0-9a-fA-F]{6}$/.test(hex)) {
        throw new Error(`unrecognised colour: « ${couleur} » (expected #rgb or #rrggbb)`);
    }
    const [r, v, b] = [0, 2, 4].map((i) => canalLineaire(parseInt(hex.slice(i, i + 2), 16)));
    return 0.2126 * r + 0.7152 * v + 0.0722 * b;
}

/** `(L + 0.05) / (l + 0.05)`, with `L ≥ l` — hence symmetric. */
export function rapportDeContraste(a: string, b: string): number {
    const [x, y] = [luminanceRelative(a), luminanceRelative(b)];
    const [haut, bas] = x >= y ? [x, y] : [y, x];
    return (haut + 0.05) / (bas + 0.05);
}

const FONDS = ['--fond-0', '--fond-1', '--fond-2'];
/** Seven inks. Neither `--bord` (decorative, exempted) nor `--sur-accent` (on `--accent`). */
const ENCRES = [
    '--texte-fort',
    '--texte',
    '--texte-faible',
    '--accent',
    '--succes',
    '--alerte',
    '--danger',
];

const SEUIL_TEXTE = 4.5; // WCAG 1.4.3 AA
const SEUIL_COMPOSANT = 3; // WCAG 1.4.11

function pairesDuTheme(theme: string): Paire[] {
    const paires: Paire[] = [];
    for (const encre of ENCRES) {
        for (const fond of FONDS) paires.push({ theme, encre, fond, seuil: SEUIL_TEXTE });
    }
    for (const fond of FONDS) {
        paires.push({ theme, encre: '--bord-fort', fond, seuil: SEUIL_COMPOSANT });
    }
    paires.push({ theme, encre: '--sur-accent', fond: '--accent', seuil: SEUIL_TEXTE });
    // The HOVER of the primary button (S2): the ink does not change, the background does.
    // Without this pair, the product's most frequent state would be the only one whose
    // contrast nothing measures — that is the reason why
    // `--accent-survol` is a TOKEN and not a `color-mix()` or a `filter`.
    paires.push({ theme, encre: '--sur-accent', fond: '--accent-survol', seuil: SEUIL_TEXTE });
    return paires;
}

/**
 * THE 53rd PAIR — `--sur-voile` on `--video-letterbox` (sub-block S4, task 4).
 *
 * 🔴 IT REPAIRS A REAL DEFECT, AND NONE OF THE NINE CHECKS COULD
 * SEE IT.
 *
 * ❌ THIS SENTENCE SAID "EIGHT", AND IT WAS ALREADY WRONG WHEN IT WAS
 * WRITTEN — it is S4's cross-cutting review that noted it, in the THREE places
 * where task 4 put it (here, `tokens.css` and `style.css`). The ninth
 * check, §7.10, was born in task 2 (`ee56e1e`), which `git merge-base
 * --is-ancestor` establishes precedes task 4 (`fb629ea`). A task
 * therefore described the suite of checks as it was BEFORE the task that
 * had already changed it, two commits earlier, IN THE SAME BRANCH. It is the
 * exact form the cross-cutting review exists to catch: each task
 * was correct about what it saw. `base.css` sets `color: var(--texte-fort)` on `body`; in the light theme
 * `--texte-fort` is `#10131a`, an almost black ink; and the veils are
 * OFF-THEME, hence black in both. Under the light theme, the five elements
 * of the session window therefore wrote almost-black on an almost-black
 * veil. It is not a regression of the original product: it is a side
 * effect of S1, which gave a light theme to a surface that had none.
 *
 * ⚠️ WHY NO CHECK SAW IT, and the argument is only HALF right:
 * `tokens.css` places the veils outside the pairs, "their readability depends on the
 * video underneath, which is not knowable". That is true OF THE VEIL; it
 * is not true of THE INK placed on it, which, for its part, is perfectly
 * knowable as soon as the background is.
 *
 * ⚠️ AND THIS PAIR ONLY MEASURES ONE REGION: the band
 * `object-fit: contain` leaves around the image (`#remote { background:
 * var(--video-letterbox) }`), the ONLY one where the background under the ink is known.
 * Above the image, the background stays unknowable, and the reservation of §11 of
 * the spec stands whole — "`--voile-flottant` is not enough on a very
 * light video, not measured".
 *
 * ⚠️ IT IS IN NEITHER OF THE TWO THEMES, hence its label: its two tokens
 * are off-theme, so it holds identically in light and dark. Counting
 * it per theme would measure the same thing twice. `evaluer` resolves it through
 * its fallback to the `racine` block, where both tokens live.
 *
 * ⚠️ THAT `#e6e8eb` IS THE RIGHT INK ON A VEIL IS A HUMAN JUDGEMENT
 * (spec §8): only its readability ON THE BLACK BAND is measured here.
 */
const PAIRE_HORS_THEME: Paire = {
    theme: 'out of theme',
    encre: '--sur-voile',
    fond: '--video-letterbox',
    seuil: SEUIL_TEXTE,
};

/**
 * The 53 DECLARED pairs — never a Cartesian product.
 * (50 in S1; S2 adds two, `--sur-accent` on `--accent-survol`; S4
 * adds one, off-theme, above.)
 *
 * 25 per theme: 7 inks × 3 backgrounds at threshold 4.5; `--bord-fort` on the 3
 * backgrounds at threshold 3; `--sur-accent` on `--accent` at threshold 4.5.
 *
 * ⚠️ `--bord` IS ABSENT, AND IT IS A DECISION, not an oversight: it returns 1.45
 * (dark) and 1.40 (light) on `--fond-0`, and it is reserved for PURELY
 * decorative separators, which WCAG 1.4.11 explicitly exempts. As soon as a
 * border carries information — field outline, state of a control —
 * it is `--bord-fort` that applies, and that one is measured.
 *
 * ⚠️ THE COROLLARY: no command can check that `--bord` was not used
 * where `--bord-fort` was needed. It is a REVIEW RULE, and the
 * spec §8 names it as such.
 */
export const PAIRES: readonly Paire[] = [
    ...pairesDuTheme('sombre'),
    ...pairesDuTheme('clair'),
    PAIRE_HORS_THEME,
];

/** The block carrying a theme's palette. The two light blocks are equal (§7.4). */
function blocDuTheme(blocs: BlocDeTheme[], theme: string): BlocDeTheme | undefined {
    if (theme === 'sombre') return blocs.find((b) => b.nom === 'racine');
    return (
        blocs.find((b) => b.nom === 'attribut-clair') ?? blocs.find((b) => b.nom === 'media-clair')
    );
}

/**
 * Evaluates the 53 pairs on the parsed blocks.
 *
 * ⚠️ A TOKEN NOT FOUND IS A FAILURE, never a silently skipped
 * pair: otherwise, a typo in a token name would LOWER
 * the number of pairs checked without any failure coming up, and the check
 * would go green while measuring less. Ratio 0 makes it visible in the
 * same report as the other failures.
 */
export function evaluer(blocs: BlocDeTheme[]): {
    verifiees: number;
    echecs: Echec[];
    minimum: number;
} {
    const echecs: Echec[] = [];
    let minimum = Infinity;

    for (const paire of PAIRES) {
        const bloc = blocDuTheme(blocs, paire.theme);
        // The dark theme's `--fond-0` may only be declared in `racine`:
        // we fall back to that block when the light block does not override the token.
        const racine = blocs.find((b) => b.nom === 'racine');
        const encre = bloc?.tokens.get(paire.encre) ?? racine?.tokens.get(paire.encre);
        const fond = bloc?.tokens.get(paire.fond) ?? racine?.tokens.get(paire.fond);

        if (encre === undefined || fond === undefined) {
            echecs.push({ paire, rapport: 0 });
            minimum = 0;
            continue;
        }
        const rapport = rapportDeContraste(encre, fond);
        if (rapport < minimum) minimum = rapport;
        if (rapport < paire.seuil) echecs.push({ paire, rapport });
    }

    return { verifiees: PAIRES.length, echecs, minimum };
}
