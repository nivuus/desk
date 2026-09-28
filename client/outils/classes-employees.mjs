#!/usr/bin/env node
// Check §7.9 — EVERY CLASS USED IS DECLARED, AND A PRIMITIVE REACHES
// THE PRODUCT.
//
// 🔴 THIS CHECK IS NOT IN THE SPEC: it is an ADDITION of sub-block S3,
// declared as such. It exists because NONE of the seven previous checks
// can see the gap S2 itself named — "the eighteen tokens taken out
// of the list have a WRITTEN caller, not a RENDERED pixel". And it is not a
// drafting gap, it is structural:
//
//   - §7.6 counts `var(--…)` in FILES. Setting
//     `class="bouton bouton--principal"` on a page adds no `var()`:
//     the check counts exactly the same used tokens before and after.
//   - §7.3 only checks the LOADING of a sheet, never its use — its
//     own header says so.
//   - §7.2 only sees colours.
//
// ── THE THREE ASSERTIONS, COUNTED SEPARATELY ──────────────────────────────
//
//   ① used ⊆ declared — a `class="bouton--principale"` (typo)
//      is a rule that applies to nothing, and the browser says NOTHING:
//      the page stays up and wrong. No other check sees it.
//   ② A — REACHABILITY: EACH of the three surfaces of the product uses at
//      least one family of primitives. It is this assertion, and it alone,
//      that closes S2's gap. It was born RED on the untouched tree —
//      measured on August 20th, 2026, `grep -n 'class=' client/{index,shell,connexion}
//      .html` returned NO line.
//
//      🔴 HARDENED BY TASK 3 OF SUB-BLOCK S4, AND IT IS BORN RED AGAIN. It
//      required "AT LEAST ONE surface, AT LEAST ONE family": that quantifier
//      was right as long as only one surface had been reworked, and it became
//      a ceiling from the second — two surfaces out of three dressed left it
//      green, and the third could stay bare forever without
//      any command saying so. That is exactly what happened: at the
//      end of S3, `client/index.html` returned "no family", and the check
//      was GREEN. The session window is reworked by S4; the quantifier
//      follows.
//
//      ⚠️ IT IS HARDENED BEFORE THE TASK THAT TURNS IT OFF, AND THAT IS THE POINT.
//      Hardening it in the same commit as the dressing would make it green from its
//      birth, hence never seen red on the tree — a check never
//      seen red is not a check. The tree itself is its proof
//      of reachability, as in S3.
//
//      ⚠️ THE OUTPUT NAMES THE SURFACE THAT USES NOTHING, not only the fact
//      that one exists: "surface X uses no family" is
//      actionable, "no surface uses any" was not.
//   ③ B — each of the primitive families appears in the gallery
//      `client/primitives.html`. It takes a share of S2's legacy item no. 4: "a
//      gallery that stopped rendering a whole family would be caught
//      by no check".
//
// ⚠️ It carries NO PARSING RULE: `classesDeclarees`,
// `classesEmployeesHtml`, `classesEmployeesTs` and `classesDeclareesEnLigne`
// live in `client/src/design/classes.ts`, which is typechecked and tested.
// It is the convention of `tokens-orphelins.mjs`, and the same reason.
//
// ⚠️ THE FAMILIES ARE NOT ENUMERATED HERE: they are DERIVED from the files
// of `client/src/design/primitives/`, one file per family. A fifth
// family therefore enters ② and ③ without a line of this script changing, and
// a copied list cannot diverge from what it describes.
//
// ═══════════════════════════════════════════════════════════════════════════
// 🔴 WHAT THIS CHECK DOES NOT SAY.
//
//   - THE REVERSE DIRECTION — every declared class is used — IS NOT TAKEN.
//     It would require a SECOND waiting list (`.bouton--discret` may have
//     no caller), and one more list for a family already guarded by its
//     gallery would be a cost with no return. It is `primitives.html` and
//     the eye that hold that direction (spec §7.8).
//   - A CLASS COMPUTED AT RUN TIME IS INVISIBLE — `el.className = x`,
//     a concatenation, a `classList.toggle(nom)`. The rule that makes this
//     check useful is the convention: classes are written as LITERALS,
//     preferably in the HTML.
//   - IT JUDGES NO APPEARANCE. That a class is set does not say
//     it renders well. Human judgement remains whole.
// ═══════════════════════════════════════════════════════════════════════════

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { basename, dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
    classesDeclarees,
    classesDeclareesEnLigne,
    classesEmployeesHtml,
    classesEmployeesTs,
} from '../src/design/classes.ts';
import configVite from '../vite.config.ts';

