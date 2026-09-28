/**
 * Conforming the accent colour received from the agent — sub-block **A1**.
 *
 * **PURE, without DOM.** It only takes strings and returns a string. Reading
 * `getComputedStyle` and writing `setProperty` live in
 * `accent-dom.ts` — it is the pattern P1 set for the clipboard
 * (`presse-papier.ts` pure / `presse-papier-dom.ts` on the DOM).
 *
 * ⚠️ **Spec §7.1 files this module as "PURE, without DOM" AND its decision D10
 * point 2 makes it read the backgrounds through `getComputedStyle`. The two are
 * irreconcilable in a single module** (the plan's divergence E8): the backgrounds are
 * therefore INJECTED, and testability decides — "a plan that dictates
 * untestable code yields to its own constraint" (E15 of P3).
 *
 * 🔴 **IT IS THE PRODUCT'S ONLY RAMPART, and it is not an empirical finding
 * but a STRUCTURAL property of the design system's checks (E10):**
 *
 * - **§7.2** is **syntactic**, and it says so itself
 *   (`client/outils/couleurs-litterales.mjs`: "an `el.style.background =
 *   'red'` would therefore pass");
 * - **§7.1** parses `tokens.css` and compares a list of pairs **declared by
 *   hand** (`design/contraste.ts`), never a Cartesian product, and
 *   `client/outils/contraste.mjs` excludes **by name** colours composed at
 *   runtime.
 *
 * **A colour received from the server is therefore outside any automatic contrast
 * check of this repository.** Its only judge is `accent.test.ts`, and that is
 * why its red had to be SEEN.
 */

import { rapportDeContraste } from './design/contraste';

/**
 * The readability threshold of a non-text component — WCAG 1.4.11.
 *
 * 🔴 **IT IS A DUPLICATION, AND IT IS DECLARED AS A DEBT.** The plan
 * prescribed importing `SEUIL_COMPOSANT` from `design/contraste.ts` — "a
 * check that has its own copy of the values validates its copy" (spec ⑥ §7.1) —
 * **and it declared it had not checked that it was exported**. It is
 * not: `const SEUIL_COMPOSANT = 3;` there is a **module** constant, not
 * exported. The course of action was written in advance: use the literal
 * and **name the duplication**, never modify `contraste.ts` to export it
 * — `client/src/design/` belongs to sub-project ⑥, closed (D-A1-14).
 *
 * ⚠️ **The day these two 3s diverge, NOTHING will say so.** The remedy, if it
 * becomes necessary, belongs to ⑥: export the constant and import it here.
 */
const SEUIL_COMPOSANT = 3;

/** `#rrggbb`, six hexadecimal digits, and nothing else. */
const FORME = /^#[0-9a-f]{6}$/;

/**
 * Returns the received colour if it is **conforming and readable**, `accentDuTheme`
 * otherwise. It **never throws**, and it **never corrects**.
 *
 * Three steps, and each is a possible refusal:
 *
 * 1. **the shape** — `#rrggbb` on the *trimmed and lowercased* value. Any other
 *    shape is refused **BEFORE** reaching `rapportDeContraste`.
 *    🔴 **That is what guarantees it never throws**:
 *    `luminanceRelative` **THROWS** on anything that is not `#rgb`, `#rgba`,
 *    `#rrggbb` or `#rrggbbaa` (divergence E9 — the spec says so nowhere, and
 *    an exception in a data channel message handler is the
 *    kind of defect that kills a session without saying anything).
 *    ⚠️ **A `try/catch` around the call would be a WEAKER check**: it
 *    would also catch a regression of `contraste.ts` by disguising it as an
 *    ordinary refusal. There is therefore none, deliberately.
 * 2. **readability** — a ratio of at least `SEUIL_COMPOSANT` against
 *    **EACH** of the backgrounds passed. ⚠️ **All three and not just `--fond-0`**:
 *    spec D10 point 2 says "the current theme's three backgrounds", and a window's
 *    accent can sit on any of them depending on the surface. It is
 *    the strictest choice, it is **deliberate**, and it will refuse more
 *    colours — **a refusal is a HEALTHY state of this mechanism**, not a failure.
 * 3. **no correction.** Neither lightening, nor darkening, nor
 *    `color-mix`: "lightening or darkening an application's colour
 *    would produce a hue nobody chose" (D10 point 3).
 *    **Refusing IS the behaviour**, not a failure fallback.
 *
 * ⚠️ **`accentDuTheme` is READ by the caller, never written here.** A colour
 * hardcoded in this file would turn §7.2 red — measured by cell E of
 * probe H1: `const REPLI = '#7aa2f7'` there returns
 * "literal colours: 1", exit 1.
 */
export function conformer(
    recue: string,
    fonds: readonly string[],
    accentDuTheme: string,
): string {
    const candidate = recue.trim().toLowerCase();
    if (!FORME.test(candidate)) return accentDuTheme;
    if (fonds.length === 0) return accentDuTheme;
    for (const fond of fonds) {
        if (!FORME.test(fond.trim().toLowerCase())) return accentDuTheme;
        if (rapportDeContraste(candidate, fond.trim().toLowerCase()) < SEUIL_COMPOSANT) {
            return accentDuTheme;
        }
    }
    return candidate;
}
