/// <reference types="vite/client" />
/**
 * Tests of `conformer` — the product's ONLY SAFEGUARD against an unreadable
 * colour received from the server.
 *
 * 🔴 **§7.1 and §7.2 see NOTHING of this path, and it is STRUCTURAL** (E10 of the
 * plan): §7.2 is syntactic and says so itself; §7.1 compares a list
 * of pairs written by hand in `design/contraste.ts`, never a cartesian
 * product, and `client/outils/contraste.mjs` explicitly excludes colours
 * composed at run time. **These tests are the only judge, and their reds were
 * SEEN.**
 *
 * 🔴 **NO COLOUR IS WRITTEN AS A LITERAL HERE, AND IT IS NOT AN
 * ORNAMENT — it is a correction.** The first draft carried
 * twenty-one of them, and it turned §7.2 RED **in commit `46e3aa8`,
 * without me seeing it**: that check's exclusion is set AS NARROWLY AS POSSIBLE
 * (`client/src/design/*.test.ts` only), and these tests are not part of the
 * base layer. Widening it would belong to ⑥, closed, and sub-block A1 forbids itself
 * from writing in `client/src/design/` as in `client/outils/` (D-A1-14).
 *
 * ⚠️ **The easy route would have been to put the fixtures in a `.json`**, which
 * §7.2 does not scan — that is, satisfying a check by EMPTYING it, which
 * this repository refuses. Everything is therefore **READ FROM `tokens/couleurs.css`**
 * (`tokens.css` before the extraction of task 6, August 25th, 2026 — this file
 * only reads COLOURS, never a scale), which is strictly stronger
 * than a literal: "a check that has its own copy of the values
 * validates its copy" (spec ⑥ §7.1).
 */

import { describe, expect, it } from 'vitest';
import { lireBlocsDeTheme } from './design/tokens';
import tokensCss from './design/tokens/couleurs.css?raw';
import { conformer } from './accent';

const blocs = lireBlocsDeTheme(tokensCss);
const sombre = blocs.find((b) => b.nom === 'racine')!.tokens;

const jeton = (nom: string): string => {
    const v = sombre.get(nom);
    if (!v) throw new Error(`token missing from tokens/couleurs.css: ${nom}`);
    return v;
};

const FONDS = ['--fond-0', '--fond-1', '--fond-2'].map(jeton);
const ACCENT_DU_THEME = jeton('--accent');
/// `--bord`: **1.447 / 1.336 / 1.215** against the three dark backgrounds — below
/// the threshold of 3 on ALL THREE. Measured by `rapportDeContraste` itself on
/// August 21st, 2026, never estimated.
const ILLISIBLE = jeton('--bord');
/// `--succes`: **8.867 / 8.186 / 7.446** — readable on all three, and
/// DIFFERENT from `--accent`, which is essential: a fixture equal to the fallback
/// would not let us tell "rendered as is" from "refused".
const LISIBLE = jeton('--succes');

describe('conformer', () => {
    it('🔴 an UNREADABLE colour is REFUSED, and the theme takes over again', () => {
        // RED: the untouched tree before `conformer` existed; then, by
        // mutation, no longer judging readability.
        // 🔴 THIS IS THE RED THE SPEC REQUIRES (criterion ③).
        expect(conformer(ILLISIBLE, FONDS, ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
    });

    it('a READABLE colour is rendered as is', () => {
        // RED: always returning `accentDuTheme` ⟹ the whole mechanism would be
        // inert. WITHOUT THIS TEST, the previous one would be satisfied by a function that
        // refuses everything — that is, by a dead product.
        expect(conformer(LISIBLE, FONDS, ACCENT_DU_THEME)).toBe(LISIBLE);
    });

    it('a colour readable on TWO backgrounds out of three is REFUSED', () => {
        // RED: `.some()` instead of `.every()`.
        // The third background IS the candidate colour: the ratio there is
        // exactly 1.000, when the first two are 8.867 and 8.186.
        // The case is not artificial — a surface whose background is precisely
        // the application's tint is a real case.
        expect(conformer(LISIBLE, [FONDS[0], FONDS[1], LISIBLE], ACCENT_DU_THEME))
            .toBe(ACCENT_DU_THEME);
    });

    it('a NON-HEXADECIMAL form is REFUSED, without throwing', () => {
        // RED: removing the "shape" step ⟹ `rapportDeContraste` THROWS (E9).
        // ⚠️ The assertion is "returns `accentDuTheme`", NOT "does not throw": a
        // test that only expected the absence of an exception would be satisfied by
        // a `catch`, which this module forbids itself.
        //
        // The first two shapes are DERIVED from a real token, and that is what
        // makes them interesting: `luminanceRelative` ACCEPTS them (`#rgb`
        // and `#rrggbbaa` are in its list), and `conformer` refuses them all the
        // same — because the protocol says `#rrggbb`, and nothing else.
        const formes = [
            LISIBLE.slice(0, 4), // trois chiffres
            `${LISIBLE}00`, // huit chiffres
            'red',
            'var(--accent)',
            '',
            '   ',
            'not a colour',
        ];
        for (const forme of formes) {
            expect(conformer(forme, FONDS, ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
        }
    });

    it('an UPPERCASE casing is accepted after normalisation', () => {
        // RED: comparing without lowercasing ⟹ the SAME colour, written in
        // capitals, would be refused for a shape reason.
        expect(conformer(`  ${LISIBLE.toUpperCase()}  `, FONDS, ACCENT_DU_THEME)).toBe(LISIBLE);
    });

    it('the rendered colour is NEVER corrected', () => {
        // RED: lightening the refused colour instead of refusing it (D10
        // point 3: "lightening or darkening an application's colour
        // would produce a tint nobody chose").
        // Neither the accepted colour nor the fallback is altered by a single bit.
        expect(conformer(LISIBLE, FONDS, ACCENT_DU_THEME)).toBe(LISIBLE);
        expect(conformer(ILLISIBLE, FONDS, ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
    });

    it('a malformed background makes it REFUSE, it does not throw', () => {
        // RED: not checking the shape of the BACKGROUNDS ⟹ `rapportDeContraste`
        // throws on the background, and the data channel message kills the session.
        // The case is REAL: `getComputedStyle` returns the EMPTY STRING for an
        // absent token — so for any page whose base layer is not linked.
        expect(conformer(LISIBLE, [FONDS[0], '', FONDS[2]], ACCENT_DU_THEME))
            .toBe(ACCENT_DU_THEME);
    });

    it('an EMPTY list of backgrounds makes it REFUSE', () => {
        // RED: returning the candidate when there is nothing to judge ⟹ a page
        // whose tokens are not set yet would accept anything.
        expect(conformer(LISIBLE, [], ACCENT_DU_THEME)).toBe(ACCENT_DU_THEME);
    });
});
