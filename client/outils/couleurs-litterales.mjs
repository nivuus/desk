#!/usr/bin/env node
// Check §7.2 of spec ⑥ — NO LITERAL COLOUR OUTSIDE
// `tokens/couleurs.css` (`tokens.css` before the extraction of task 6,
// August 25th, 2026).
//
// Exits 1 as soon as a colour is hardcoded anywhere other than in the single
// source. This script carries NO design rule: it scans, it counts, it
// names. Any rule would belong to `client/src/design/*.ts`, which is
// typechecked and tested (same clause as `client/src/connexion.ts:5-9`).
//
// ────────────────────────────────────────────────────────────────────────────
// WHY THIS CHECK CARRIES THREE GUARDS, AND WHAT EACH ONE REALLY HOLDS
// (divergence D8 of the S1 plan, then mutations played on August 19th, 2026).
//
// §7.2 of the spec prescribes looking for the keywords `black`/`white`/`red`.
// On the untouched tree, the raw search raises two false positives, both of them
// FRENCH prose (the comments were in French at the time):
//
//     $ grep -rnoE 'black|white|\bred\b' client/src --include="*.css" --include="*.ts"
//     client/src/resize.ts:7:red
//     client/src/resize.test.ts:30:red
//
// `resize.ts:7` said « re**d**éclenche que si l'élément change encore de
// taille »: the `\b` following `red` is satisfied because the next
// character is `é`, which ASCII word classes do not count as a letter.
// A check born red on two innocent comments is a check
// that will be loosened — and spec §11 names this failure mode.
//
// ⚠️ THREE guards hold it, and they OVERLAP on this precise case:
//
//   ① blanking comments — the `/* … */` everywhere, and in TypeScript
//     the `//` that do not follow a `:` (without this reservation, `http://`
//     would erase real code, and could therefore HIDE a colour instead of
//     revealing one). Line breaks are kept: the line numbers stay
//     right.
//   ② keywords are NEVER looked for in a `.ts`.
//   ③ keywords are only looked for on the VALUE side of a CSS declaration
//     (`property: …;`).
//
// Mutations played, with their report — it is the only way to know which one
// carries what, and the first one CONTRADICTED the initial draft of this header,
// which credited ① with handling D8:
//
//   ① removed alone            → 11 occurrences, `resize.ts` STILL absent
//   ② removed alone            → 11 occurrences, `resize.ts` STILL absent
//   ① and ② removed together   → 11 occurrences, `resize.ts` STILL absent
//   ①, ② and ③ removed         → 13, with `resize.ts:7` AND `resize.test.ts:30`
//                                — the two exact occurrences of D8
//   ① removed, dedicated CSS case → a commented-out `/* background: #ff0000; */`
//                                comes up
//
// In other words: ① exists for COMMENTED colours in CSS — not for D8,
// which ② and ③ already cover. None of the three is dead, and none is removed
// "since the others cover it": that is true one at a time, false all together.
//
// The `#rrggbb`, `rgb(`, `hsl(` notations, for their part, are looked for EVERYWHERE:
// none collides with French prose.
//
// ────────────────────────────────────────────────────────────────────────────
// DECLARED SCOPE, AND WHAT THIS CHECK DOES NOT SEE.
//
// • In a `.ts`, a bare `red` is indistinguishable from prose or from an
//   identifier: keywords are not looked for there. An
//   `el.style.background = 'red'` would therefore pass. It is a measured limit
//   (D8), not an oversight — and hexadecimal notations, for their part, are seen there.
// • The scanned HTML files are the VITE ENTRIES, read from `vite.config.ts`, and not
//   all the `client/*.html`. `client/probe-coalesced.html` carries four
//   literal colours (`#222`, `#eee`, `#f4f4f4`) and is NOT part of the product:
//   it is not a Vite entry, it does not go out into `dist/`, and the S1 plan
//   (D11) files it with `client/recette/*.html` among the bench instruments.
//   Scanning it would make this check be born red on out-of-scope code —
//   exactly the pressure to loosen that measurement D8 rules out.
//   ⚠️ Corollary: a new page not declared in `vite.config.ts` is
//   scanned by nothing. It is the same blind spot `vite.config.ts:9-13`
//   already documents for the build itself.
// • ⚠️ THIS CHECK COUNTS ELEVEN, WHERE THE S1 PLAN ANNOUNCED NINE, and the
//   two of the gap are `style.css:3-4` — `--surface: #0b0d10` and
//   `--text: #e6e8eb`. They are indeed token DECLARATIONS, but they
//   live in `style.css`, not in `tokens.css`: the exclusion of §7.2 is
//   by FILE, never by role, and a declaration outside the single source
//   is precisely the drift this check exists to see. ✅ Green DID
//   ARRIVE at task 9 (`ab9e9b9`), which migrated the two declarations:
//   this check has returned ZERO on 46 files since. The plan also attributed
//   a count of eleven to a comment handling defect: the
//   mutations above refute it — such a defect would return THIRTEEN.
// • 🔴 FOUR REQUIREMENTS THAT CANNOT HOLD TOGETHER, and the fourth
//   gives way. The S1 plan asks at the same time: (T1) that the `*.test.ts` be
//   SCANNED, "a colour hardcoded in a test being a duplicated
//   value like any other"; (T5) that `contraste.test.ts` carry the vectors
//   `#000000`, `#ffffff` and `#808080`, whose value is set by WCAG 2.1
//   and not by our tokens — that is what keeps it from validating the product
//   against itself; (T9) that `reprise.test.ts` compare `--fond-0` to
//   `#0b0d10` EXACTLY, which is the comparison that PROVES the carry-over;
//   and (T9) that this check return ZERO at the end of the sub-block. The vectors of
//   T5 and the comparison of T9 are MANDATORY literals: forbidding them
//   would make those two tasks impossible.
//   ⚠️ The exclusion is therefore set AS NARROWLY AS POSSIBLE: `client/src/design/*.test.ts`
//   only. Every other test of the package stays scanned — a colour in
//   `resize.test.ts` or `webrtc.test.ts` is still refused, and none
//   carries one today. Report of the gap, on the tree of August 19th, 2026: without
//   this exclusion the check returns 46, of which 35 come from the only two
//   test files of the base layer and 11 from `style.css`, the real target.
//   ⚠️ What it costs: a base-layer test that copied a product colour
//   for no reason would no longer be caught. It is a review rule, in the
//   same way as "use `--bord-fort` where it carries information".
// • This check says no colour is hardcoded. It says NOTHING about
//   the correctness of the token used instead — it is a review rule
//   (spec §8).

