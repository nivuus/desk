import { describe, expect, it } from 'vitest';
import { blocApres, compounds, declarationsDe, preludes, sansCommentaires } from './css';
import primitivesCss from './primitives.css?raw';
import boutonCss from './primitives/bouton.css?raw';
import champCss from './primitives/champ.css?raw';
import surfaceCss from './primitives/surface.css?raw';
import messageCss from './primitives/message.css?raw';
import baseCss from './base.css?raw';

/**
 * 🔴 THE GUARDS READ THE FAMILIES, NOT `primitives.css`, which is now only
 * a list of `@import` since the extraction of T4. Reading it alone would make
 * the four absence guards measure ZERO rules — it is G5 that caught it,
 * RED, at the extraction itself.
 *
 * ⚠️ ANY NEW FAMILY IS ADDED HERE **AND** IN `primitives.css`: G5 compares
 * the two lists, so that a fifth family imported without being read here
 * — hence out of reach of G1 to G4 — brings the guard down.
 */
const FAMILLES = new Map([
    ['./primitives/bouton.css', boutonCss],
    ['./primitives/champ.css', champCss],
    ['./primitives/surface.css', surfaceCss],
    ['./primitives/message.css', messageCss],
]);

/**
 * THE SHAPE GUARDS OF THE PRIMITIVES — sub-project ⑥, sub-block S2.
 *
 * 🔴 THIS FILE ONLY READS A NON-EMPTY TEXT THANKS TO `test: { css: true }` of
 * `client/vite.config.ts`. Without that line, Vitest short-circuits CSS
 * files — the `?raw` query included — and `primitivesCss` is the EMPTY string:
 * guards ① to ④ below, which are ABSENCE tests, would all turn
 * green WHILE MEASURING NOTHING. That is also why there is deliberately no
 * `client/vitest.config.ts`: it would take precedence over the Vite configuration
 * without a word.
 *
 * 🔴 AND THAT IS EXACTLY WHY G5 EXISTS. G1 to G4 look for the absence of
 * something; a `primitives.css` reduced to its header would satisfy all
 * four. G5 is the REACHABILITY guard: it requires that there be something
 * to measure.
 *
 * 🔴 BLANKING THE COMMENTS IS NOT A DETAIL. The header of
 * `primitives.css` explains WHY `outline: none`, `opacity` and literal
 * lengths are forbidden there — so it WRITES those strings. A guard
 * that looked for them in the raw text would be satisfied by its own
 * justification, and would stay green on a file whose rule was removed.
 * That is word for word what happened to the bootstrap guard in S1.
 *
 * ⚠️ BUT ITS REAL SCOPE IS NARROWER THAN THAT, AND IT IS MEASURED —
 * writing it wide would be claiming beyond the measurement. Neutralising the blanking
 * brings down G1 (and hence G5, which reads the same preludes): the
 * `preludes` scan takes the text of a comment preceding a `{` for a list of
 * selectors, and G1 then reports 889 parasitic compounds including `/*` (⚠️ 672 at
 * task 2, when `bouton.css` was alone: the number grew with the next three
 * families, and it was MEASURED AGAIN on August 20th, 2026 rather than copied).
 * G2, G3 and
 * G4 stay GREEN without it, including with an `outline: none;` written in a
 * comment placed INSIDE a block (tried): those three do not look for
 * a substring, they read a PROPERTY POSITION in a
 * declaration, and a `/* … outline` is not one. Blanking remains
 * required — it is the only safeguard of G1 and G5 —, and it is the way G2 to
 * G4 are written that puts them out of reach of the trap, not it.
 */

/* 🔴 THE STYLESHEET READER LIVES IN `./css`, EXTRACTED BY TASK 1 OF S4 —
   blanking, preludes, declarations, matched block, compounds. It was here
   privately; the new guards of S4 (§7.10, `style.test.ts`) reuse it instead
   of copying it, and copied machinery drifts from one guard to the other
   without any command saying so. This file returns EXACTLY the same
   verdicts as before the extraction: G1 to G7, nine tests, no message changed.
   ⚠️ `Declaration` is no longer declared here: `declarationsDe` returns its type,
   and the guards below never name one. */

