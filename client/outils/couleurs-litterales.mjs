#!/usr/bin/env node
// Contrôle §7.2 de la spec ⑥ — AUCUNE COULEUR LITTÉRALE HORS DE `tokens.css`.
//
// Sort 1 dès qu'une couleur est écrite en dur ailleurs que dans la source
// unique. Ce script ne porte AUCUNE règle de design : il balaie, il compte, il
// nomme. Toute règle appartiendrait à `client/src/design/*.ts`, qui est
// typechecké et testé (même clause que `client/src/connexion.ts:5-9`).
//
// ────────────────────────────────────────────────────────────────────────────
// POURQUOI CE CONTRÔLE PORTE TROIS GARDES, ET CE QUE CHACUN TIENT RÉELLEMENT
// (divergence D8 du plan S1, puis mutations jouées le 19 août 2026).
//
// Le §7.2 de la spec prescrit de chercher les mots-clés `black`/`white`/`red`.
// Sur l'arbre intact, la recherche brute lève deux faux positifs, tous deux du
// FRANÇAIS :
//
//     $ grep -rnoE 'black|white|\bred\b' client/src --include="*.css" --include="*.ts"
//     client/src/resize.ts:7:red
//     client/src/resize.test.ts:30:red
//
// `resize.ts:7` dit « re**d**éclenche que si l'élément change encore de
// taille » : le `\b` qui suit `red` est satisfait parce que le caractère
// suivant est `é`, que les classes de mots ASCII ne comptent pas comme lettre.
// Un contrôle qui naît rouge sur deux commentaires innocents est un contrôle
// qu'on assouplira — et la spec §11 nomme ce mode de défaillance.
//
// ⚠️ TROIS gardes le tiennent, et ils SE RECOUVRENT sur ce cas précis :
//
//   ① blanchiment des commentaires — les `/* … */` partout, et en TypeScript
//     les `//` qui ne suivent pas un `:` (sans cette réserve, `http://`
//     effacerait du code réel, donc pourrait CACHER une couleur au lieu d'en
//     révéler une). Les sauts de ligne sont conservés : les numéros restent
//     justes.
//   ② les mots-clés ne sont JAMAIS cherchés dans un `.ts`.
//   ③ les mots-clés ne sont cherchés que du côté VALEUR d'une déclaration CSS
//     (`propriété: …;`).
//
// Mutations jouées, avec leur relevé — c'est la seule façon de savoir lequel
// porte quoi, et la première a DÉMENTI la rédaction initiale de cet en-tête,
// qui créditait ① du traitement de D8 :
//
//   ① retiré seul              → 11 occurrences, `resize.ts` TOUJOURS absent
//   ② retiré seul              → 11 occurrences, `resize.ts` TOUJOURS absent
//   ① et ② retirés ensemble    → 11 occurrences, `resize.ts` TOUJOURS absent
//   ①, ② et ③ retirés          → 13, avec `resize.ts:7` ET `resize.test.ts:30`
//                                — les deux occurrences exactes de D8
//   ① retiré, cas CSS dédié    → un `/* background: #ff0000; */` commenté
//                                remonte
//
// Autrement dit : ① existe pour les couleurs COMMENTÉES en CSS — pas pour D8,
// que ② et ③ couvrent déjà. Aucun des trois n'est mort, et aucun ne se retire
// « puisque les autres couvrent » : c'est vrai un par un, faux tous ensemble.
//
// Les notations `#rrggbb`, `rgb(`, `hsl(` sont, elles, cherchées PARTOUT :
// aucune ne collisionne avec du français.
//
// ────────────────────────────────────────────────────────────────────────────
// PORTÉE DÉCLARÉE, ET CE QUE CE CONTRÔLE NE VOIT PAS.
//
// • Dans un `.ts`, un `red` nu est indiscernable d'une prose française ou d'un
//   identifiant : les mots-clés n'y sont pas cherchés. Un
//   `el.style.background = 'red'` passerait donc. C'est une limite mesurée
//   (D8), pas un oubli — et les notations hexadécimales, elles, y sont vues.
// • Les HTML balayés sont les ENTRÉES VITE, lues dans `vite.config.ts`, et non
//   tous les `client/*.html`. `client/probe-coalesced.html` porte quatre
//   couleurs littérales (`#222`, `#eee`, `#f4f4f4`) et n'est PAS du produit :
//   ce n'est pas une entrée Vite, il ne sort pas dans `dist/`, et le plan S1
//   (D11) le range avec `client/recette/*.html` parmi les instruments de banc.
//   Le balayer ferait naître ce contrôle rouge sur du code hors périmètre —
//   exactement la pression à l'assouplissement que la mesure D8 écarte.
//   ⚠️ Corollaire : une page neuve non déclarée dans `vite.config.ts` n'est
//   balayée par rien. C'est le même angle mort que `vite.config.ts:9-13`
//   documente déjà pour le build lui-même.
// • ⚠️ CE CONTRÔLE COMPTE ONZE, LÀ OÙ LE PLAN S1 EN ANNONÇAIT NEUF, et les
//   deux de l'écart sont `style.css:3-4` — `--surface: #0b0d10` et
//   `--text: #e6e8eb`. Ce sont bien des DÉCLARATIONS de token, mais elles
//   vivent dans `style.css`, pas dans `tokens.css` : l'exclusion du §7.2 est
//   par FICHIER, jamais par rôle, et une déclaration hors de la source unique
//   est précisément la dérive que ce contrôle existe pour voir. ✅ Le vert EST
//   ARRIVÉ à la tâche 9 (`ab9e9b9`), qui a fait migrer les deux déclarations :
//   ce contrôle rend ZÉRO sur 46 fichiers depuis. Le plan attribuait par ailleurs
//   un compte de onze à un défaut de traitement des commentaires : les
//   mutations ci-dessus le réfutent — un tel défaut rendrait TREIZE.
// • 🔴 QUATRE EXIGENCES QUI NE PEUVENT PAS TENIR ENSEMBLE, et la quatrième
//   cède. Le plan S1 demande à la fois : (T1) que les `*.test.ts` soient
//   BALAYÉS, « une couleur codée en dur dans un test étant une valeur en
//   double comme une autre » ; (T5) que `contraste.test.ts` porte les vecteurs
//   `#000000`, `#ffffff` et `#808080`, dont la valeur est fixée par WCAG 2.1
//   et non par nos tokens — c'est ce qui l'empêche de valider le produit
//   contre lui-même ; (T9) que `reprise.test.ts` compare `--fond-0` à
//   `#0b0d10` EXACTEMENT, ce qui est la comparaison qui PROUVE la reprise ;
//   et (T9) que ce contrôle rende ZÉRO à la fin du sous-bloc. Les vecteurs de
//   T5 et la comparaison de T9 sont des littéraux OBLIGATOIRES : les interdire
//   rendrait ces deux tâches impossibles.
//   ⚠️ L'exclusion est donc posée AU PLUS ÉTROIT : `client/src/design/*.test.ts`
//   seulement. Tout autre test du paquet reste balayé — une couleur dans
//   `resize.test.ts` ou `webrtc.test.ts` est toujours refusée, et aucun n'en
//   porte aujourd'hui. Relevé de l'écart, sur l'arbre du 19 août 2026 : sans
//   cette exclusion le contrôle rend 46, dont 35 viennent des deux seuls
//   fichiers de test du socle et 11 de `style.css`, la vraie cible.
//   ⚠️ Ce que cela coûte : un test du socle qui recopierait une couleur du
//   produit sans raison ne serait plus attrapé. C'est une règle de revue, au
//   même titre que « employer `--bord-fort` là où il porte une information ».
// • Ce contrôle dit qu'aucune couleur n'est écrite en dur. Il ne dit RIEN de
//   la justesse du token employé à la place — c'est une règle de revue
//   (spec §8).