import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, relative } from 'node:path';

const NOTATIONS = /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/g;
const MOTS_CLES = /\b(black|white|red)\b/g;
const DECLARATION = /(?:^|[;{])\s*[-a-zA-Z][-a-zA-Z0-9]*\s*:\s*([^;{}]*)/g;

/** Blanks an interval while keeping the line breaks, hence the line numbers. */
function blanchir(texte, debut, fin) {
    const morceau = texte.slice(debut, fin).replace(/[^\n]/g, ' ');
    return texte.slice(0, debut) + morceau + texte.slice(fin);
}

function sansCommentaires(texte, extension) {
    let sortie = texte;
    const blocs = [];
    // `/* … */`, common to CSS, TypeScript and the HTML `<style>`.
    for (const m of texte.matchAll(/\/\*[\s\S]*?\*\//g)) {
        blocs.push([m.index, m.index + m[0].length]);
    }
    if (extension === '.html') {
        for (const m of texte.matchAll(/<!--[\s\S]*?-->/g)) {
            blocs.push([m.index, m.index + m[0].length]);
        }
    }
    if (extension === '.ts') {
        // ⚠️ The `(?<![:/])` sets aside `http://` and `ws://`: blanking up to the
        // end of the line from the `//` of a URL scheme would erase real
        // code, and could therefore HIDE a colour instead of revealing one.
        for (const m of texte.matchAll(/(?<![:/])\/\/[^\n]*/g)) {
            blocs.push([m.index, m.index + m[0].length]);
        }
    }
    for (const [debut, fin] of blocs) sortie = blanchir(sortie, debut, fin);
    return sortie;
}

const ligneDe = (texte, index) => texte.slice(0, index).split('\n').length;

function occurrences(texte, extension) {
    const propre = sansCommentaires(texte, extension);
    const lignes = propre.split('\n');
    const trouvees = [];

    for (const m of propre.matchAll(NOTATIONS)) {
        trouvees.push({ ligne: ligneDe(propre, m.index), motif: m[0] });
    }
    // The keywords: on the value side of a declaration, and never in a `.ts`.
    if (extension !== '.ts') {
        for (const d of propre.matchAll(DECLARATION)) {
            const value = d[1];
            const debutValeur = d.index + d[0].length - value.length;
            for (const m of value.matchAll(MOTS_CLES)) {
                trouvees.push({
                    ligne: ligneDe(propre, debutValeur + m.index),
                    motif: m[0],
                });
            }
        }
    }
    return trouvees
        .sort((a, b) => a.ligne - b.ligne)
        .map((o) => ({ ...o, texte: (lignes[o.ligne - 1] ?? '').trim() }));
}

function fichiersSous(repertoire, extensions) {
    if (!existsSync(repertoire)) return [];
    const trouves = [];
    for (const entree of readdirSync(repertoire)) {
        const chemin = join(repertoire, entree);
        if (statSync(chemin).isDirectory()) {
            trouves.push(...fichiersSous(chemin, extensions));
        } else if (extensions.some((e) => entree.endsWith(e))) {
            trouves.push(chemin);
        }
    }
    return trouves.sort();
}

/** The PRODUCT HTML files: the entries declared to Vite, and nothing else. */
function entreesVite(racine) {
    const config = join(racine, 'vite.config.ts');
    if (!existsSync(config)) return [];
    const bloc = readFileSync(config, 'utf8').match(/input:\s*\{([\s\S]*?)\}/);
    if (!bloc) return [];
    return [...bloc[1].matchAll(/['"]([^'"]+\.html)['"]/g)]
        .map((m) => join(racine, m[1]))
        .filter((c) => existsSync(c))
        .sort();
}

const args = process.argv.slice(2);
const iRacine = args.indexOf('--racine');
const racine = iRacine === -1 ? 'client' : args[iRacine + 1];

// ⚠️ TWO EXCLUSIONS, AND THE SECOND IS A DELIBERATE DIVERGENCE FROM THE PLAN.
// See the box "FOUR REQUIREMENTS THAT CANNOT HOLD TOGETHER".
//
// 🔴 `tokens/couleurs.css` ALONE, SINCE THE EXTRACTION OF TASK 6
// (August 25th, 2026): it is now the only file where a literal colour
// is LEGITIMATE. `tokens.css` (the entry point, which now only imports
// its two children) and `tokens/echelles.css` (no colour, only
// lengths and durations) need no exclusion: neither one nor
// the other carries a `#…`/`rgb(`/`hsl(` notation, so excluding them would be
// inert — but it would also be WRONG BY CONSTRUCTION: the exclusion is "by
// FILE, never by role" (see the box above), and the role that
// justifies it now belongs to `couleurs.css` alone.
const exclus = new Set([join(racine, 'src/design/tokens/couleurs.css')]);
const estTestDuSocle = (chemin) =>
    chemin.startsWith(join(racine, 'src/design/')) && chemin.endsWith('.test.ts');
const aBalayer = [
    ...fichiersSous(join(racine, 'src'), ['.css', '.ts']),
    ...entreesVite(racine),
].filter((c) => !exclus.has(c) && !estTestDuSocle(c));

let total = 0;
for (const chemin of aBalayer) {
    const texte = readFileSync(chemin, 'utf8');
    const extension = chemin.slice(chemin.lastIndexOf('.'));
    for (const o of occurrences(texte, extension)) {
        console.log(`${relative('.', chemin)}:${o.ligne}: ${o.texte}`);
        total += 1;
    }
}

console.log(`files scanned: ${aBalayer.length}`);
console.log(`literal colours: ${total}`);
if (total > 0) {
    console.log('→ they must live in client/src/design/tokens/couleurs.css');
}
process.exit(total > 0 ? 1 : 0);
