#!/usr/bin/env node
// Check §7.6 of spec ⑥ — NO ORPHAN TOKEN, NO UNDECLARED `var()`.
//
// THREE assertions — two inclusions in both directions between the tokens
// DECLARED by `tokens/couleurs.css` and `tokens/echelles.css` (`tokens.css`
// before the extraction of task 6, August 25th, 2026 — this check needs
// BOTH, the orphan list carrying colours as well as
// scales) and the tokens USED by the production sheets, plus one
// about the waiting list itself:
//
//   ① used ⊆ declared — a `var(--fond-O)` (letter O instead of zero) is a
//      typo the browser silently swallows: the property takes
//      its fallback value, or nothing, and the page stays up but wrong;
//   ② declared ⊆ used — a token nobody calls is dead code, and
//      dead code in a single source of values gets copied for a long time;
//   ③ no entry of the waiting list names an ALREADY CLOSED sub-block —
//      the partial mitigation of re-labelling, built in sub-block S3, and which
//      the S2 plan declared impossible THREE times. Its doctrine and its
//      objection live next to the list.
//
// 🔴 "USED" HAS TWO SOURCES SINCE TASK 7 OF A1 (August 25th, 2026), NOT ONE:
// a `var(--…)` in CSS (below), AND a TypeScript `poserToken(...)`
// outside tests (`tokens-orphelins/js.mjs`, with its doctrine and its
// selection rule). Without the second, a token set AT RUN TIME and
// referenced by NO CSS — the exact case of `--accent-fenetre` — had only
// two outcomes, BOTH WRONG: NOT DECLARED AT ALL, and the token stays
// invisible to both inclusions (the real legacy item, chosen by `accent-dom.ts`
// rather than the other); or DECLARED WITHOUT A CSS USER, and ② reports it as
// an ORPHAN — for the WRONG reason, since the JS does use it.
// See `accent-dom.ts` for the legacy item this hole forced.
//
// It carries NO parsing rule: `tokensDeclares` and `tokensReferences`
// live in `client/src/design/tokens.ts`, which is typechecked and tested.
//
// ═══════════════════════════════════════════════════════════════════════════
// 🔴 `client/design.html` IS EXCLUDED FROM THE "USED" HALF, AND THAT IS WHAT
// MAKES THIS CHECK ABLE TO FAIL.
//
// The gallery renders ALL the tokens by construction — that is its raison d'être.
// Including it in the "used" scope would make inclusion ② true
// forever: the check would validate the gallery and nothing else. Spec §7.6
// says so, and this repository caught four checks unable to fail in
// sub-block D10 alone, three of which were written by a plan.
//
// ⚠️ THE EXCLUSION IS LOAD-BEARING, NOT DECORATIVE, and it can be measured: the scope
// below scans the HTML SURFACES as much as the `.css` sheets, because
// a page can reference a token from an inline `<style>` — which is what
// the gallery does precisely. Removing `design.html` from `EXCLUS` turns
// red A green (red C of task 12).
// ═══════════════════════════════════════════════════════════════════════════
//
// ⚠️ SIZE OF THIS FILE — THE PREDICTION OF THE S2 PLAN IS WRONG, AND IT IS SAID
// RATHER THAN PLANED DOWN. The plan expected from task 8 a file "shorter
// than at `56b975a` (233)", the 18 entries removed from the waiting list being
// supposed to slim it down. Measured by the command at task 8 of S2:
//
//   list entries      28 → 10   (−18)
//   comments         104 → 153  (+49)
//   code              87 →  93  (+6: the second exclusion and `--sans-exclusion`)
//   blank lines       14 →  14
//   TOTAL            233 → 270  (+37)
//
// **The 18 removed entries were more than cancelled out by comment.**
// It is, in small, the lesson this repository paid for in large: "an addition of
// comment can cancel an extraction". The +49 is not padding
// — it is the MEASURED reason for the second exclusion (task 7), the box about
// the re-tags nothing checks, and the S1 report redone instead of being
// erased, three blocks the plan REQUIRES. **Planing them down would trade a truth
// for a number**, which `CLAUDE.md` explicitly forbids.
//
// 🔴 "270 versus 300" (task 8 of S2) WAS ALREADY WRONG BEFORE THIS TASK
// (256 since task 6 of S2/A1); task 7 of A1 (the "set by the
// JS" half, `tokens-orphelins/js.mjs`) makes it go up again. NO NUMBER
// IS WRITTEN HERE ANY MORE: `wc -l client/outils/tokens-orphelins.mjs`, RERUN,
// is the only source of truth.
//
// ⚠️ GATE ARMED AT 300 LINES (self-imposed, like its two token
// neighbours): at that threshold, split THE REPORT — the `console.log` of the three
// inclusions and of the total — into `tokens-orphelins/rapport.mjs` (parameters:
// `declares`, `employePar`, `orphelins`, `nonDeclares`,
// `EN_ATTENTE_D_APPELANT`, `SOUS_BLOCS_CLOS`), keeping here only the
// COLLECTION (CSS, surfaces, JS) and the COMPUTATION of the gaps — the landing point
// of task 8 of S2 (`attente.mjs`) is already reached, this one is the
// NEXT. BEFORE the addition that would cross, never after.
// ═══════════════════════════════════════════════════════════════════════════