import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, relative } from 'node:path';

const NOTATIONS = /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/g;
const MOTS_CLES = /\b(black|white|red)\b/g;
const DECLARATION = /(?:^|[;{])\s*[-a-zA-Z][-a-zA-Z0-9]*\s*:\s*([^;{}]*)/g;

/** Blanchit un intervalle en gardant les sauts de ligne, donc les numéros. */
function blanchir(texte, debut, fin) {
    const morceau = texte.slice(debut, fin).replace(/[^\n]/g, ' ');
    return texte.slice(0, debut) + morceau + texte.slice(fin);
}

function sansCommentaires(texte, extension) {
    let sortie = texte;
    const blocs = [];
    // `/* … */`, commun au CSS, au TypeScript et aux `<style>` du HTML.
    for (const m of texte.matchAll(/\/\*[\s\S]*?\*\//g)) {
        blocs.push([m.index, m.index + m[0].length]);
    }
    if (extension === '.html') {
        for (const m of texte.matchAll(/<!--[\s\S]*?-->/g)) {
            blocs.push([m.index, m.index + m[0].length]);
        }
    }
    if (extension === '.ts') {
        // ⚠️ Le `(?<![:/])` écarte `http://` et `ws://` : blanchir jusqu'au
        // bout de ligne à partir d'un `//` de schéma d'URL effacerait du code
        // réel, donc pourrait CACHER une couleur au lieu d'en révéler une.
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
    // Les mots-clés : côté valeur d'une déclaration, et jamais dans un `.ts`.
    if (extension !== '.ts') {
        for (const d of propre.matchAll(DECLARATION)) {
            const valeur = d[1];
            const debutValeur = d.index + d[0].length - valeur.length;
            for (const m of valeur.matchAll(MOTS_CLES)) {
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

/** Les HTML de PRODUIT : les entrées déclarées à Vite, et rien d'autre. */
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

// ⚠️ DEUX EXCLUSIONS, ET LA SECONDE EST UNE DIVERGENCE ASSUMÉE AVEC LE PLAN.
// Voir l'encadré « QUATRE EXIGENCES QUI NE PEUVENT PAS TENIR ENSEMBLE ».
const exclus = new Set([join(racine, 'src/design/tokens.css')]);
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

console.log(`fichiers balayés : ${aBalayer.length}`);
console.log(`couleurs littérales : ${total}`);
if (total > 0) {
    console.log('→ elles doivent vivre dans client/src/design/tokens.css');
}
process.exit(total > 0 ? 1 : 0);
