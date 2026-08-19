#!/usr/bin/env node
// Contrôle §7.6 de la spec ⑥ — AUCUN TOKEN ORPHELIN, AUCUN `var()` NON DÉCLARÉ.
//
// Deux inclusions, dans les deux sens, entre les tokens DÉCLARÉS par
// `tokens.css` et les tokens EMPLOYÉS par les feuilles de production :
//
//   ① employé ⊆ déclaré — un `var(--fond-O)` (lettre O au lieu du zéro) est une
//      faute de frappe que le navigateur avale en silence : la propriété prend
//      sa valeur de repli, ou rien, et la page reste debout mais fausse ;
//   ② déclaré ⊆ employé — un token que personne n'appelle est du code mort, et
//      un code mort dans une source unique de valeurs se recopie longtemps.
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

import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { tokensDeclares, tokensReferences } from '../src/design/tokens.ts';
import configVite from '../vite.config.ts';

/** La source unique : elle DÉCLARE, elle n'emploie pas. Hors du périmètre. */
const SOURCE = 'client/src/design/tokens.css';

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
// LA LISTE D'ATTENTE — 10 tokens déclarés que le produit n'appelle pas ENCORE.
//
// ⚠️ CE NOMBRE EST TENU À JOUR PAR LA TÂCHE QUI LE REND FAUX, jamais par une
// tâche de ménage plus tard : elle en a 28 à la fin de S1, et chaque famille
// de primitives de S2 le fait descendre dans SON commit. Un compte qui
// n'appartient à personne dérive — ce dépôt l'a payé assez souvent.
//
// 🔴 CE N'EST PAS UN ASSOUPLISSEMENT DU CONTRÔLE, ET LA DIFFÉRENCE TIENT À UN
// MOT : ÉGALITÉ, pas inclusion. Le contrôle exige que l'ensemble des orphelins
// soit EXACTEMENT cette liste. Il échoue donc dans LES DEUX SENS :
//
//   • un token orphelin absent de la liste  → « nouvel orphelin »   (rouge)
//   • un token de la liste qui a un appelant → « à retirer d'ici »  (rouge)
//
// La seconde moitié est celle qui compte : elle rend la liste AUTO-NETTOYANTE.
// Un seuil (« au plus 28 orphelins ») aurait pourri sur place ; une liste
// nommée dont chaque retrait est FORCÉ par le contrôle rétrécit toute seule,
// et le jour où elle est vide, ces trois blocs disparaissent avec elle.
//
// ── POURQUOI CETTE PALETTE N'EST PAS SIMPLEMENT RÉDUITE À CE QUI SERT ──────
// C'était la voie évidente, et elle est REFUSÉE SUR MESURE, prise le 19 août
// 2026 : sur les 50 paires de contraste déclarées du §4.5 que le contrôle §7.1
// vérifie, **46 citent au moins un token de cette liste**. Élaguer la palette
// pour verdir §7.6 ferait tomber §7.1 de 50 paires à 4 — on satisferait un
// contrôle en vidant l'autre, ce qui est exactement le geste que ce dépôt
// combat. Relevé par la commande :
//
//   node --input-type=module -e "import {PAIRES} from './src/design/contraste.ts'; …"
//   → paires totales : 50 | paires citant au moins un token sans appelant : 46
//
// ⚠️ CE CONTRÔLE EST DONC ROUGE PAR CONSTRUCTION JUSQU'À S4 SI ON LE PREND
// COMME MESURE DE « la palette est-elle entièrement employée ? ». Ce n'est pas
// ce qu'il mesure. Ce qu'il mesure, à partir de S1, c'est que **l'écart entre
// la palette et son emploi soit CONNU, ÉNUMÉRÉ ET DÉCROISSANT** — et cela, il
// peut l'échouer dès aujourd'hui, dans les deux sens.
//
// Chaque entrée nomme le sous-bloc qui la consommera. Relevé le 19 août 2026,
// au commit de la tâche 12 du sous-bloc S1.
// ═══════════════════════════════════════════════════════════════════════════
const EN_ATTENTE_D_APPELANT = new Map([
    // ── Échelle typographique — S2 ────────────────────────────────────────
    ['--t-xs', 'S2 — la mention légale et les étiquettes'],
    ['--t-2xl', 'S3 — le titre de l’écran de connexion'],
    ['--t-3xl', 'S3 — le titre du hub'],
    // ── Interlignes ───────────────────────────────────────────────────────
    ['--lh-large', 'S3 — les paragraphes longs'],
    // ── Espacement — S2 et S3 ─────────────────────────────────────────────
    ['--e-1', 'S2 — l’écart interne d’une étiquette'],
    ['--e-5', 'S3 — la gouttière entre cartes'],
    ['--e-6', 'S3 — la marge des sections'],
    ['--e-7', 'S3 — la marge de tête des surfaces'],
    // ── Rayons — S2 ───────────────────────────────────────────────────────
    ['--r-plein', 'S2 — les pastilles et les boutons ronds'],
    // ── Le cas particulier, et il est nommé ───────────────────────────────
    // 🔴 `--police-mono` N'A QU'UN SEUL APPELANT PRÉVU, `#stats`, et la spec
    // §4.3 laisse son sort ouvert : « si aucun appelant n'apparaît, le token
    // sort ». Il N'A PAS été câblé par S1, et pas par oubli : `#stats` hérite
    // aujourd'hui de `--police-ui`, si bien que lui poser la pile monospace
    // CHANGERAIT SON APPARENCE — ce que S1 s'interdit nommément (« visuellement
    // quasi neutre sur index.html »). C'est donc S4, le sous-bloc qui a le
    // droit de toucher la fenêtre de session, qui tranche : ou il le câble, ou
    // il le retire. Aucun autre sous-bloc n'a le droit de laisser cette ligne
    // en place sans décider.
    ['--police-mono', 'S4 — #stats, OU RETRAIT : le seul token dont le sort est encore ouvert'],
]);

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

const source = join(racine, SOURCE);
if (!existsSync(source)) {
    console.error(`${SOURCE} est absent : rien n'a été mesuré, ce n'est pas un succès.`);
    process.exit(2);
}

// Le périmètre « employé » : toute feuille de `client/src/` sauf la source, et
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
const feuilles = fichiersCss(join(racine, 'client/src')).filter((f) => f !== source);
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

const declares = tokensDeclares(readFileSync(source, 'utf8'));
const orphelins = [...declares].filter((t) => !employePar.has(t)).sort();
const nonDeclares = [...employePar.keys()].filter((t) => !declares.has(t)).sort();

console.log(`source        : ${SOURCE} — ${declares.size} token(s) déclaré(s)`);
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

const echecs = nonDeclares.length + nouveaux.length + aRetirer.length;
console.log(`\ntotal : ${echecs} écart(s)`);
if (echecs === 0) {
    console.log(
        `les deux inclusions tiennent ; ${EN_ATTENTE_D_APPELANT.size} token(s) restent ` +
            `en attente d'appelant, nommés dans ce script`,
    );
}
process.exit(echecs > 0 ? 1 : 0);
