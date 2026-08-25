#!/usr/bin/env node
// Contrôle §7.6 de la spec ⑥ — AUCUN TOKEN ORPHELIN, AUCUN `var()` NON DÉCLARÉ.
//
// TROIS assertions — deux inclusions dans les deux sens entre les tokens
// DÉCLARÉS par `tokens/couleurs.css` et `tokens/echelles.css` (`tokens.css`
// avant l'extraction de la tâche 6, 25 août 2026 — ce contrôle a besoin des
// DEUX, la liste d'orphelins portant aussi bien des couleurs que des
// échelles) et les tokens EMPLOYÉS par les feuilles de production, plus une
// sur la liste d'attente elle-même :
//
//   ① employé ⊆ déclaré — un `var(--fond-O)` (lettre O au lieu du zéro) est une
//      faute de frappe que le navigateur avale en silence : la propriété prend
//      sa valeur de repli, ou rien, et la page reste debout mais fausse ;
//   ② déclaré ⊆ employé — un token que personne n'appelle est du code mort, et
//      un code mort dans une source unique de valeurs se recopie longtemps ;
//   ③ aucune entrée de la liste d'attente ne nomme un sous-bloc DÉJÀ CLOS —
//      la mitigation partielle du re-étiquetage, bâtie au sous-bloc S3, et que
//      le plan de S2 déclarait impossible TROIS fois. Sa doctrine et son
//      objection vivent auprès de la liste.
//
// Il ne porte AUCUNE règle de parsing : `tokensDeclares` et `tokensReferences`
// vivent dans `client/src/design/tokens.ts`, qui est typechecké et testé.
//
// ═══════════════════════════════════════════════════════════════════════════
// 🔴 `client/design.html` EST EXCLU DE LA MOITIÉ « EMPLOYÉ », ET C'EST CE QUI
// REND CE CONTRÔLE CAPABLE D'ÉCHOUER.
//
// La galerie rend TOUS les tokens par construction — c'est sa raison d'être.
// L'inclure dans le périmètre « employé » rendrait l'inclusion ② vraie pour
// toujours : le contrôle validerait la galerie et rien d'autre. La spec §7.6
// le dit, et ce dépôt a attrapé quatre contrôles incapables d'échouer sur le
// seul sous-bloc D10, dont trois écrits par un plan.
//
// ⚠️ L'EXCLUSION EST PORTEUSE, PAS DÉCORATIVE, et cela se mesure : le périmètre
// ci-dessous balaie les SURFACES HTML autant que les feuilles `.css`, parce
// qu'une page peut référencer un token depuis un `<style>` en ligne — ce que
// la galerie fait précisément. Retirer `design.html` de `EXCLUS` fait passer
// la rouge A au vert (rouge C de la tâche 12).
// ═══════════════════════════════════════════════════════════════════════════
//
// ⚠️ TAILLE DE CE FICHIER — LA PRÉDICTION DU PLAN S2 EST FAUSSE, ET C'EST DIT
// PLUTÔT QUE RABOTÉ. Le plan attendait de la tâche 8 un fichier « plus court
// qu'à `56b975a` (233) », les 18 entrées retirées de la liste d'attente devant
// le faire maigrir. Relevé par la commande à la tâche 8 de S2 :
//
//   entrées de liste  28 → 10   (−18)
//   commentaires     104 → 153  (+49)
//   code              87 →  93  (+6 : la seconde exclusion et `--sans-exclusion`)
//   lignes vides      14 →  14
//   TOTAL            233 → 270  (+37)
//
// **Les 18 entrées retirées ont été plus qu'annulées par du commentaire.**
// C'est, en petit, la leçon que ce dépôt a payée en grand : « une addition de
// commentaire peut annuler une extraction ». Les +49 ne sont pas du remplissage
// — ce sont la raison MESURÉE de la seconde exclusion (tâche 7), l'encadré des
// re-tags que rien ne contrôle, et le relevé de S1 refait au lieu d'être
// effacé, trois blocs que le plan EXIGE. **Les raboter échangerait une vérité
// contre un nombre**, ce que `CLAUDE.md` interdit nommément.
//
// 🔴 CE FICHIER NE FRANCHIT AUCUNE PORTE : 270 contre 300, marge 30. Mais la
// marge n'est plus confortable, et la règle du dépôt s'applique — TOUTE
// ADDITION SUBSTANTIELLE À CE FICHIER APPELLE UNE EXTRACTION, JAMAIS UNE
// COMPRESSION. Le point de chute est nommé d'avance :
// `client/outils/tokens-orphelins/attente.mjs`, qui emporterait
// `EN_ATTENTE_D_APPELANT` **avec sa doctrine**, comme `serveur/instances.rs` a
// emporté `TAMPON` avec le commentaire qui le justifie.
// ═══════════════════════════════════════════════════════════════════════════