import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { tokensDeclares, tokensReferences } from '../src/design/tokens.ts';
import configVite from '../vite.config.ts';
import { EN_ATTENTE_D_APPELANT } from './tokens-orphelins/attente.mjs';
import { SOUS_BLOCS_CLOS } from './tokens-orphelins/sous-blocs-clos.mjs';
import { tokensPosesParLeJs } from './tokens-orphelins/js.mjs';

/**
 * 🔴 TWO SOURCES SINCE THE EXTRACTION OF TASK 6 (August 25th, 2026), NOT ONE:
 * they DECLARE, they do not use. Outside the scope — like
 * `tokens.css`, their common entry point, which no longer declares any
 * itself (it now only IMPORTS the two) and therefore joins the exclusion
 * for the same reason, below.
 */
const SOURCES = [
    'client/src/design/tokens/couleurs.css',
    'client/src/design/tokens/echelles.css',
];
const PORTE = 'client/src/design/tokens.css';

/**
 * 🔴 THE ONLY EXCLUSION FROM THE "USED" SCOPE. See the box above.
 * Any addition to this list must carry the reason why the file
 * cannot make the check fail — not the reason why it is
 * in the way.
 */
const EXCLUS = new Map([
    [
        'client/design.html',
        'the gallery renders every token by construction; including it would make ' +
            'the inclusion “declared ⊆ used” true forever',
    ],
    [
        // 🔴 THE REASON WHY THIS FILE CANNOT MAKE THE CHECK
        // FAIL, and not the reason why it would be in the way — it is the clause of
        // the box above. Measured, not assumed: before this entry, the
        // page brought down FIVE "TO REMOVE FROM THE LIST" lines (--e-5,
        // --e-6, --e-7, --lh-large, --t-3xl), all used by its ONLY
        // demonstration layout. The waiting list would have shrunk by
        // five without the product gaining a single caller.
        'client/primitives.html',
        'a demonstration page uses tokens by construction, in its ' +
            'own layout; including it would take out of the waiting list ' +
            'tokens that the PRODUCT does not call',
    ],
]);

// ═══════════════════════════════════════════════════════════════════════════
// 🔴 THE WAITING LIST LIVES IN `tokens-orphelins/attente.mjs`, WITH ALL ITS
// DOCTRINE — extracted by task 8 of S2, the data and its justification
// together. What one needs to know here fits in one word: the check requires
// EQUALITY between the set of orphans and this list, so it fails in
// BOTH DIRECTIONS — an orphan absent from the list, as well as an entry of the list
// that gained a caller. It is the second half that makes it SELF-CLEANING.
// ═══════════════════════════════════════════════════════════════════════════