const CSS = sansCommentaires([...FAMILLES.values()].join('\n'));
const SELECTEURS = preludes(CSS).filter((p) => !p.startsWith('@'));
const DECLARATIONS = declarationsDe(CSS);

/** The selector lists that cite a family — the tool of G5 and G6. */
const famille = (nom: string) => SELECTEURS.filter((s) => s.includes(`.${nom}`));

/**
 * G6 — the states and parts a family must declare.
 *
 * 🔴 IT READS SELECTORS, NOT PROSE. A removed rule brings down this
 * guard; the COMMENT mentioning it, blanked from the start, does not hold it back.
 * That is the only way to know it measures code — S1 paid twice for a
 * guard satisfied by its own justification.
 *
 * ⚠️ WHAT IT DOES NOT SAY: that the state is WELL expressed. That a field in error
 * stands out, that a disabled one reads as inert — those are human
 * judgements of §8, and none will become a measurement.
 */
function etatsManquants(nom: string, attendus: string[]): string[] {
    // 🔴 `:not(…)` IS REMOVED BEFORE THE SEARCH, AND IT IS NOT A NICETY:
    // without it, `.champ__saisie:hover:not(:disabled)` contains the substring
    // `:disabled`, and the guard can no longer fail when the RULE
    // `.champ__saisie:disabled` disappears. Measured: the T3 red at first brought
    // nothing down, on a rule really removed. It is the pattern of the
    // vacuous check, caught here on the guard itself.
    const selecteurs = famille(nom)
        .map((s) => s.replace(/:not\([^)]*\)/g, ''))
        .join('  ');
    return attendus.filter((etat) => !selecteurs.includes(etat));
}