import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { tokensDeclares, tokensReferences } from '../src/design/tokens.ts';
import configVite from '../vite.config.ts';
import { EN_ATTENTE_D_APPELANT } from './tokens-orphelins/attente.mjs';
import { SOUS_BLOCS_CLOS } from './tokens-orphelins/sous-blocs-clos.mjs';

/**
 * 🔴 DEUX SOURCES DEPUIS L'EXTRACTION DE LA TÂCHE 6 (25 août 2026), PAS UNE :
 * elles DÉCLARENT, elles n'emploient pas. Hors du périmètre — comme
 * `tokens.css`, leur porte d'entrée commune, qui n'en déclare plus aucun
 * lui-même (il ne fait plus qu'IMPORTER les deux) et rejoint donc l'exclusion
 * pour la même raison, ci-dessous.
 */
const SOURCES = [
    'client/src/design/tokens/couleurs.css',
    'client/src/design/tokens/echelles.css',
];
const PORTE = 'client/src/design/tokens.css';

/**
 * 🔴 LA SEULE EXCLUSION DU PÉRIMÈTRE « EMPLOYÉ ». Voir l'encadré ci-dessus.
 * Toute addition à cette liste doit porter la raison pour laquelle le fichier
 * ne peut pas faire échouer le contrôle — pas la raison pour laquelle il est
 * gênant.
 */
const EXCLUS = new Map([
    [
        'client/design.html',
        'la galerie rend tous les tokens par construction ; l’inclure rendrait ' +
            'l’inclusion « déclaré ⊆ employé » vraie pour toujours',
    ],
    [
        // 🔴 LA RAISON POUR LAQUELLE CE FICHIER NE PEUT PAS FAIRE ÉCHOUER LE
        // CONTRÔLE, et non celle pour laquelle il gênerait — c'est la clause de
        // l'encadré ci-dessus. Mesurée, non supposée : avant cette entrée, la
        // page faisait tomber CINQ lignes « À RETIRER DE LA LISTE » (--e-5,
        // --e-6, --e-7, --lh-large, --t-3xl), toutes employées par sa SEULE
        // mise en page de démonstration. La liste d'attente aurait rétréci de
        // cinq sans que le produit ait gagné un seul appelant.
        'client/primitives.html',
        'une page de démonstration emploie des tokens par construction, dans sa ' +
            'propre mise en page ; l’inclure ferait sortir de la liste d’attente ' +
            'des tokens que le PRODUIT n’appelle pas',
    ],
]);

// ═══════════════════════════════════════════════════════════════════════════
// 🔴 LA LISTE D'ATTENTE VIT DANS `tokens-orphelins/attente.mjs`, AVEC TOUTE SA
// DOCTRINE — extraite par la tâche 8 de S2, la donnée et sa justification
// ensemble. Ce qu'il faut savoir ici tient en un mot : le contrôle exige
// l'ÉGALITÉ entre l'ensemble des orphelins et cette liste, donc il échoue dans
// LES DEUX SENS — un orphelin absent de la liste, comme une entrée de la liste
// qui a gagné un appelant. C'est la seconde moitié qui la rend AUTO-NETTOYANTE.
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
        console.error(`${SOURCES[i]} est absent : rien n'a été mesuré, ce n'est pas un succès.`);
        process.exit(2);
    }
}
const porte = join(racine, PORTE);

// Le périmètre « employé » : toute feuille de `client/src/` sauf les sources
// (et leur porte d'entrée commune, `tokens.css`), et
// toute surface HTML sauf celles d'`EXCLUS`. Les surfaces sont incluses parce
// qu'un `<style>` en ligne emploie des tokens comme une feuille.
//
// 🔴 LES SURFACES VIENNENT DES ENTRÉES VITE, JAMAIS D'UN `client/*.html`.
// Un balayage du répertoire attrape `probe-coalesced.html`,
// `recette/latency-test.html` et `recette/scroll-test.html`, qui sont des
// INSTRUMENTS DE BANC : ils ne sortent pas du build, ils ne reçoivent pas
// l'amorce, et un `var(--…)` écrit dans l'un d'eux ne doit pas compter comme
// un appelant de production. `client/vite.config.ts:9-13` porte déjà cet
// avertissement, mesuré sur `connexion.html`.
// ⚠️ La liste est LUE depuis `vite.config.ts`, pas recopiée : c'est la même
// raison qui interdit à ce script d'avoir sa propre copie des valeurs. Node
// v24.9.0 importe le `.ts` nativement, comme pour `tokens.ts`.
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

