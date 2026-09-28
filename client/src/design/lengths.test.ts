import { describe, expect, it } from 'vitest';
import { declarationsDe, sansCommentaires } from './css';

/**
 * ═══════════════════════════════════════════════════════════════════════════
 * CHECK §7.10 — NO LENGTH OUTSIDE TOKENS IN A SURFACE SHEET.
 *
 * 🔴 IT IS THE FIRST CHECK OF SUB-PROJECT ⑥ THAT MEASURES A LENGTH, and
 * that is its raison d'être. Until now, "none of the eight checks
 * measures a length" was written in five places of the repository in that exact
 * wording, and more in other phrasings: the clause "no length
 * outside the scale" of §8 of the spec was a STATEMENT DEBT that nothing could
 * either hold or refute. It becomes a command.
 *
 * 🔴 IT WAS BORN RED ON THE UNTOUCHED TREE — EIGHT OCCURRENCES FOR SIX VALUES —,
 * AND THAT IS ITS PROOF OF REACHABILITY, like assertion ② A of §7.9 in S3.
 * The six: `6px` ×2 (the two banners), `18px` ×2 (the two corner buttons),
 * `0.02em` (the kerning of `#stats`), `72rem`, `18rem` and `26rem` (the three
 * container measures). Tasks 5 to 8 of S4 bring them all down.
 *
 * ⚠️ TWO NUMBERS, NEVER ONE, AND THEY ARE TRUE OF DIFFERENT THINGS.
 * Sub-block S3 published SIX by counting DISTINCT VALUES; a check
 * counts OCCURRENCES, because it cannot deduplicate without deciding that
 * two `6px` written in two places are the same. It is divergence D5 of S2
 * replayed on another quantity — "neither ten nor seventeen is enough without saying
 * which one is counted". Both are therefore printed, success included: "a
 * drift check whose value is never read only serves to pass"
 * (`poids-css.mjs`).
 *
 * ── THE BOUNDARY WITH G4, WRITTEN FROM BOTH SIDES ─────────────────────────
 * G4 (`primitives.test.ts`) guards `client/src/design/primitives/` and its four
 * families. §7.10 guards the SURFACE SHEETS. Neither doubles
 * the other, and the scope below is DERIVED — never enumerated: all the
 * `*.css` of `client/src/` OUTSIDE `client/src/design/`. A new surface
 * sheet — `client/src/session/*.css`, which task 9 creates — therefore enters this
 * check without a line of this file changing, and a copied list
 * cannot diverge from what it describes.
 *
 * ⚠️ WHAT IT DOES NOT SAY, AND IT IS EXACTLY THE LIMIT OF G4: *that the RIGHT
 * token was chosen.* `padding: var(--e-8)` on a banner would be green and
 * absurd. The right use remains a review rule, like that of `--bord`
 * versus `--bord-fort` (spec §4.5, §8).
 *
 * ⚠️ NOR DOES IT SEE A LENGTH COMPUTED AT RUN TIME —
 * `el.style.padding = …` in a `.ts`. It is the same blind spot §7.9
 * declares for classes, and the same countermeasure: the convention is to write
 * lengths in the CSS.
 *
 * ⚠️ A FLAW OF THE PLAN, REPORTED RATHER THAN COPIED. The S4 plan prescribes
 * playing the reachability red "by emptying `client/src/style.css`". MEASURED:
 * that is NOT enough — the scope being derived over THREE sheets, emptying
 * only `style.css` leaves `shell.css` and `connexion.css` carrying their
 * declarations and 3 occurrences outside tokens, and the reachability assertion
 * stays GREEN, rightly so. The red that counts empties ALL THREE: the absence
 * assertion then turns green with `0 occurrence(s)` — the exact state it
 * exists to denounce — and only reachability falls. The plan's prescription
 * assumed a single file; it is wrong for a derived scope.
 *
 * 🔴 THIS FILE ONLY READS A NON-EMPTY TEXT THANKS TO `test: { css: true }` of
 * `client/vite.config.ts`. Without that line, Vitest short-circuits CSS
 * files — the `?raw` query included — and the sheets would be the EMPTY string:
 * the absence assertion would turn green WHILE MEASURING NOTHING. That is also
 * why there is deliberately no `client/vitest.config.ts`, which would take
 * precedence over the Vite configuration without a word.
 *
 * 🔴 A NEW TRAP, MEASURED ON AUGUST 20TH, 2026, AND IT MAKES A RED INDISTINGUISHABLE
 * FROM A GOOD ONE. This test must be run FROM `client/`. Run from the root
 * of the repository (`npm --prefix client exec -- vitest run …`), the Vite root changes,
 * `client/vite.config.ts` is no longer the configuration used, and the CSS is
 * short-circuited SILENTLY: the three sheets are found by the glob —
 * "surface sheets: 3" is printed — but are the EMPTY string.
 * The absence assertion turns green, and it is reachability that falls.
 * Two reds of task 2 were first played that way: they went red
 * for that reason, never for the one we thought we were measuring. "A red
 * that goes red for the wrong reason is indistinguishable from a good one if one only
 * reads its `exit=1`" — §9 of the plan, paid for the very day it wrote it.
 * ⚠️ The reachability assertion, for its part, DID CATCH THIS CASE. That is its second
 * value, not planned: it also guards the HARNESS, not only the tree.
 * ⚠️ `npm --prefix client test` DOES NOT FALL INTO THIS TRAP — the `test` script
 * of `package.json` runs with `client/` as cwd. It is `exec` that
 * sets it, and `npx vitest` run from the root too.
 * ═══════════════════════════════════════════════════════════════════════════
 */