describe('primitives.css — the shape guards', () => {
    it('G1 — no bare element selector: every compound carries a class', () => {
        // 🔴 IT IS THIS GUARD THAT KEEPS `index.html` NEUTRAL. The session
        // window carries five elements without any primitive class, including
        // TWO `<button>`: a `button { … }` written here would change its
        // appearance without any of the nine checks saying so.
        const nus: string[] = [];
        for (const list of SELECTEURS) {
            for (const selecteur of list.split(',')) {
                for (const compound of compounds(selecteur.trim())) {
                    if (!compound.includes('.')) nus.push(compound);
                }
            }
        }
        expect(nus, 'bare element selectors in primitives.css').toEqual([]);
    });

    it('G2 — the focus ring is never removed', () => {
        // `base.css` sets `:focus-visible` GLOBALLY: no primitive has
        // to declare it, and the only risk is that one of them erases it "to
        // look tidy". None of the nine checks would see it.
        const effacements = DECLARATIONS.filter(
            (d) =>
                /^outline(-(width|style))?$/i.test(d.propriete) &&
                /^(none|0|0px|0rem|0em)$/i.test(d.value),
        ).map((d) => `${d.propriete}: ${d.value}`);
        expect(effacements, 'removals of the focus ring').toEqual([]);
    });

    it('G3 — no state is expressed through a runtime composition', () => {
        // `opacity` and `filter` compose the colour AT RENDERING: the effective
        // tint then escapes the 52 pairs of check §7.1. A disabled
        // state expressed by an opacity would be the only state of the product
        // whose contrast nothing measured.
        const compositions = DECLARATIONS.filter((d) =>
            ['opacity', 'filter', 'backdrop-filter'].includes(d.propriete.toLowerCase()),
        ).map((d) => `${d.propriete}: ${d.value}`);
        expect(compositions, 'runtime compositions in primitives.css').toEqual([]);
    });

    it('G4 — no length off the scale: every unit goes through a token', () => {
        // ⚠️ THIS GUARD DOES NOT SAY THE RIGHT TOKEN WAS CHOSEN. It says
        // that no length is written outside the scales of §4.4 — which
        // none of the nine checks measures HERE.
        // ❌ "since none measures a length": no longer true since
        // sub-block S4. §7.10 measures one, but on the SURFACE sheets
        // only — `client/src/design/` is outside its scope, and
        // that is precisely the boundary the two guards each write
        // from their side. G4 therefore remains the only guard of these four families.
        const hors: string[] = [];
        for (const d of DECLARATIONS) {
            const reste = d.value.replace(/var\(\s*--[a-z0-9-]+\s*\)/gi, ' ');
            const trouve = reste.match(/(\d+(?:\.\d+)?)(px|rem|em|ms|s|pt|ch|vw|vh)\b/);
            if (trouve) hors.push(`${d.propriete}: ${d.value} → « ${trouve[0]} » outside tokens`);
        }
        expect(hors, 'literal lengths in primitives.css').toEqual([]);
    });

    it('G5 — reachability: the file declares rules, including the button family', () => {
        // 🔴 WITHOUT THIS GUARD, THE FOUR PREVIOUS ONES PROVE NOTHING: they are
        // absence tests, and an empty file satisfies them all.
        expect(
            SELECTEURS.length,
            'primitives.css declares NO rule: G1 to G4 are then green while measuring nothing',
        ).toBeGreaterThan(0);
        expect(famille('bouton'), 'the .bouton family is missing from primitives.css').not.toEqual(
            [],
        );
        // 🔴 AND THAT THIS FILE DOES READ EVERYTHING `primitives.css` IMPORTS:
        // a family imported but absent from `FAMILLES` would escape G1, G2,
        // G3 and G4 without any command saying so.
        const importees = [...primitivesCss.matchAll(/@import\s+'([^']+)'/g)].map((m) => m[1]);
        expect(importees.sort(), 'the imported families and those this test reads diverge').toEqual(
            [...FAMILLES.keys()].sort(),
        );
    });

    it('G6 — the FIELD family declares its states and its parts', () => {
        expect(
            etatsManquants('champ', [
                '.champ__etiquette',
                '.champ__saisie',
                '.champ__aide',
                '.champ__erreur',
                '::placeholder',
                ':disabled',
                '.champ--erreur',
            ]),
            'states or parts missing from the field family',
        ).toEqual([]);
    });

    it('G6 — the SURFACE family declares its parts, and the separator', () => {
        expect(
            etatsManquants('carte', ['.carte__titre', '.carte__corps']),
            'parts missing from the card family',
        ).toEqual([]);
        expect(famille('separateur'), 'the separator is missing from primitives').not.toEqual([]);
    });

    it('G6 — the MESSAGE family declares its four tones', () => {
        // The NEUTRAL tone is `.message` itself: the other three only
        // change the ink and the stroke.
        expect(
            etatsManquants('message', ['.message--succes', '.message--alerte', '.message--danger']),
            'tones missing from the message family',
        ).toEqual([]);
    });

    it('G7 — base.css neutralises the transitions under prefers-reduced-motion', () => {
        // 🔴 BLANKING IS STRICTLY NECESSARY HERE: the header of the
        // rule COPIES the measurement command that imposed it, hence the string
        // `@media (prefers-reduced-motion: reduce)` spelled out, AND the
        // block `{ :root { --duree-1: 0.01ms; } }` that follows it in the report.
        //
        // ❌ "A guard that looked for it in the raw text would stay GREEN on
        // a `base.css` whose rule was removed" — written here by task 6
        // and REFUTED BY MEASUREMENT on August 20th, 2026 (cross review, log
        // `journaux-design-s2/rouges-rejouees.log`). It does not stay green: its
        // FIRST assertion is indeed satisfied by the comment, but the
        // SECOND then reads the block of the report and falls —
        //   `expected [ '--duree-1' ] to include 'transition-duration'`.
        // ⚠️ AND IT IS WORSE THAN WHAT THE FALSE STATEMENT DESCRIBED: without blanking,
        // this guard returns THIS SAME RED whether the rule is PRESENT or ABSENT —
        // it stops discriminating, and becomes a false positive on a perfectly
        // correct `base.css`. Blanking is not what keeps it from being
        // wrongly green: it is what makes it able to say anything at all.
        const base = sansCommentaires(baseCss);
        const debut = base.search(/@media\s*\(\s*prefers-reduced-motion\s*:\s*reduce\s*\)/);
        expect(
            debut,
            'no @media (prefers-reduced-motion: reduce) query in base.css, comments blanked',
        ).toBeGreaterThan(-1);
        expect(
            declarationsDe(blocApres(base, debut)).map((d) => d.propriete),
            'the reduced-motion query carries no declaration',
        ).toContain('transition-duration');
    });
});