const texteSource = sources.map((chemin) => readFileSync(chemin, 'utf8')).join('\n');
const declares = tokensDeclares(texteSource);
const orphelins = [...declares].filter((t) => !employePar.has(t)).sort();
const nonDeclares = [...employePar.keys()].filter((t) => !declares.has(t)).sort();

console.log(`sources       : ${SOURCES.join(', ')} — ${declares.size} token(s) déclaré(s)`);
console.log(
    `périmètre     : ${balayes.length} fichier(s) — ${employePar.size} token(s) employé(s)`,
);
console.log(`  balayés : ${balayes.join(', ')}`);
for (const [chemin, raison] of EXCLUS) {
    console.log(`  ${sansExclusion ? 'INCLUS (--sans-exclusion)' : 'exclu'} : ${chemin} — ${raison}`);
}

// ① employé ⊆ déclaré
console.log(`\ninclusion ① — tout var(--…) est déclaré : ${nonDeclares.length} écart(s)`);
for (const token of nonDeclares) {
    console.log(`  NON DÉCLARÉ  ${token}  employé par ${employePar.get(token).join(', ')}`);
}

// ② déclaré ⊆ employé, à la liste d'attente près — et l'ÉGALITÉ, pas l'inclusion.
const nouveaux = orphelins.filter((t) => !EN_ATTENTE_D_APPELANT.has(t));
const aRetirer = [...EN_ATTENTE_D_APPELANT.keys()].filter((t) => employePar.has(t)).sort();
console.log(
    `inclusion ② — tout token déclaré a un appelant : ${orphelins.length} orphelin(s), ` +
        `dont ${orphelins.length - nouveaux.length} en attente déclarée`,
);
for (const token of nouveaux) {
    console.log(`  NOUVEL ORPHELIN  ${token}  déclaré et appelé par personne`);
}
for (const token of aRetirer) {
    console.log(
        `  À RETIRER DE LA LISTE  ${token}  a désormais un appelant ` +
            `(${employePar.get(token).join(', ')}) : la liste d'attente doit rétrécir`,
    );
}

// ③ AUCUNE ENTRÉE NE NOMME UN SOUS-BLOC DÉJÀ CLOS — la mitigation partielle du
//   re-étiquetage, bâtie au sous-bloc S3 (tâche 7).
//
// 🔴 CE QU'ELLE ATTRAPE, ET QU'AUCUNE AUTRE ASSERTION NE VOIT : les deux
// inclusions ci-dessus comparent des ENSEMBLES DE NOMS DE TOKENS. Déplacer une
// entrée de « S2 » à « S3 » ne les fait bouger ni l'une ni l'autre — c'est le
// point le plus faible du dispositif, et la porte par laquelle une liste
// d'attente s'assouplit indéfiniment sans qu'aucune commande ne le dise.
//
// ⚠️ ELLE EST PARTIELLE, et le mot est pesé : elle juge le SOUS-BLOC NOMMÉ,
// jamais le CONTENU de la raison, et elle dépend d'une liste de sous-blocs clos
// TENUE À LA MAIN — un sous-bloc qui ne s'y déclare pas la neutralise. La
// doctrine complète, et l'objection qu'elle ne garde qu'une entrée après S3,
// vivent auprès de la liste, dans `tokens-orphelins/attente.mjs`.
const surSousBlocClos = [...EN_ATTENTE_D_APPELANT.entries()]
    .filter(([, entree]) => SOUS_BLOCS_CLOS.has(entree.sousBloc))
    .sort(([a], [b]) => a.localeCompare(b));
console.log(
    `\ninclusion ③ — aucune entrée ne nomme un sous-bloc clos ` +
        `(${[...SOUS_BLOCS_CLOS].join(', ')}) : ${surSousBlocClos.length} écart(s)`,
);
for (const [token, entree] of surSousBlocClos) {
    console.log(
        `  SOUS-BLOC CLOS  ${token}  nommait ${entree.sousBloc}, qui est clos : ` +
            `décider ou re-étiqueter avec sa raison`,
    );
}

const echecs = nonDeclares.length + nouveaux.length + aRetirer.length + surSousBlocClos.length;
console.log(`\ntotal : ${echecs} écart(s)`);
if (echecs === 0) {
    console.log(
        `les trois assertions tiennent ; ${EN_ATTENTE_D_APPELANT.size} token(s) restent ` +
            `en attente d'appelant, nommés dans tokens-orphelins/attente.mjs`,
    );
}
process.exit(echecs > 0 ? 1 : 0);