/**
 * The SURFACE sheets, DERIVED and not enumerated — see the header.
 * `client/src/design/` is excluded: G4 takes care of it, and `tokens.css` lives there, whose
 * literals ARE the scale.
 */
const FEUILLES = import.meta.glob<string>(['../**/*.css', '!../design/**'], {
    query: '?raw',
    import: 'default',
    eager: true,
});

/**
 * The units that make a length. It is G4's list, word for word, and the
 * sharing is deliberate: two guards measuring the same thing with two
 * lists would diverge without any command saying so.
 * ⚠️ `ms` and `s` are DURATIONS, not lengths, and they are here for the
 * same reason — durations have their scale (`--duree-1`, `--duree-2`), and a
 * literal duration is the same drift under another name. The title of the check
 * says "length" because that is the word of spec §8; the real scope is
 * "any dimensioned value".
 */
const UNITES = /(\d+(?:\.\d+)?)(px|rem|em|ms|s|pt|ch|vw|vh|dvw|dvh|vmin|vmax)\b/g;

/**
 * THE THREE EXCEPTIONS, CLOSED, EACH WITH ITS REASON.
 *
 * ① WINDOW FILLS — `100vw`, `100vh`, `100dvh`. It is the counting
 *    rule `style.css` has applied since S1 and that the S3 log
 *    stated BEFORE counting: "a window fill is not an
 *    out-of-scale value". No scale claims to cover "the whole
 *    window", and a token worth `100vh` would only be an alias.
 * ② ZERO — `0`, `0px`. No scale has a null step, and the fallback of the
 *    `env(titlebar-area-*)` of the Window Controls Overlay carries one by
 *    construction (task 10).
 * ③ `tokens.css` — the single source; its literals ARE the scale. It is
 *    outside the derived scope above anyway, and saying so here keeps anyone
 *    from bringing it back in "for completeness".
 */
function horsExceptions(value: string): string {
    return value
        .replace(/var\(\s*--[a-z0-9-]+\s*\)/gi, ' ')
        .replace(/\b100(vw|vh|dvw|dvh)\b/g, ' ')
        .replace(/(^|[\s(,])0(px)?(?=$|[\s),;])/g, '$1 ');
}

interface Occurrence {
    file: string;
    propriete: string;
    value: string;
    fautive: string;
}

/** `../style.css` → `client/src/style.css`, so that the output can be opened. */
const chemin = (cle: string) => cle.replace(/^\.\.\//, 'client/src/');

const occurrences: Occurrence[] = [];
/** The dimensioned declarations that DO go through a token — reachability. */
let parToken = 0;
let declarationsLues = 0;

for (const [cle, texte] of Object.entries(FEUILLES).sort()) {
    for (const d of declarationsDe(sansCommentaires(texte))) {
        declarationsLues += 1;
        if (/var\(\s*--[a-z0-9-]+\s*\)/i.test(d.value) && !UNITES.test(d.value)) parToken += 1;
        UNITES.lastIndex = 0;
        for (const m of horsExceptions(d.value).matchAll(UNITES)) {
            occurrences.push({
                file: chemin(cle),
                propriete: d.propriete,
                value: d.value,
                fautive: m[0],
            });
        }
    }
}
const values = new Set(occurrences.map((o) => o.fautive));

// ── THE REPORT, ALWAYS PRINTED, SUCCESS INCLUDED ──────────────────────────
console.log(`§7.10  surface sheets: ${Object.keys(FEUILLES).length}`);
console.log(`       declarations read: ${declarationsLues}, of which ${parToken} through a token`);
for (const o of occurrences) {
    console.log(`       ${o.file}  ${o.propriete}: ${o.value}  → « ${o.fautive} »`);
}
console.log(
    `       outside tokens: ${occurrences.length} occurrence(s), ` +
        `${values.size} distinct value(s)` +
        (values.size ? ` — ${[...values].sort().join(', ')}` : ''),
);

describe('§7.10 — no length outside tokens in a surface sheet', () => {
    it('reachability: sheets are read, and lengths go through a token in them', () => {
        // 🔴 WITHOUT THIS ASSERTION, THE NEXT ONE IS GREEN ON EMPTY FILES.
        // It is G5 of `primitives.test.ts`, and it is the trap this sub-project
        // paid for in S2: "four guards out of five would prove nothing". Its
        // red is played by EMPTYING `client/src/style.css`, never by adding
        // a value to it.
        expect(
            Object.keys(FEUILLES).length,
            'no surface sheet found: §7.10 is green while measuring nothing',
        ).toBeGreaterThan(0);
        expect(
            declarationsLues,
            'no declaration read: §7.10 is green while measuring nothing',
        ).toBeGreaterThan(0);
        expect(
            parToken,
            'no length goes through a token: the check does not measure what it thinks',
        ).toBeGreaterThan(0);
    });

    it('every length goes through a token, except the three named exceptions', () => {
        expect(
            occurrences.map((o) => `${o.file}  ${o.propriete}: ${o.value}  → « ${o.fautive} »`),
            `lengths outside tokens: ${occurrences.length} occurrence(s) for ${values.size} distinct value(s)`,
        ).toEqual([]);
    });
});
