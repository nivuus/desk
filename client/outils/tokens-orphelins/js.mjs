// THE "SET BY THE JS" HALF OF CHECK §7.6 — sub-block A1, task 7 of the
// `legs-sans-vm` project (August 25th, 2026).
//
// 🔴 WHAT `tokens-orphelins.mjs` COULD NOT SEE, AND WHAT MADE THE
// LEGACY ITEM OF `accent-dom.ts` INVISIBLE TO IT. Check §7.6 only knew ONE
// way to "use" a token: a `var(--…)` in CSS (or in an
// inline `<style>` of an HTML surface). But `accent-dom.ts` sets
// `--accent-fenetre` through `acces.poserToken(TOKEN_ACCENT, …)`, which calls
// `document.documentElement.style.setProperty(nom, value)` — NO `var()`
// appears anywhere on this path. A token DECLARED, set at run time,
// and referenced by NO `var()`, was therefore an ORPHAN in the sense of the old
// check, although it was not: the comment of `accent-dom.ts`
// had seen it (D-A1-2, probe H1 of August 21st, 2026) and had chosen NOT to
// declare the token rather than live with a permanently blind
// check. This file closes that hole: it gives the check a SECOND
// way to "use" a token, symmetric to the first.
//
// ═══════════════════════════════════════════════════════════════════════════
// 🔴 SELECTION RULE, STATED BEFORE THE SCAN — AND THE REASON THAT EXCLUDES
// THE TESTS.
//
// Two passes, over the SAME scope of files:
//
//   ① COLLECT THE TOKEN CONSTANTS: any declaration
//      `const <NOM> = '<--token>'` (or in double quotes), anywhere in
//      this scope — it is the pattern `export const TOKEN_ACCENT =
//      '--accent-fenetre';` of `accent-dom.ts`. The resulting table maps an
//      IDENTIFIER (`TOKEN_ACCENT`) to a TOKEN NAME (`--accent-fenetre`).
//
//   ② FIND THE CALLS: any `poserToken(` followed either by a
//      `'--token'` literal directly, or by an identifier ① resolved. It is
//      the trace of a `setProperty` to come — the ONLY trace, since
//      `poserToken` is an interface method name
//      (`AccesTokens::poserToken`, `accent-dom.ts`), never a keyword of the
//      language: a future second caller (another `…-dom.ts`) would be
//      caught by the SAME rule, without modification.
//
// 🔴 THE SCOPE IS `client/src/**/*.ts`, **MINUS EVERY `*.test.ts`**, AND
// IT IS THE TRAP NAMED BY THE TASK BRIEF: `accent-dom.test.ts` sets
// ITS OWN fake implementation of `poserToken` to spy on the calls
// (`poserToken: (nom, value) => { … }`, an object property DEFINITION,
// never a CALL — it therefore already does not match ①, `poserToken(`
// requiring a PARENTHESIS right after the name, where the test writes
// `poserToken:`). But a future test could very well write
// `objet.poserToken('--test-token', 'x')` to exercise an edge case, and
// THAT WOULD BE A CALL — the same textual form as a real one. A check that
// went red on its OWN test would be "worse than no check" (task
// brief): nobody would believe it the time it was right. Hence
// excluding the whole scope, not a list of patterns to recognise and
// set aside case by case.
//
// ⚠️ WHAT THIS RULE DOES NOT SEE, AND IT IS AN ACCEPTED LIMIT: a call
// through an ALIAS (`const p = acces.poserToken; p(TOKEN_ACCENT, …)`) or through
// DESTRUCTURING (`const { poserToken: pose } = acces; pose(…)`) would stay
// invisible — the rule looks for the TEXT `poserToken(`, never a flow
// analysis. The only production caller to date (`accent-dom.ts`) uses
// neither.
// ═══════════════════════════════════════════════════════════════════════════

import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';

/** All the `.ts` under `repertoire`, excluding `*.test.ts`. */
function fichiersTs(repertoire, acc = []) {
    if (!existsSync(repertoire)) return acc;
    for (const entree of readdirSync(repertoire, { withFileTypes: true })) {
        const chemin = join(repertoire, entree.name);
        if (entree.isDirectory()) fichiersTs(chemin, acc);
        else if (entree.name.endsWith('.ts') && !entree.name.endsWith('.test.ts')) acc.push(chemin);
    }
    return acc;
}

const RE_CONSTANTE = /\bconst\s+([A-Za-z_$][\w$]*)\s*=\s*['"](--[\w-]+)['"]/g;
const RE_APPEL = /\bposerToken\(\s*(?:['"](--[\w-]+)['"]|([A-Za-z_$][\w$]*))/g;

/**
 * The tokens set by `poserToken(...)` in the PRODUCTION scope
 * (`client/src/**\/*.ts`, `*.test.ts` excluded) → the list of files,
 * relative to `racine`, where the call was found.
 *
 * ⚠️ IDENTIFIER RESOLUTION IS GLOBAL TO THE SCOPE, NOT PER FILE:
 * a constant declared in one file and a call in another (a future
 * shared import) are reconciled. This repository today has only one
 * caller and one constant, BOTH in `accent-dom.ts` — the
 * global scope changes nothing there, but restricting it to the file would break the
 * day a second module imported the constant of a first one.
 */
export function tokensPosesParLeJs(racine) {
    const files = fichiersTs(join(racine, 'client/src'));

    const constantes = new Map();
    for (const chemin of files) {
        const texte = readFileSync(chemin, 'utf8');
        for (const m of texte.matchAll(RE_CONSTANTE)) constantes.set(m[1], m[2]);
    }

    const poses = new Map();
    for (const chemin of files) {
        const relatif = relative(racine, chemin).split('\\').join('/');
        const texte = readFileSync(chemin, 'utf8');
        for (const m of texte.matchAll(RE_APPEL)) {
            const token = m[1] ?? constantes.get(m[2]);
            if (!token) continue;
            if (!poses.has(token)) poses.set(token, []);
            poses.get(token).push(relatif);
        }
    }
    return poses;
}