const args = process.argv.slice(2);
const iRacine = args.indexOf('--racine');
const racine = iRacine === -1 ? process.cwd() : args[iRacine + 1];
const sansExclusion = args.includes('--sans-exclusion');

function fichiersCss(repertoire, acc = []) {
    if (!existsSync(repertoire)) return acc;
    for (const entree of readdirSync(repertoire, { withFileTypes: true })) {
        const chemin = join(repertoire, entree.name);
        if (entree.isDirectory()) fichiersCss(chemin, acc);
        else if (entree.name.endsWith('.css')) acc.push(chemin);
    }
    return acc;
}

const sources = SOURCES.map((s) => join(racine, s));
for (const [i, chemin] of sources.entries()) {
    if (!existsSync(chemin)) {
        console.error(`${SOURCES[i]} is missing: nothing was measured, this is not a success.`);
        process.exit(2);
    }
}
const porte = join(racine, PORTE);

// The "used" scope: every sheet of `client/src/` except the sources
// (and their common entry point, `tokens.css`), and
// every HTML surface except those of `EXCLUS`. The surfaces are included because
// an inline `<style>` uses tokens just like a sheet.
//
// 🔴 THE SURFACES COME FROM THE VITE ENTRIES, NEVER FROM A `client/*.html`.
// A scan of the directory catches `probe-coalesced.html`,
// `recette/latency-test.html` and `recette/scroll-test.html`, which are
// BENCH INSTRUMENTS: they do not come out of the build, they do not receive
// the bootstrap, and a `var(--…)` written in one of them must not count as
// a production caller. `client/vite.config.ts:9-13` already carries this
// warning, measured on `connexion.html`.
// ⚠️ The list is READ from `vite.config.ts`, not copied: it is the same
// reason that forbids this script from having its own copy of the values. Node
// v24.9.0 imports the `.ts` natively, as for `tokens.ts`.
const feuilles = fichiersCss(join(racine, 'client/src')).filter(
    (f) => !sources.includes(f) && f !== porte,
);
const surfaces = Object.values(configVite.build.rollupOptions.input).map((n) =>
    join(racine, 'client', n),
);

const employePar = new Map();
const balayes = [];
for (const chemin of [...feuilles, ...surfaces]) {
    const relatif = relative(racine, chemin).split('\\').join('/');
    if (!sansExclusion && EXCLUS.has(relatif)) continue;
    balayes.push(relatif);
    for (const token of tokensReferences(readFileSync(chemin, 'utf8'))) {
        if (!employePar.has(token)) employePar.set(token, []);
        employePar.get(token).push(relatif);
    }
}

// 🔴 THE SECOND WAY TO "USE" A TOKEN — set by the JS, never read by
// a `var()`. See `tokens-orphelins/js.mjs` for the selection rule (and
// why it excludes the `*.test.ts`) and for the raison d'être of this block:
// without it, a token DECLARED and set by `poserToken(...)` but referenced by
// NO CSS stays invisible to the two inclusions below — it is
// exactly the blind spot that kept `accent-dom.ts` from declaring
// `--accent-fenetre` (D-A1-2, probe H1 of August 21st, 2026).
const posesJs = tokensPosesParLeJs(racine);
for (const [token, files] of posesJs) {
    if (!employePar.has(token)) employePar.set(token, []);
    for (const relatif of files) employePar.get(token).push(`${relatif} (JS, poserToken)`);
}

const texteSource = sources.map((chemin) => readFileSync(chemin, 'utf8')).join('\n');
const declares = tokensDeclares(texteSource);
const orphelins = [...declares].filter((t) => !employePar.has(t)).sort();
const nonDeclares = [...employePar.keys()].filter((t) => !declares.has(t)).sort();

