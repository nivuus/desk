#!/usr/bin/env node
// Contrôle §7.3 de la spec ⑥ — TOUTE SURFACE BÂTIE PORTE LES TOKENS.
//
// À lancer APRÈS `npm run build`. C'est le contrôle que le cadrage réclame
// quand il exige des tokens « partagés entre hub et fenêtre de session » : une
// convention ne garantit rien, une commande si.
//
// ────────────────────────────────────────────────────────────────────────────
// DEUX ASSERTIONS, ÉVALUÉES ET COMPTÉES SÉPARÉMENT — ET C'EST LE POINT.
//
//   A — chaque `dist/*.html` porte au moins un `<link rel="stylesheet">` ;
//   B — pour CHAQUE page, au moins une des feuilles qu'ELLE lie déclare
//       `--fond-0`.
//
// B est strictement plus fort que la lettre de la spec, qui écrit « l'ENSEMBLE
// des CSS émis doit contenir la déclaration `--fond-0` ». Pris ainsi, une page
// pourrait lier une feuille sans tokens et passer, du moment qu'une AUTRE page
// en lie une qui en a. Résoudre les `href` page par page ne coûte que cette
// résolution, et ferme le trou.
//
// ⚠️ B N'EST ÉVALUÉE QUE SUR LES PAGES QUI PASSENT A, délibérément. Une page
// sans aucun lien échoue A ; la compter aussi en B ferait remonter la même
// panne deux fois et, surtout, MASQUERAIT B derrière A — le jour où A devient
// verte, personne ne saurait si B avait jamais été éprouvée. C'est la rouge
// que le sous-bloc P2 a dû rejouer après coup pour cette raison exacte. Sur
// l'arbre intact du 19 août 2026, B est rouge sur `dist/index.html`, une page
// qui passe A : les deux assertions sont donc réellement indépendantes, et on
// le VOIT dans le même rapport.
//
// ────────────────────────────────────────────────────────────────────────────
// PORTÉE HONNÊTE. Ce contrôle vérifie qu'une surface CHARGE les tokens, pas
// qu'elle les EMPLOIE. Une page qui chargerait la feuille et écrirait ses
// propres couleurs passerait ici et tomberait sur §7.2. Les deux se
// complètent ; aucun ne suffit.
//
// Il ne voit que ce que Vite BÂTIT. Une page absente de `vite.config.ts` ne
// sort pas dans `dist/` et échappe donc à ce contrôle — angle mort que
// `client/vite.config.ts:9-13` documente déjà pour le build lui-même. Les
// instruments de banc (`probe-coalesced.html`, `client/recette/*.html`) ne
// sont pas des entrées et sont hors produit : c'est correct qu'ils échappent.
// La galerie `design.html`, elle, EST une entrée et EST contrôlée — l'y
// inclure ne peut rien flatter, puisque la mesure porte sur le chargement.

import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';

const TOKEN_TEMOIN = '--fond-0';

const args = process.argv.slice(2);
const iDist = args.indexOf('--dist');
const dist = iDist === -1 ? 'client/dist' : args[iDist + 1];

if (!existsSync(dist)) {
    console.error(`${dist} est absent : lancer d'abord « npm run build ».`);
    process.exit(2);
}

/** Les `href` des feuilles de style liées par une page, résolus sur le disque. */
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
    const chemin = join(dist, page);
    const feuilles = feuillesLiees(chemin);

    if (feuilles.length === 0) {
        echecsA.push(page);
        continue; // voir l'encadré : B ne se juge pas sur une page qui n'a pas de lien.
    }

    const porteuses = feuilles.filter(
        (f) => existsSync(f.chemin) && readFileSync(f.chemin, 'utf8').includes(TOKEN_TEMOIN),
    );
    if (porteuses.length === 0) {
        echecsB.push({ page, feuilles: feuilles.map((f) => f.href) });
    }
}

console.log(`pages bâties : ${pages.length} (${pages.join(', ')})`);
console.log('');
console.log(`assertion A — un <link rel="stylesheet"> par page : ${echecsA.length} échec(s)`);
for (const page of echecsA) console.log(`  ÉCHEC A  ${page} : aucune feuille de style liée`);
console.log(
    `assertion B — une feuille liée déclarant ${TOKEN_TEMOIN} : ${echecsB.length} échec(s)` +
        ` (évaluée sur ${pages.length - echecsA.length} page(s))`,
);
for (const e of echecsB) {
    console.log(`  ÉCHEC B  ${e.page} : lie ${e.feuilles.join(', ')}, aucune ne déclare ${TOKEN_TEMOIN}`);
}

const total = echecsA.length + echecsB.length;
console.log('');
console.log(total === 0 ? 'les DEUX assertions sont vertes' : `total : ${total} échec(s)`);
process.exit(total > 0 ? 1 : 0);
