import { describe, expect, it } from 'vitest';
import styleCss from './style.css?raw';
import etatTerminalCss from './session/etat-terminal.css?raw';
import boutonsDeCoinCss from './session/boutons-de-coin.css?raw';
import { declarationsDe, preludes, sansCommentaires } from './design/css';

/**
 * ═══════════════════════════════════════════════════════════════════════════
 * THE GUARDS OF THE SESSION SHEET — sub-project ⑥, sub-block S4, task 7.
 *
 * 🔴 IT EXISTS TO TURN INTO A COMMAND A PROPERTY THE SPEC PROTECTS
 * WITH A SENTENCE. §5.2 writes: "Any rework of the button in S4 must
 * keep this property" — `pointer-events: none` on
 * `#fullscreen[data-actif="true"]` —, "changing its size changes the area it
 * occupies, and the justification is written in terms of it". A specification
 * sentence catches nothing; this file does.
 *
 * ⚠️ IT IS WRITTEN AND SEEN RED **BEFORE** THE SIZE CHANGE IT PROTECTS,
 * and the order matters: a guard written AFTER the change does not prove
 * it would have caught the regression. It is the lesson of task 6 of D9 —
 * handle the margin BEFORE the addition, not after.
 *
 * 🔴 ANCHORED ON THE DECLARATION, NEVER ON A SUBSTRING, and this repository paid for
 * this trap THREE TIMES (S1 on `CLE_THEME`, S2 on G1 and G5, S3 on its red
 * no. 16). The comment of `style.css` that justifies this rule WRITES the words
 * `pointer-events`: a guard that looked for them in the raw text would be
 * satisfied by the justification of the file it analyses, and would stay green
 * on a sheet whose rule was removed. The blanking of `./design/css`
 * is therefore mandatory, and the counter-check is played: a mutation that only
 * touches the comment leaves this guard GREEN.
 *
 * 🔴 THIS FILE ONLY READS A NON-EMPTY TEXT THANKS TO `test: { css: true }` of
 * `client/vite.config.ts`, and it must be run FROM `client/`: from the
 * root of the repository, the Vite root changes, the CSS is short-circuited SILENTLY
 * and `styleCss` is the empty string. It is the reachability assertion that
 * catches this case — see `design/lengths.test.ts`, which paid for it.
 * ═══════════════════════════════════════════════════════════════════════════
 */

const CSS = sansCommentaires(styleCss);
const SELECTEURS = preludes(CSS).filter((p) => !p.startsWith('@'));
const CSS_TERMINAL = sansCommentaires(etatTerminalCss);
// 🔴 THE BLOCK OF THE TWO CORNER BUTTONS WAS EXTRACTED from `style.css` by the S4
// cross review (ceiling of 300 lines crossed, caught up by an EXTRACTION
// and never by a compression). Guard ① follows it into its file: it holds
// the RULE, not the file where it lives.
const CSS_BOUTONS = sansCommentaires(boutonsDeCoinCss);

/** The declarations of the block whose prelude is exactly `selecteur`. */
function declarationsDuBloc(css: string, selecteur: string): string[] {
    const regles = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)];
    return regles
        .filter((r) => r[1].trim() === selecteur)
        .flatMap((r) => declarationsDe(`{${r[2]}}`))
        .map((d) => `${d.propriete}: ${d.value}`);
}