console.log(`sources       : ${SOURCES.join(', ')} — ${declares.size} token(s) declared`);
console.log(
    `scope         : ${balayes.length} file(s) — ${employePar.size} token(s) used`,
);
console.log(`  scanned: ${balayes.join(', ')}`);
console.log(
    `set by the JS: ${posesJs.size} token(s) (poserToken, outside *.test.ts) — ` +
        `${[...posesJs.keys()].sort().join(', ') || 'none'}`,
);
for (const [chemin, raison] of EXCLUS) {
    console.log(`  ${sansExclusion ? 'INCLUDED (--sans-exclusion)' : 'excluded'} : ${chemin} — ${raison}`);
}

// ① used ⊆ declared
console.log(`\ninclusion ① — every var(--…) is declared: ${nonDeclares.length} gap(s)`);
for (const token of nonDeclares) {
    console.log(`  UNDECLARED  ${token}  used by ${employePar.get(token).join(', ')}`);
}

// ② declared ⊆ used, up to the waiting list — and EQUALITY, not inclusion.
const nouveaux = orphelins.filter((t) => !EN_ATTENTE_D_APPELANT.has(t));
const aRetirer = [...EN_ATTENTE_D_APPELANT.keys()].filter((t) => employePar.has(t)).sort();
console.log(
    `inclusion ② — every declared token has a caller: ${orphelins.length} orphan(s), ` +
        `of which ${orphelins.length - nouveaux.length} declared as waiting`,
);
for (const token of nouveaux) {
    console.log(`  NEW ORPHAN  ${token}  declared and called by no one`);
}
for (const token of aRetirer) {
    console.log(
        `  TO REMOVE FROM THE LIST  ${token}  now has a caller ` +
            `(${employePar.get(token).join(', ')}): the waiting list must shrink`,
    );
}

// ③ NO ENTRY NAMES AN ALREADY CLOSED SUB-BLOCK — the partial mitigation of
//   re-labelling, built in sub-block S3 (task 7).
//
// 🔴 WHAT IT CATCHES, AND WHAT NO OTHER ASSERTION SEES: the two
// inclusions above compare SETS OF TOKEN NAMES. Moving an
// entry from "S2" to "S3" moves neither of them — it is the
// weakest point of the arrangement, and the door through which a waiting
// list loosens indefinitely without any command saying so.
//
// ⚠️ IT IS PARTIAL, and the word is weighed: it judges the NAMED SUB-BLOCK,
// never the CONTENT of the reason, and it depends on a list of closed sub-blocks
// KEPT BY HAND — a sub-block that does not declare itself there neutralises it. The
// full doctrine, and the objection that it only guards one entry after S3,
// live next to the list, in `tokens-orphelins/attente.mjs`.
const surSousBlocClos = [...EN_ATTENTE_D_APPELANT.entries()]
    .filter(([, entree]) => SOUS_BLOCS_CLOS.has(entree.sousBloc))
    .sort(([a], [b]) => a.localeCompare(b));
console.log(
    `\ninclusion ③ — no entry names a closed sub-block ` +
        `(${[...SOUS_BLOCS_CLOS].join(', ')}): ${surSousBlocClos.length} gap(s)`,
);
for (const [token, entree] of surSousBlocClos) {
    console.log(
        `  CLOSED SUB-BLOCK  ${token}  named ${entree.sousBloc}, which is closed: ` +
            `decide or re-label with its reason`,
    );
}

const echecs = nonDeclares.length + nouveaux.length + aRetirer.length + surSousBlocClos.length;
console.log(`\ntotal: ${echecs} gap(s)`);
if (echecs === 0) {
    console.log(
        `the three assertions hold; ${EN_ATTENTE_D_APPELANT.size} token(s) remain ` +
            `waiting for a caller, named in tokens-orphelins/attente.mjs`,
    );
}
process.exit(echecs > 0 ? 1 : 0);