const args = process.argv.slice(2);
const iRacine = args.indexOf('--racine');
const racine = iRacine === -1 ? process.cwd() : args[iRacine + 1];

/** The surfaces of the PRODUCT — those a user sees.
 *
 * 🔴 `client/hub.html` ENTERED IT ON AUGUST 31ST, 2026, AND IT WAS NOT THERE: the
 * hub has been served at the root since batch 14, so it is THE surface
 * the user reaches, and yet it was outside any §7.9 check.
 *
 * 🔴 AND `client/shell.html` LEFT IT THE SAME DAY (task 9): the shell page
 * became a mere redirect — the body of `shell-page.ts` fits in one
 * `import` and one `location.replace`, everything else in it being comment;
 * `wc -l` for the size, never a number copied here. It
 * no longer carries ANY class — neither on `<html>`, nor on `<body>`, which only contains
 * a `<script>`. Keeping it here would make it count as a BARE surface
 * (no primitive family used), which would fail assertion ②
 * for a page that paints nothing any more: it is not a missing primitive,
 * it is the absence of any markup.
 */
const SURFACES_PRODUIT = [
    'client/index.html',
    'client/hub.html',
    'client/connexion.html',
];
/** The gallery of human judgement, the one ③ queries. */
const GALERIE_PRIMITIVES = 'client/primitives.html';

const rel = (chemin) => relative(racine, chemin).split('\\').join('/');

function fichiersSuffixes(repertoire, suffixe, acc = []) {
    if (!existsSync(repertoire)) return acc;
    for (const entree of readdirSync(repertoire, { withFileTypes: true })) {
        const chemin = join(repertoire, entree.name);
        if (entree.isDirectory()) fichiersSuffixes(chemin, suffixe, acc);
        else if (entree.name.endsWith(suffixe)) acc.push(chemin);
    }
    return acc;
}

const src = join(racine, 'client/src');
if (!existsSync(src)) {
    console.error("client/src is missing: nothing was measured, this is not a success.");
    process.exit(2);
}

const surfaces = Object.values(configVite.build.rollupOptions.input).map((n) =>
    join(racine, 'client', n),
);

// ── THE DECLARED SET ──────────────────────────────────────────────────────
const declarePar = new Map();
const ajouterDeclarees = (classes, source) => {
    for (const nom of classes) {
        if (!declarePar.has(nom)) declarePar.set(nom, []);
        declarePar.get(nom).push(source);
    }
};

for (const chemin of fichiersSuffixes(src, '.css')) {
    ajouterDeclarees(classesDeclarees(readFileSync(chemin, 'utf8')), rel(chemin));
}
// ⚠️ INLINE `<style>` ARE MANDATORY IN THIS SET:
// `client/design.html` declares inline the six classes that `galerie.ts`
// uses. Omitting them would make this check be born RED on correct code,
// that is, the pressure to loosen that this repository rules out.
for (const chemin of surfaces) {
    ajouterDeclarees(classesDeclareesEnLigne(readFileSync(chemin, 'utf8')), rel(chemin));
}

// ── THE USED SET ──────────────────────────────────────────────────────────
const employePar = new Map();
const ajouterEmployees = (classes, source) => {
    for (const nom of classes) {
        if (!employePar.has(nom)) employePar.set(nom, []);
        employePar.get(nom).push(source);
    }
};

