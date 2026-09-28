/**
 * The three theme states — PURE: INJECTED dependencies, no DOM.
 *
 * ⚠️ NEITHER `window`, NOR `localStorage`, NOR `document` IS AVAILABLE HERE
 * under Vitest: `client/` has no `vitest.config.*` and no `jsdom`
 * (`ls client/node_modules/@types/` returns `estree` alone). `Coffre` and `Racine`
 * are therefore PARAMETERS, on the exact pattern of `client/src/jeton.ts:4-10`.
 * A `const racine = document.documentElement` at the top of the module would be enough to
 * make this file impossible to load under Node, hence impossible to test.
 *
 * ⚠️ ERASABLE TypeScript (no `enum`, no `namespace`): this module is
 * in the same directory as those a `.mjs` imports, and the constraint
 * applies there for consistency. See the header of `tokens.ts` for the measurement.
 *
 * ────────────────────────────────────────────────────────────────────────────
 * WHAT THESE FUNCTIONS DO NOT PROVE, word for word from spec §7.5:
 * "it does not establish that the browser does fire `storage` between two
 * real windows. It exercises OUR handler, not the platform."
 *
 * ✅ The confirmation with two real windows WAS DONE on August 19th, 2026, at
 * commit `604f91c`, and it stays OUTSIDE THE CRITERIA — same status as the
 * real-VM confirmations of sub-project ⑤. Two tabs of
 * `client/design.html`: the neighbour follows the switch AND re-renders, and
 * going back to `systeme` REMOVES the attribute from it. Evidence:
 * `docs/superpowers/plans/journaux-design-s1/corroboration-deux-fenetres.md`.
 * ⚠️ ONE run, ONE browser (desktop Chromium), and TWO WINDOWS OF THE
 * GALLERY — not a shell page opening N sessions through `window.open`. Whether the
 * theme reaches the PRODUCT's N windows stays unmeasured.
 *
 * ⚠️ "`surStockageModifie` writes nothing into the vault" is guaranteed by the
 * SIGNATURE, not by a test: the function receives no `Coffre`, so it
 * has nothing to write. A test of this property could never fail, and
 * this repository keeps no check unable to fail. It is what closes the
 * loop between windows: a neighbour that rewrote while reacting
 * would trigger a `storage` in its own neighbours, without end.
 */

export type Theme = 'systeme' | 'clair' | 'sombre';

/**
 * ⚠️ The key is PREFIXED, where spec §4.2 wrote a bare `theme`. The repository
 * already has the convention: `client/src/jeton.ts:37-38` declares
 * `guac.jeton.acces` and `guac.jeton.rafraichissement`. A bare key on the same
 * origin as the hub, the shell page and N session windows is a collision
 * waiting to happen; the prefix costs nothing.
 */
export const CLE_THEME = 'guac.theme';

const ATTRIBUT = 'data-theme';

export interface Coffre {
    getItem(cle: string): string | null;
    setItem(cle: string, valeur: string): void;
}

export interface Racine {
    setAttribute(nom: string, valeur: string): void;
    removeAttribute(nom: string): void;
}

function estTheme(valeur: string | null): valeur is Theme {
    return valeur === 'systeme' || valeur === 'clair' || valeur === 'sombre';
}

/** Reads the vault. Any unknown value — `null` included — returns `'systeme'`. */
export function themeStocke(coffre: Coffre): Theme {
    const valeur = coffre.getItem(CLE_THEME);
    return estTheme(valeur) ? valeur : 'systeme';
}

/**
 * Sets `data-theme`, or REMOVES it for `'systeme'`: its ABSENCE means
 * "systeme". See the matching test for what a `data-theme="systeme"`
 * would break — and above all for what it would not break, visibly.
 */
export function appliquer(racine: Racine, theme: Theme): void {
    if (theme === 'systeme') racine.removeAttribute(ATTRIBUT);
    else racine.setAttribute(ATTRIBUT, theme);
}

/**
 * Writes AND applies locally.
 *
 * 🔴 BOTH HALVES ARE NECESSARY, and it is the trap spec §4.2
 * names: the `storage` event DOES NOT FIRE in the document that
 * wrote. The window changing the theme is precisely the only one
 * the user is looking at; if it merely wrote, it would be the only one
 * not to change appearance.
 */
export function choisir(coffre: Coffre, racine: Racine, theme: Theme): void {
    coffre.setItem(CLE_THEME, theme);
    appliquer(racine, theme);
}

/**
 * Reacts to a `storage` event coming from ANOTHER window. Writes NOTHING —
 * see the header.
 *
 * ⚠️ The key filter is not a theoretical precaution:
 * `client/src/connexion.ts:57` really writes `guac.jeton.acces`, so every
 * neighbouring window receives that event. Without the filter, `data-theme`
 * would be a JWT.
 */
export function surStockageModifie(racine: Racine, cle: string | null, valeur: string | null): void {
    if (cle !== CLE_THEME) return;
    appliquer(racine, estTheme(valeur) ? valeur : 'systeme');
}