/**
 * ═══════════════════════════════════════════════════════════════════════════
 * THE WINDOW CONTROLS OVERLAY GUARD — sub-block S4, task 10.
 *
 * ❌ "THERE IS NO MANIFEST IN THIS REPOSITORY" IS NO LONGER TRUE SINCE
 * SUB-BLOCK G5 (August 21st, 2026), which sets `client/dist/hub.webmanifest` and
 * publishes a `blob:` manifest per application. **Both declare
 * `display_override: ["window-controls-overlay"]`, and Chromium KEEPS it** —
 * its parsed manifest carries `displayOverrides:
 * ["kWindowControlsOverlay","kStandalone"]`, measured twice.
 *
 * 🔴 AND YET THIS GUARD'S CONCLUSION HOLDS, FOR A REASON OTHER THAN
 * THE ONE WRITTEN: WCO only exists in an **installed** PWA window, and
 * a headless Chromium installs none —
 * `matchMedia('(display-mode: window-controls-overlay)').matches` is **false**
 * in both runs. **NO REACHABLE STATE MAKES THE RULE ACT**, so
 * a criterion that claimed to exercise it would always be vacuous BY
 * CONSTRUCTION. The premise has aged, the conclusion has not.
 *
 * 🔴 THE LEGACY ITEM NAMED BELOW WAS THEREFORE TAKEN UP, AND ITS VERDICT IS
 * `NOT MEASURABLE BY THIS SETUP`: G5 brings the DECLARATION, and it does not claim
 * more. **The legacy item remains WHOLE**, passed on with its reason rather than
 * closed. An acceptance criterion that claimed to exercise it would be vacuous
 * BY CONSTRUCTION, and not for lack of effort: that is why S4 prescribes
 * none. What this guard holds is a SHAPE property whose red, on the other hand,
 * has a REAL consequence — writing `env(titlebar-area-height, 8px)` moves the
 * banner down by 8 px NOW, on the product as it runs. It is therefore
 * not a check that validates its own writing.
 *
 * 🔴 WHAT IT DOES NOT PROVE: anything about the behaviour UNDER WCO. The rule has never
 * been rendered in a window with an overlaid title bar. The recipient of this
 * legacy item is NAMED: the acceptance run of sub-block G5 of app management, the one that
 * sets the manifest — it is up to it to look at the session window under an
 * overlaid bar.
 *
 * ── WHY ② FORBIDS A MEDIA QUERY RATHER THAN CHECKING IT ─────────────────────
 * A fallback neutralises an `env()`; NOTHING neutralises an `@media` block. A
 * rule conditional on WCO written as `@media (display-mode:
 * window-controls-overlay)` could therefore change TODAY's layout
 * without any command saying so. Forbidding it makes the "inert
 * today" property TOTAL instead of partial.
 *
 * ⚠️ THE SCOPE IS DERIVED, NEVER ENUMERATED: all the `*.css` of
 * `client/src/`, base layer and primitives included. A new sheet therefore enters
 * this guard without a line here changing, and a copied list cannot
 * diverge from what it describes.
 *
 * ⚠️ EVERYTHING IS READ AFTER BLANKING, in BOTH DIRECTIONS: an `env(titlebar-area-
 * height, 8px)` written in a COMMENT does not make ① go red, and an
 * `env(titlebar-area-` that only lived in a comment does NOT satisfy
 * reachability ③. It is the trap this repository paid for three times, and
 * the box of `./style.css` writes precisely those strings.
 * ═══════════════════════════════════════════════════════════════════════════
 */
const FEUILLES_SRC = import.meta.glob<string>('./**/*.css', {
    query: '?raw',
    import: 'default',
    eager: true,
});

/** `./session/etat-terminal.css` → `client/src/session/etat-terminal.css`. */
const cheminSrc = (cle: string) => cle.replace(/^\.\//, 'client/src/');

/** Every `env(titlebar-area-…)` of the sheets, with its tail of arguments. */
const envs: { file: string; texte: string; repli: string }[] = [];
/** Every at-rule prelude conditional on WCO. */
const requetesWco: { file: string; prelude: string }[] = [];

for (const [cle, brut] of Object.entries(FEUILLES_SRC).sort()) {
    const css = sansCommentaires(brut);
    for (const m of css.matchAll(/env\(\s*(titlebar-area-[a-z-]+)\s*([^)]*)\)/g)) {
        envs.push({ file: cheminSrc(cle), texte: `env(${m[1]}${m[2]})`, repli: m[2].trim() });
    }
    for (const p of preludes(css)) {
        if (/display-mode\s*:\s*window-controls-overlay/.test(p)) {
            requetesWco.push({ file: cheminSrc(cle), prelude: p });
        }
    }
}

// The report, always printed, success included — "a drift check whose
// value is never read only serves to pass" (`poids-css.mjs`).
console.log(`WCO guard  sheets of client/src/: ${Object.keys(FEUILLES_SRC).length}`);
console.log(`           env(titlebar-area-*) read: ${envs.length}`);
for (const e of envs) console.log(`           ${e.file}  ${e.texte}`);