for (const chemin of surfaces) {
    ajouterEmployees(classesEmployeesHtml(readFileSync(chemin, 'utf8')), rel(chemin));
}
for (const chemin of fichiersSuffixes(src, '.ts')) {
    if (chemin.endsWith('.test.ts')) continue;
    ajouterEmployees(classesEmployeesTs(readFileSync(chemin, 'utf8')), rel(chemin));
}

// ── THE FAMILIES, DERIVED FROM THE FILES ──────────────────────────────────
const familles = fichiersSuffixes(join(src, 'design/primitives'), '.css')
    .sort()
    .map((chemin) => ({
        nom: basename(chemin, '.css'),
        classes: classesDeclarees(readFileSync(chemin, 'utf8')),
    }));
const classesDePrimitive = new Set(familles.flatMap((f) => [...f.classes]));

// ── THE REPORT, ALWAYS PRINTED, SUCCESS INCLUDED ──────────────────────────
// "A drift check whose value is never read only serves to pass"
// (`poids-css.mjs`).
console.log(`declared: ${declarePar.size} class(es)`);
console.log(`used: ${employePar.size} class(es)`);
console.log(`primitive families: ${familles.map((f) => f.nom).join(', ')}`);

// ① used ⊆ declared
const nonDeclarees = [...employePar.keys()].filter((n) => !declarePar.has(n)).sort();
console.log(`\n① every used class is declared: ${nonDeclarees.length} gap(s)`);
for (const nom of nonDeclarees) {
    console.log(`  UNDECLARED  ${nom}  used by ${employePar.get(nom).join(', ')}`);
}

// ② A — a primitive reaches a surface of the product
console.log('\n② A — the primitives reach the PRODUCT, EACH of the three surfaces:');
const nues = [];
for (const surface of SURFACES_PRODUIT) {
    const chemin = join(racine, surface);
    if (!existsSync(chemin)) {
        console.log(`  ${surface}: NOT FOUND`);
        nues.push(surface);
        continue;
    }
    const employees = classesEmployeesHtml(readFileSync(chemin, 'utf8'));
    const parFamille = familles
        .filter((f) => [...f.classes].some((c) => employees.has(c)))
        .map((f) => f.nom);
    if (parFamille.length === 0) nues.push(surface);
    console.log(
        `  ${surface}: ${parFamille.length === 0 ? 'no family' : parFamille.join(', ')}`,
    );
}
// 🔴 A BARE SURFACE IS A FAILURE, AND IT IS NAMED. The count is that of the
// surfaces WITHOUT a family, plus that of the surfaces NOT FOUND: a surface
// removed from `SURFACES_PRODUIT` without being removed from disk would return `0 gaps`
// while measuring nothing any more, and that is the pattern of the vacuous check.
const echecA = nues.length;
for (const surface of nues) {
    console.log(
        `  ${surface} uses NO primitive family: ` +
            'the primitives have a WRITTEN caller there, not a RENDERED pixel',
    );
}

// ③ B — each family is rendered by the gallery
const galerie = join(racine, GALERIE_PRIMITIVES);
const employeesGalerie = existsSync(galerie)
    ? classesEmployeesHtml(readFileSync(galerie, 'utf8'))
    : new Set();
const absentes = familles.filter((f) => ![...f.classes].some((c) => employeesGalerie.has(c)));
console.log(`\n③ B — each family is rendered by ${GALERIE_PRIMITIVES}:`);
for (const f of familles) {
    const rendues = [...f.classes].filter((c) => employeesGalerie.has(c)).length;
    console.log(`  ${f.nom}: ${rendues} class(es) used out of ${f.classes.size} declared`);
}
for (const f of absentes) {
    console.log(`  family ${f.nom.toUpperCase()} missing from ${GALERIE_PRIMITIVES}`);
}

const echecs = nonDeclarees.length + echecA + absentes.length;
console.log(`\ntotal: ${echecs} gap(s)`);
process.exit(echecs > 0 ? 1 : 0);
