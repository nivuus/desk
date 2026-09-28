#!/usr/bin/env node
// L'AGRÉGATEUR DES CONTRÔLES DU SOCLE VISUEL — sous-projet ⑥, spec §7.
//
// Il lance les SEPT contrôles qui sont des scripts, imprime le verdict de
// chacun, et sort non nul si l'un d'eux échoue.
//
// ⚠️ ILS ÉTAIENT SIX JUSQU'AU SOUS-BLOC S3, qui a ajouté §7.9 — le contrôle
// que la spec ne prévoit pas, et qui est déclaré comme une ADDITION DE PLAN.
// Il existe parce qu'aucun des sept autres ne peut voir qu'une surface du
// produit EMPLOIE réellement une primitive : §7.6 compte des `var(--…)` dans
// des fichiers, §7.3 ne vérifie qu'un chargement, §7.2 ne voit que des
// couleurs. Sa raison longue vit dans `classes-employees.mjs`.
//
// ⚠️ IL NE S'ARRÊTE PAS AU PREMIER ÉCHEC, et c'est délibéré : un opérateur doit
// voir tous les défauts d'un coup plutôt qu'un par relance. C'est l'inverse du
// choix de `scripts/verify-all.sh`, qui s'arrête net — la différence tient à ce
// qu'ici les sept contrôles sont indépendants et rapides, là-bas les étapes sont
// longues et une étape cassée rend souvent les suivantes illisibles.
//
// 🔴 LE HUITIÈME CONTRÔLE, §7.5 (la bascule de thème), N'EST PAS ICI : c'est un
// test unitaire, il tourne dans `npm test`. Le dire évite qu'un lecteur compte
// sept et conclue qu'il en manque un.
//
// ⚠️ IL BÂTIT D'ABORD. §7.3 et §7.7 lisent `dist/`, et un `dist/` périmé rendrait
// un verdict sur le build d'avant — on mesurerait l'état précédent en croyant
// lire le sien. C'est le piège maison « un pilote qui laisse un superviseur
// vivant fait relire le journal de la tentative précédente », transposé.

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const outils = dirname(fileURLToPath(import.meta.url));
const paquet = join(outils, '..');
const racine = join(paquet, '..');

/** Les sept, dans l'ordre où ils se lisent : d'abord la source, puis le bâti. */
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
    // Les contrôles s'expriment en chemins depuis la racine du dépôt.
    const r = spawnSync('node', [join(outils, script)], { cwd: racine, stdio: 'inherit' });
    if (r.status !== 0) echoues.push(`${titre} (sortie ${r.status})`);
}

console.log(`\n═══ ${CONTROLES.length - echoues.length}/${CONTROLES.length} check(s) green ═══`);
for (const echec of echoues) console.log(`  FAILURE  ${echec}`);
if (echoues.length === 0) {
    console.log('  §7.5 (the theme switch) is a unit test: it runs in `npm test`.');
}
process.exit(echoues.length > 0 ? 1 : 0);