describe('the Window Controls Overlay — the rule is shipped, and it is INERT', () => {
    it('③ reachability: there is at least one env(titlebar-area-) to read', () => {
        // 🔴 WITHOUT THIS ASSERTION, ① IS GREEN WHILE MEASURING NOTHING — and ② is
        // anyway, since it negates. Its red is played by EMPTYING the
        // sheets that carry the rule.
        // ⚠️ AND ALL OF THEM MUST BE EMPTIED: the scope is derived over all the
        // `*.css` of `client/src/`, so emptying `style.css` ALONE leaves
        // `session/etat-terminal.css` carrying its `env()` and reachability
        // stays GREEN, rightly so. It is the prescription flaw that
        // `design/lengths.test.ts` already measured on its own red.
        expect(
            envs.length,
            'no env(titlebar-area-) in client/src/: the WCO guard is green while measuring nothing',
        ).toBeGreaterThan(0);
    });

    it('① every env(titlebar-area-*) carries the 0px fallback', () => {
        expect(
            envs.filter((e) => e.repli !== ', 0px').map((e) => `${e.file}  ${e.texte}`),
            'an env(titlebar-area-*) without the « , 0px » fallback changes the layout TODAY',
        ).toEqual([]);
    });

    it('② no @media query conditional on the WCO', () => {
        // A fallback neutralises an `env()`; nothing neutralises an `@media` block.
        expect(
            requetesWco.map((r) => `${r.file}  ${r.prelude}`),
            'a @media (display-mode: window-controls-overlay) can change the layout without any command saying so',
        ).toEqual([]);
    });
});

describe('style.css — the guards of the session window', () => {
    it('② reachability: the sheet declares rules', () => {
        // 🔴 WITHOUT THIS ASSERTION, THE NEXT ONE IS GREEN ON AN EMPTY SHEET.
        // It is G5 of `primitives.test.ts`, and the trap S2 measured:
        // "four guards out of five would prove nothing". Its red is played by
        // EMPTYING `client/src/style.css`, never by adding something to it.
        expect(
            SELECTEURS.length,
            'style.css declares NO rule: the guards of this sheet are then green while measuring nothing',
        ).toBeGreaterThan(0);
        // 🔴 AND REACHABILITY FOLLOWS THE RULE, NOT THE ORIGINAL FILE.
        // Since the extraction, emptying `style.css` alone would leave guard ①
        // green: its rule lives elsewhere. It is the scope flaw that
        // `design/lengths.test.ts` measured on its own red, and it is
        // replayed here identically as soon as an extraction moves a rule.
        expect(
            preludes(sansCommentaires(boutonsDeCoinCss)).filter((p) => !p.startsWith('@')).length,
            'session/boutons-de-coin.css declares NO rule: guard ① is then green while measuring nothing',
        ).toBeGreaterThan(0);
    });

    it('① the active fullscreen button is OUT of the event flow', () => {
        // The reason, written in `style.css` and repeated here so that it
        // survives a reading of this file alone: without this declaration,
        // a corner area stays in the event flow and swallows the clicks
        // meant for the game (minimap, shop…) in absolute fullscreen. Leaving
        // fullscreen remains possible with a long press on Escape (Keyboard
        // Lock), so removing the button from the flow traps nobody.
        //
        // ⚠️ THIS GUARD HOLDS THE PROPERTY, NOT THE GEOMETRY. No page is
        // opened, no pixel is measured, and the advance of the `⛶` glyph remains
        // unknown to everyone here. That the occupied area is the right one is a
        // HUMAN JUDGEMENT (spec §8), and it has not been made.
        expect(
            declarationsDuBloc(CSS_BOUTONS, '#fullscreen[data-actif="true"]'),
            'session/boutons-de-coin.css: the #fullscreen[data-actif="true"] rule does not declare pointer-events: none',
        ).toContain('pointer-events: none');
    });

    it('③ the terminal screen stays HIDDEN as long as it carries `hidden`', () => {
        // 🔴 `[hidden]` LOSES AGAINST AN AUTHOR RULE. The
        // `[hidden] { display: none }` that makes the attribute effective lives in the
        // user agent stylesheet, and the cascade compares ORIGIN before
        // specificity: `.ecran { display: grid }` wins, even though it is less
        // specific. Without the explicit rule this assertion requires,
        // the full-frame screen would be VISIBLE FROM LOAD, on every
        // session, over the video — and no other check would see it.
        //
        // ⚠️ ANCHORED ON THE WHOLE RULE: the selector is compared by exact
        // equality, and the declaration is read AFTER blanking. An `.ecran[hidden]`
        // written in a comment therefore does not satisfy this guard — it is the
        // trap this repository paid for three times.
        expect(
            declarationsDuBloc(CSS_TERMINAL, '.ecran[hidden]'),
            'session/etat-terminal.css does not declare .ecran[hidden] { display: none }',
        ).toContain('display: none');
    });
});
