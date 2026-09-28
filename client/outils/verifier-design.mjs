#!/usr/bin/env node
// THE AGGREGATOR OF THE VISUAL BASE LAYER CHECKS — sub-project ⑥, spec §7.
//
// It runs the SEVEN checks that are scripts, prints the verdict of
// each, and exits non-zero if one of them fails.
//
// ⚠️ THEY WERE SIX UNTIL SUB-BLOCK S3, which added §7.9 — the check
// the spec does not plan for, and which is declared as a PLAN ADDITION.
// It exists because none of the seven others can see that a surface of the
// product really USES a primitive: §7.6 counts `var(--…)` in
// files, §7.3 only checks loading, §7.2 only sees
// colours. Its long reason lives in `classes-employees.mjs`.
//
// ⚠️ IT DOES NOT STOP AT THE FIRST FAILURE, and it is deliberate: an operator must
// see all the defects at once rather than one per rerun. It is the opposite of the
// choice of `scripts/verify-all.sh`, which stops dead — the difference lies in
// the fact that here the seven checks are independent and fast, there the steps are
// long and a broken step often makes the following ones unreadable.
//
// 🔴 THE EIGHTH CHECK, §7.5 (the theme switch), IS NOT HERE: it is a
// unit test, it runs in `npm test`. Saying so keeps a reader from counting
// seven and concluding one is missing.
//
// ⚠️ IT BUILDS FIRST. §7.3 and §7.7 read `dist/`, and a stale `dist/` would return
// a verdict on the previous build — one would measure the previous state believing
// to read one's own. It is the home-grown trap "a driver that leaves a supervisor
// alive makes one re-read the log of the previous attempt", transposed.

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const outils = dirname(fileURLToPath(import.meta.url));
const paquet = join(outils, '..');
const racine = join(paquet, '..');

/** The seven, in the order they are read: first the source, then the build. */
const CONTROLES = [
    ['§7.4  the three theme blocks do not diverge', 'blocs-de-theme.mjs'],
    ['§7.1  the contrasts hold the WCAG thresholds', 'contraste.mjs'],
    ['§7.2  no literal colour outside tokens/couleurs.css', 'couleurs-litterales.mjs'],
    ['§7.6  no orphan token, no undeclared var()', 'tokens-orphelins.mjs'],
    ['§7.9  every used class is declared, and a primitive reaches the product', 'classes-employees.mjs'],
    ['§7.3  every built surface carries the tokens', 'surfaces-baties.mjs'],
    ['§7.7  the CSS weight does not drift', 'poids-css.mjs'],
];

console.log('==> npm run build (otherwise §7.3 and §7.7 would judge the previous build)');
const build = spawnSync('npm', ['run', 'build'], { cwd: paquet, stdio: 'inherit' });
if (build.status !== 0) {
    console.error('\nFAILURE: the build failed — no check was run.');
    process.exit(2);
}

const echoues = [];
for (const [titre, script] of CONTROLES) {
    console.log(`\n==> ${titre}`);
    // The checks are expressed as paths from the root of the repository.
    const r = spawnSync('node', [join(outils, script)], { cwd: racine, stdio: 'inherit' });
    if (r.status !== 0) echoues.push(`${titre} (sortie ${r.status})`);
}

console.log(`\n═══ ${CONTROLES.length - echoues.length}/${CONTROLES.length} check(s) green ═══`);
for (const echec of echoues) console.log(`  FAILURE  ${echec}`);
if (echoues.length === 0) {
    console.log('  §7.5 (the theme switch) is a unit test: it runs in `npm test`.');
}
process.exit(echoues.length > 0 ? 1 : 0);
