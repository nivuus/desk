/**
 * Wiring the accent colour onto the DOM — sub-block **A1**.
 *
 * **The only DOM line of the mechanism**, and nothing else: the decision — judging
 * readability, refusing, rendering the theme — lives in `accent.ts`, **pure** and
 * tested. The pattern is `presse-papier-dom.ts` (P1), itself backed by
 * `presse-papier.ts`.
 *
 * **Written BEFORE touching `main.ts`**, and for the reason
 * `presse-papier-dom.ts` documents: `main.ts` was at **466 lines at the time
 * of writing this module** — it has 483 since the three wiring lines
 * went in — for a project
 * ceiling of 500, it has **NO test**, and `client/` has **neither jsdom nor
 * happy-dom**. This module does have one, because it touches neither `document` nor
 * `window` directly but receives `lireToken` and `poserToken` by injection —
 * the pattern of `attachFullscreenAuDOM`, `armerLeSon` and `theme.ts`.
 *
 * 🔴 **A1 SETS THE REPOSITORY'S FIRST `setProperty`.** Noted before writing:
 * `grep -rn 'setProperty' client/src/` returned **zero**. The rule this
 * precedent sets, and which must therefore be written down: **a runtime token
 * write is only done on `document.documentElement`, never on an
 * element.** A token set on `document.body` would be invisible to
 * `getComputedStyle(document.documentElement)`, and that is exactly the red of
 * criterion ② of the acceptance run.
 *
 * 🔴 **AND THE FIRST PRODUCT CALLER OF `getComputedStyle`.** The only other one
 * is `design/galerie.ts`, which itself declares it is **not** part of the product —
 * no surface imports it, it has no test. The rule of §4.1 of spec ⑥
 * — "on a theme change, never per frame" — is respected and even
 * exceeded: we read **when a message arrives**, that is at most once every
 * `PERIODE_ACCENT` (5 s), and **only when the icon has changed**, the
 * sensor only announcing on change.
 *
 * ⚠️ **`--accent-fenetre` is declared NOWHERE in `tokens.css`, and it is
 * a MEASURED decision** (D-A1-2, probe H1 of August 21st, 2026): declaring it there
 * makes §7.6 **RED** ("NEW ORPHAN — declared and called by nobody"),
 * because this check's "used" scope is the `.css` only and
 * its detection only recognises `var(--…)` — a TypeScript `setProperty` is
 * invisible to it **twice**. And the "waiting line" route the spec
 * declares acceptable is only tenable **if A1 refuses to declare itself closed**,
 * which is worse. The probe's five cells are filed in
 * `journaux-accent-a1/01-sonde-h1.log`.
 *
 * ⚠️ **Consequence to be written down, since nobody will see it elsewhere**: before the
 * first message, `var(--accent-fenetre)` is **undefined**. No stylesheet
 * references it today — noted, `grep -rn 'accent-fenetre' client/src/` only
 * returns this file —, so nothing suffers from it; but **any future reference
 * must carry a fallback (`var(--accent-fenetre, var(--accent))`) or declare the
 * token**, and §7.6 will say so at its FIRST inclusion ("no undeclared
 * `var(--…)`"). **The declaration goes with its caller, and both
 * belong to G5**, which will set up the PWA manifest.
 *
 * ❌ TWO CLAUSES OF THIS PARAGRAPH WERE TAKEN UP BY G5 (August 21st, 2026), AND
 * THE FIRST ONE IS MEASURED WRONG.
 *
 *   ① "**a fallback OR declare the token**" suggests the fallback is enough.
 *      **IT IS NOT ENOUGH**: `tokensReferences` uses
 *      `/var\(\s*(--[\w-]+)/g`, so `var(--accent-fenetre, var(--accent))`
 *      STILL captures `--accent-fenetre`, and §7.6 turns red —
 *      "UNDECLARED --accent-fenetre used by …". **Played**, on the real
 *      check, log filed (G5's red no. 8). Only the SECOND half of the
 *      alternative holds: one must **declare**.
 *
 *   ② "G5, which will set up the PWA manifest": **it did set it up**, and it
 *      **painted nothing** for all that — `--accent-fenetre` stays without a declaration and
 *      without a caller. Four reasons, all measured (decision D3 of its
 *      plan): a manifest's `theme_color` is **static and per
 *      APPLICATION** whereas this token is **per WINDOW**; S4's WCO guard
 *      forbids **any** `@media (display-mode: window-controls-overlay)`;
 *      `tokens.css` is at **300 lines for a gate of 300**, and its
 *      readers are **NINE** and not seven; and §7.4 would require a light
 *      counterpart or an eighth off-theme entry. **The legacy stays OPEN.**
 */

import { conformer } from './accent';

/** The token this module writes, and the only one. */
export const TOKEN_ACCENT = '--accent-fenetre';

/**
 * The current theme's three backgrounds, plus the theme's accent.
 *
 * ⚠️ **ALL THREE, and not just `--fond-0`**: spec D10 point 2 says "the
 * current theme's three backgrounds", and a window's accent can sit on
 * any of them depending on the surface.
 */
const FONDS = ['--fond-0', '--fond-1', '--fond-2'] as const;
const ACCENT_DU_THEME = '--accent';

/** What this module needs from the document, and nothing else. */
export interface AccesTokens {
    /** Reads the CURRENT value of a token on the root. */
    lireToken(nom: string): string;
    /** Writes a token on the root — **never on an element**. */
    poserToken(nom: string, value: string): void;
}

/**
 * Receives a colour announced by the agent, conforms it, and sets the result.
 *
 * 🔴 **The backgrounds are reread AT EACH MESSAGE, never stored at mount time.**
 * Otherwise, a theme switch would leave the accent judged against the OLD
 * theme: the light and dark backgrounds have nothing in common, and a colour
 * readable on `#0b0d10` is not necessarily readable on `#ffffff`. It is a test.
 *
 * ⚠️ **On a REFUSAL, we set `--accent` — we do not leave the token in place.**
 * Setting nothing would leave it its PREVIOUS value, that is, the hue
 * of an icon that is no longer this window's: a stale state, more
 * misleading than a visible fallback. It is also a test.
 */
export function attacherAccentAuDOM(acces: AccesTokens): { recevoir(couleur: string): void } {
    return {
        recevoir(couleur: string): void {
            const fonds = FONDS.map((nom) => acces.lireToken(nom));
            const accentDuTheme = acces.lireToken(ACCENT_DU_THEME);
            acces.poserToken(TOKEN_ACCENT, conformer(couleur, fonds, accentDuTheme));
        },
    };
}
