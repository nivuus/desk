#!/usr/bin/env node
// Check §7.3 of spec ⑥ — EVERY BUILT SURFACE CARRIES THE TOKENS.
//
// To be run AFTER `npm run build`. It is the check the framing calls for
// when it requires tokens "shared between hub and session window": a
// convention guarantees nothing, a command does.
//
// ────────────────────────────────────────────────────────────────────────────
// TWO ASSERTIONS, EVALUATED AND COUNTED SEPARATELY — AND THAT IS THE POINT.
//
//   A — each `dist/*.html` carries at least one `<link rel="stylesheet">`;
//   B — for EACH page, at least one of the sheets IT links declares
//       `--fond-0`.
//
// B is strictly stronger than the letter of the spec, which writes "the SET
// of emitted CSS must contain the `--fond-0` declaration". Taken that way, a page
// could link a sheet without tokens and pass, as long as ANOTHER page
// links one that has them. Resolving the `href` page by page only costs that
// resolution, and closes the hole.
//
// ⚠️ B IS ONLY EVALUATED ON THE PAGES THAT PASS A, deliberately. A page
// without any link fails A; counting it in B too would report the same
// failure twice and, above all, would HIDE B behind A — the day A turns
// green, nobody would know whether B had ever been exercised. It is the red
// sub-block P2 had to replay after the fact for this exact reason.
// B WAS SEEN RED on `dist/index.html` — a page that passed A — on
// the untouched tree of August 19th, 2026, at commit `71f3c36`: the two assertions
// are therefore really independent, and one SEES it in the same report.
// ⚠️ Since task 11 (`72fe0f1`), BOTH assertions are green and B is
// evaluated on FOUR pages instead of one. This paragraph is a DATED report,
// hence true as history: do not read it in the present tense.
//
// 🔴 `shell.html` IS EXCLUDED FROM ASSERTION A SINCE AUGUST 31ST, 2026
// (task 9). It became a PURE REDIRECT — the body of
// `shell-page.ts` fits in one `import` and one `location.replace`, all the
// rest being comment; `wc -l` for the size, never a number
// copied here. "NO `<link>` HERE, on purpose" says its own comment —
// the page paints nothing, and linking a sheet would flash a style before
// the redirect. Without this exclusion, this assertion would be WRONG BY
// DESIGN, forever, on a page that behaves exactly as
// intended — the pattern `tokens-orphelins.mjs` names "the reason why
// a file CANNOT make the check fail, never the reason why
// it would be in the way". It stays counted in `built pages` (it
// does come out of `npm run build`, `vite.config.ts` keeps it in
// `rollupOptions.input` for already installed PWAs) — only assertion A
// ignores it, B being unable to judge on zero links anyway.
//
// ────────────────────────────────────────────────────────────────────────────
// HONEST SCOPE. This check verifies that a surface LOADS the tokens, not
// that it USES them. A page that loaded the sheet and wrote its
// own colours would pass here and fall on §7.2. The two
// complement each other; neither is enough.
//
// It only sees what Vite BUILDS. A page absent from `vite.config.ts` does not
// go out into `dist/` and therefore escapes this check — a blind spot that
// `client/vite.config.ts:9-13` already documents for the build itself. The
// bench instruments (`probe-coalesced.html`, `client/recette/*.html`) are
// not entries and are outside the product: it is correct that they escape.
// The `design.html` gallery, on the other hand, IS an entry and IS checked — including
// it cannot flatter anything, since the measurement is about loading.

import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';

const TOKEN_TEMOIN = '--fond-0';

/**
 * 🔴 THE ONLY EXCLUSION FROM ASSERTION A. See the box above. Any
 * addition to this list must carry the reason why the page CANNOT
 * make the check fail — not the reason why it would be in the way.
 */
const EXCLUS_DE_A = new Map([
    [
        'shell.html',
        'pure redirection since 31 August 2026 (task 9); it paints ' +
            'nothing and so links NO sheet, on purpose — see its comment',
    ],
]);

const args = process.argv.slice(2);
const iDist = args.indexOf('--dist');
const dist = iDist === -1 ? 'client/dist' : args[iDist + 1];

if (!existsSync(dist)) {
    console.error(`${dist} is missing: run « npm run build » first.`);
    process.exit(2);
}

/** The `href` of the stylesheets linked by a page, resolved on disk. */
function feuillesLiees(cheminHtml) {
    const html = readFileSync(cheminHtml, 'utf8');
    const liens = [...html.matchAll(/<link\b[^>]*>/g)]
        .filter((m) => /rel\s*=\s*["']stylesheet["']/.test(m[0]))
        .map((m) => m[0].match(/href\s*=\s*["']([^"']+)["']/)?.[1])
        .filter(Boolean);
    return liens.map((href) => ({
        href,
        chemin: href.startsWith('/')
            ? join(dist, href.slice(1))
            : resolve(dirname(cheminHtml), href),
    }));
}

const pages = readdirSync(dist).filter((f) => f.endsWith('.html')).sort();
const echecsA = [];
const echecsB = [];

for (const page of pages) {
    if (EXCLUS_DE_A.has(page)) continue; // see the box: pure redirect, no link on purpose.

    const chemin = join(dist, page);
    const feuilles = feuillesLiees(chemin);

    if (feuilles.length === 0) {
        echecsA.push(page);
        continue; // see the box: B is not judged on a page that has no link.
    }

    const porteuses = feuilles.filter(
        (f) => existsSync(f.chemin) && readFileSync(f.chemin, 'utf8').includes(TOKEN_TEMOIN),
    );
    if (porteuses.length === 0) {
        echecsB.push({ page, feuilles: feuilles.map((f) => f.href) });
    }
}

console.log(`built pages: ${pages.length} (${pages.join(', ')})`);
for (const [page, raison] of EXCLUS_DE_A) {
    console.log(`  excluded from A: ${page} — ${raison}`);
}
console.log('');
console.log(`assertion A — one <link rel="stylesheet"> per page: ${echecsA.length} failure(s)`);
for (const page of echecsA) console.log(`  FAILURE A  ${page}: no linked stylesheet`);
console.log(
    `assertion B — a linked sheet declaring ${TOKEN_TEMOIN}: ${echecsB.length} failure(s)` +
        // Neither the failures of A, nor the pages EXCLUDED FROM A (`continue` before B) are
        // evaluated by B — the three counted built pages, the failures, the
        // excluded and the evaluated ones, must add up to this total.
        ` (evaluated on ${pages.length - echecsA.length - EXCLUS_DE_A.size} page(s))`,
);
for (const e of echecsB) {
    console.log(`  FAILURE B  ${e.page}: links ${e.feuilles.join(', ')}, none declares ${TOKEN_TEMOIN}`);
}

const total = echecsA.length + echecsB.length;
console.log('');
console.log(total === 0 ? 'BOTH assertions are green' : `total: ${total} failure(s)`);
process.exit(total > 0 ? 1 : 0);
