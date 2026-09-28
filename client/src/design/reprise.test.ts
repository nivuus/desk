/// <reference types="vite/client" />
import { describe, expect, it } from 'vitest';
import { lireBlocsDeTheme } from './tokens';
import couleursCss from './tokens/couleurs.css?raw';
import echellesCss from './tokens/echelles.css?raw';
import baseCss from './base.css?raw';

// 🔴 SINCE THE EXTRACTION OF TASK 6 (August 25th, 2026): the colours and the
// scales live in two separate files, each with its own unconditional `:root {}`.
// This test NEEDS both in the SAME "racine" block
// — it compares `--fond-0` (colour) AND `--e-3` (scale) on `racine` — so
// it is HERE, and not in an isolated test, that the merge of `lireBlocsDeTheme`
// is exercised on REAL content rather than on a demonstration.
const tokensCss = `${couleursCss}\n${echellesCss}`;

/**
 * THE COMPARISON THAT PROVES THE CARRY-OVER — not the claim.
 *
 * ⚠️ WHAT THESE ASSERTIONS ESTABLISH: equality of the DECLARED values, at a
 * 16 px root. NOT equality of the rendered pixels — that would require a rendering
 * engine, and spec §7.8 rules out image comparison, its first reason
 * being disqualifying: system fonts render differently from one machine
 * to the next, so a reference taken on one workstation would fail on the next
 * for a reason that is not a defect.
 *
 * The reference values below are those of `client/src/style.css`
 * BEFORE this base layer, at commit `7e9b438`: that is why they are written as
 * literals here, and it is the only reason check §7.2
 * excludes `client/src/design/*.test.ts` — otherwise this task would be
 * impossible.
 */

const blocs = lireBlocsDeTheme(tokensCss);
const racine = blocs.find((b) => b.nom === 'racine');

describe('the colours already in place are taken over character for character', () => {
    it('`--fond-0` dark is exactly the former `--surface`', () => {
        expect(racine?.tokens.get('--fond-0')).toBe('#0b0d10');
    });

    it('`--texte-fort` dark is exactly the former `--text`', () => {
        expect(racine?.tokens.get('--texte-fort')).toBe('#e6e8eb');
    });
});

describe('the lengths taken over render the SAME number of pixels', () => {
    it('each step used renders, at a 16 px root, the previous literal', () => {
        // 🔴 Setting `--e-3: 0.8rem` would give 12.8 px: plausible, and wrong. It is
        // exactly the kind of error none of the nine checks catches.
        // ❌ "since none measures a length": no longer true since
        // sub-block S4 — §7.10 measures one. But `tokens.css` is its
        // exception ③, NAMED: its literals ARE the scale, and a check
        // that refused them would refuse the scale itself. It is this test,
        // and it alone, that holds the value of the steps.
        const attendus: Array<[string, string, number]> = [
            ['--e-2', '0.5rem', 8],
            ['--e-3', '0.75rem', 12],
            ['--e-8', '4rem', 64],
            ['--t-s', '0.75rem', 12],
            ['--t-m', '0.875rem', 14],
        ];
        for (const [token, litteral, pixels] of attendus) {
            const value = racine?.tokens.get(token);
            expect(value, token).toBe(litteral);
            expect(Number.parseFloat(litteral) * 16, `${token} en pixels`).toBe(pixels);
        }
    });

    it('the steps without a `rem` unit are worth the previous literal, as is', () => {
        expect(racine?.tokens.get('--r-2')).toBe('6px');
        expect(racine?.tokens.get('--duree-2')).toBe('300ms');
        expect(racine?.tokens.get('--lh-normal')).toBe('1.5');
    });
});

describe('the six out-of-theme veils take the literals over verbatim', () => {
    it('each one is worth the exact value it replaces', () => {
        const attendus: Record<string, string> = {
            '--video-letterbox': '#000',
            '--voile-flottant': 'rgb(0 0 0 / 0.72)',
            '--voile-bouton': 'rgb(0 0 0 / 0.55)',
            '--voile-bouton-survol': 'rgb(0 0 0 / 0.75)',
            '--voile-micro-actif': 'rgb(220 38 38 / 0.85)',
            '--voile-micro-refuse': 'rgb(120 53 15 / 0.85)',
        };
        for (const [token, value] of Object.entries(attendus)) {
            expect(racine?.tokens.get(token), token).toBe(value);
        }
    });
});

describe('the precondition of everything above: the root is FREE', () => {
    it('no rule whose selector contains `html` sets a font size', () => {
        // 🔴 IT IS THE ONLY MECHANICAL LINK BETWEEN `base.css` AND THE FIGURES
        // ABOVE. `--e-3` is only 12 px if `1rem` is 16 px, that is only
        // if the root is not forced. With the `font: 14px/1.5` that
        // `style.css` set on `html, body` before this base layer, `0.75rem`
        // would be 10.5 px — and NONE of the nine checks would say so.
        const regles = [...baseCss.matchAll(/([^{}]+)\{([^{}]*)\}/g)];
        const fautives = regles
            .filter(([, selecteur]) => /(^|[\s,>+~])html\b/.test(selecteur))
            .filter(([, , corps]) => /(^|[;\s])font-size\s*:|(^|[;\s])font\s*:/.test(corps))
            .map(([, selecteur]) => selecteur.trim());
        expect(fautives).toEqual([]);
        // And the check can fail: it MUST see the `html, body` rule.
        expect(regles.some(([, s]) => /(^|[\s,>+~])html\b/.test(s))).toBe(true);
    });
});
