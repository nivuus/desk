#!/usr/bin/env node
// L'AGRÉGATEUR DES CONTRÔLES DU SOCLE VISUEL — sous-projet ⑥, spec §7.
//
// Il lance les SIX contrôles qui sont des scripts, imprime le verdict de
// chacun, et sort non nul si l'un d'eux échoue.
//
// ⚠️ IL NE S'ARRÊTE PAS AU PREMIER ÉCHEC, et c'est délibéré : un opérateur doit
// voir tous les défauts d'un coup plutôt qu'un par relance. C'est l'inverse du
// choix de `scripts/verify-all.sh`, qui s'arrête net — la différence tient à ce
// qu'ici les six contrôles sont indépendants et rapides, là-bas les étapes sont
// longues et une étape cassée rend souvent les suivantes illisibles.
//
// 🔴 LE SEPTIÈME CONTRÔLE, §7.5 (la bascule de thème), N'EST PAS ICI : c'est un
// test unitaire, il tourne dans `npm test`. Le dire évite qu'un lecteur compte
// six et conclue qu'il en manque un.
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

/** Les six, dans l'ordre où ils se lisent : d'abord la source, puis le bâti. */
const CONTROLES = [
    ['§7.4  les trois blocs de thème ne divergent pas', 'blocs-de-theme.mjs'],
    ['§7.1  les contrastes tiennent les seuils WCAG', 'contraste.mjs'],
    ['§7.2  aucune couleur littérale hors de tokens.css', 'couleurs-litterales.mjs'],
    ['§7.6  aucun token orphelin, aucun var() non déclaré', 'tokens-orphelins.mjs'],
    ['§7.3  toute surface bâtie porte les tokens', 'surfaces-baties.mjs'],
    ['§7.7  le poids CSS ne dérive pas', 'poids-css.mjs'],
];

console.log('==> npm run build (sans quoi §7.3 et §7.7 jugeraient le build d’avant)');
const build = spawnSync('npm', ['run', 'build'], { cwd: paquet, stdio: 'inherit' });
if (build.status !== 0) {
    console.error('\nÉCHEC : le build a échoué — aucun contrôle n’a été joué.');
    process.exit(2);
}

const echoues = [];
for (const [titre, script] of CONTROLES) {
    console.log(`\n==> ${titre}`);
    // Les contrôles s'expriment en chemins depuis la racine du dépôt.
    const r = spawnSync('node', [join(outils, script)], { cwd: racine, stdio: 'inherit' });
    if (r.status !== 0) echoues.push(`${titre} (sortie ${r.status})`);
}

console.log(`\n═══ ${CONTROLES.length - echoues.length}/${CONTROLES.length} contrôle(s) vert(s) ═══`);
for (const echec of echoues) console.log(`  ÉCHEC  ${echec}`);
if (echoues.length === 0) {
    console.log('  §7.5 (la bascule de thème) est un test unitaire : il tourne dans `npm test`.');
}
process.exit(echoues.length > 0 ? 1 : 0);
