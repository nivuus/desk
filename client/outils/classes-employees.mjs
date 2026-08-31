#!/usr/bin/env node
// Contrôle §7.9 — TOUTE CLASSE EMPLOYÉE EST DÉCLARÉE, ET UNE PRIMITIVE ATTEINT
// LE PRODUIT.
//
// 🔴 CE CONTRÔLE N'EST PAS DANS LA SPEC : c'est une ADDITION du sous-bloc S3,
// déclarée comme telle. Il existe parce qu'AUCUN des sept contrôles précédents
// ne peut voir l'écart que S2 a lui-même nommé — « les dix-huit tokens sortis
// de la liste ont un appelant ÉCRIT, pas un pixel RENDU ». Et ce n'est pas une
// lacune de rédaction, c'est structurel :
//
//   - §7.6 compte des `var(--…)` dans des FICHIERS. Poser
//     `class="bouton bouton--principal"` sur une page n'ajoute aucun `var()` :
//     le contrôle compte exactement les mêmes tokens employés avant et après.
//   - §7.3 ne vérifie que le CHARGEMENT d'une feuille, jamais son emploi — son
//     propre en-tête le dit.
//   - §7.2 ne voit que des couleurs.
//
// ── LES TROIS ASSERTIONS, COMPTÉES SÉPARÉMENT ─────────────────────────────
//
//   ① employé ⊆ déclaré — un `class="bouton--principale"` (faute de frappe)
//      est une règle qui ne s'applique à rien, et le navigateur ne dit RIEN :
//      la page reste debout et fausse. Aucun autre contrôle ne le voit.
//   ② A — ATTEIGNABILITÉ : CHACUNE des trois surfaces du produit emploie au
//      moins une famille de primitives. C'est cette assertion, et elle seule,
//      qui referme l'écart de S2. Elle est née ROUGE sur l'arbre intact —
//      mesuré le 20 août 2026, `grep -n 'class=' client/{index,shell,connexion}
//      .html` ne rendait AUCUNE ligne.
//
//      🔴 DURCIE PAR LA TÂCHE 3 DU SOUS-BLOC S4, ET ELLE RENAÎT ROUGE. Elle
//      exigeait « AU MOINS UNE surface, AU MOINS UNE famille » : ce quantifieur
//      était juste tant qu'une seule surface avait été reprise, et il est devenu
//      un plafond dès la deuxième — deux surfaces sur trois habillées le
//      laissaient vert, et la troisième pouvait rester nue pour toujours sans
//      qu'aucune commande ne le dise. C'est exactement ce qui s'est passé : à la
//      fin de S3, `client/index.html` rendait « aucune famille », et le contrôle
//      était VERT. La fenêtre de session est reprise par S4 ; le quantifieur
//      suit.
//
//      ⚠️ ELLE EST DURCIE AVANT LA TÂCHE QUI L'ÉTEINT, ET C'EST LE POINT. La
//      durcir dans le même commit que l'habillage la rendrait verte dès sa
//      naissance, donc jamais vue rouge sur l'arbre — un contrôle qu'on n'a
//      jamais vu rouge n'est pas un contrôle. L'arbre lui-même est sa preuve
//      d'atteignabilité, comme en S3.
//
//      ⚠️ LA SORTIE NOMME LA SURFACE QUI N'EMPLOIE RIEN, pas seulement le fait
//      qu'il en existe une : « la surface X n'emploie aucune famille » est
//      actionnable, « aucune surface n'en emploie » ne l'était pas.
//   ③ B — chacune des familles de primitives apparaît dans la galerie
//      `client/primitives.html`. Elle prend une part du legs n°4 de S2 : « une
//      galerie qui cesserait de rendre une famille entière ne serait attrapée
//      par aucun contrôle ».
//
// ⚠️ Il ne porte AUCUNE RÈGLE DE PARSING : `classesDeclarees`,
// `classesEmployeesHtml`, `classesEmployeesTs` et `classesDeclareesEnLigne`
// vivent dans `client/src/design/classes.ts`, qui est typechecké et testé.
// C'est la convention de `tokens-orphelins.mjs`, et la même raison.
//
// ⚠️ LES FAMILLES NE SONT PAS ÉNUMÉRÉES ICI : elles sont DÉRIVÉES des fichiers
// de `client/src/design/primitives/`, un fichier par famille. Une cinquième
// famille entre donc dans ② et ③ sans qu'une ligne de ce script ne change, et
// une liste recopiée ne peut pas diverger de ce qu'elle décrit.
//
// ═══════════════════════════════════════════════════════════════════════════
// 🔴 CE QUE CE CONTRÔLE NE DIT PAS.
//
//   - LE SENS INVERSE — toute classe déclarée est employée — N'EST PAS PRIS.
//     Il exigerait une SECONDE liste d'attente (`.bouton--discret` peut n'avoir
//     aucun appelant), et une liste de plus pour une famille déjà gardée par sa
//     galerie serait un coût sans contrepartie. C'est `primitives.html` et
//     l'œil qui tiennent ce sens-là (spec §7.8).
//   - UNE CLASSE CALCULÉE À L'EXÉCUTION EST INVISIBLE — `el.className = x`,
//     une concaténation, un `classList.toggle(nom)`. La règle qui rend ce
//     contrôle utile est la convention : les classes s'écrivent en LITTÉRAL,
//     dans le HTML de préférence.
//   - IL NE JUGE D'AUCUNE APPARENCE. Qu'une classe soit posée ne dit pas
//     qu'elle rende bien. Le jugement humain reste entier.
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

/** Les surfaces du PRODUIT — celles qu'un utilisateur voit.
 *
 * 🔴 `client/hub.html` Y EST ENTRÉ LE 31 AOÛT 2026, ET IL N'Y ÉTAIT PAS : le
 * hub est servi à la racine depuis le lot 14, donc c'est LA surface que
 * l'utilisateur atteint, et elle était pourtant hors de tout contrôle §7.9.
 *
 * 🔴 ET `client/shell.html` EN EST SORTI LE MÊME JOUR (tâche 9) : la page-shell
 * est devenue une simple redirection — le corps de `shell-page.ts` tient en un
 * `import` et un `location.replace`, tout le reste y étant du commentaire ;
 * `wc -l` pour la taille, jamais un nombre recopié ici. Elle ne
 * porte plus AUCUNE classe — ni sur `<html>`, ni sur `<body>`, qui ne contient
 * qu'un `<script>`. La garder ici la ferait compter comme une surface NUE
 * (aucune famille de primitives employée), ce qui échouerait l'assertion ②
 * pour une page qui ne peint plus rien : ce n'est pas un manque de primitive,
 * c'est l'absence de tout balisage.
 */
const SURFACES_PRODUIT = [
    'client/index.html',
    'client/hub.html',
    'client/connexion.html',
];
/** La galerie du jugement humain, celle que ③ interroge. */
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
    console.error("client/src est absent : rien n'a été mesuré, ce n'est pas un succès.");
    process.exit(2);
}

const surfaces = Object.values(configVite.build.rollupOptions.input).map((n) =>
    join(racine, 'client', n),
);

// ── L'ENSEMBLE DÉCLARÉ ────────────────────────────────────────────────────
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
// ⚠️ LES `<style>` EN LIGNE SONT OBLIGATOIRES DANS CET ENSEMBLE :
// `client/design.html` déclare en ligne les six classes que `galerie.ts`
// emploie. Les omettre ferait naître ce contrôle ROUGE sur du code correct,
// c'est-à-dire la pression à l'assouplissement que ce dépôt écarte.
for (const chemin of surfaces) {
    ajouterDeclarees(classesDeclareesEnLigne(readFileSync(chemin, 'utf8')), rel(chemin));
}

// ── L'ENSEMBLE EMPLOYÉ ────────────────────────────────────────────────────
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

// ── LES FAMILLES, DÉRIVÉES DES FICHIERS ───────────────────────────────────
const familles = fichiersSuffixes(join(src, 'design/primitives'), '.css')
    .sort()
    .map((chemin) => ({
        nom: basename(chemin, '.css'),
        classes: classesDeclarees(readFileSync(chemin, 'utf8')),
    }));
const classesDePrimitive = new Set(familles.flatMap((f) => [...f.classes]));

// ── LE RELEVÉ, TOUJOURS IMPRIMÉ, SUCCÈS COMPRIS ───────────────────────────
// « Un contrôle de dérive dont on ne lit jamais la valeur ne sert qu'à passer »
// (`poids-css.mjs`).
console.log(`déclarées : ${declarePar.size} classe(s)`);
console.log(`employées : ${employePar.size} classe(s)`);
console.log(`familles de primitives : ${familles.map((f) => f.nom).join(', ')}`);

// ① employé ⊆ déclaré
const nonDeclarees = [...employePar.keys()].filter((n) => !declarePar.has(n)).sort();
console.log(`\n① toute classe employée est déclarée : ${nonDeclarees.length} écart(s)`);
for (const nom of nonDeclarees) {
    console.log(`  NON DÉCLARÉE  ${nom}  employée par ${employePar.get(nom).join(', ')}`);
}

// ② A — une primitive atteint une surface du produit
console.log('\n② A — les primitives atteignent le PRODUIT, CHACUNE des trois surfaces :');
const nues = [];
for (const surface of SURFACES_PRODUIT) {
    const chemin = join(racine, surface);
    if (!existsSync(chemin)) {
        console.log(`  ${surface} : INTROUVABLE`);
        nues.push(surface);
        continue;
    }
    const employees = classesEmployeesHtml(readFileSync(chemin, 'utf8'));
    const parFamille = familles
        .filter((f) => [...f.classes].some((c) => employees.has(c)))
        .map((f) => f.nom);
    if (parFamille.length === 0) nues.push(surface);
    console.log(
        `  ${surface} : ${parFamille.length === 0 ? 'aucune famille' : parFamille.join(', ')}`,
    );
}
// 🔴 UNE SURFACE NUE EST UN ÉCHEC, ET ELLE EST NOMMÉE. Le compte est celui des
// surfaces SANS famille, plus celui des surfaces INTROUVABLES : une surface
// retirée de `SURFACES_PRODUIT` sans être retirée du disque rendrait `0 écart`
// en ne mesurant plus rien, et c'est le patron du contrôle vacueux.
const echecA = nues.length;
for (const surface of nues) {
    console.log(
        `  ${surface} n'emploie AUCUNE famille de primitives : ` +
            'les primitives y ont un appelant ÉCRIT, pas un pixel RENDU',
    );
}

// ③ B — chaque famille est rendue par la galerie
const galerie = join(racine, GALERIE_PRIMITIVES);
const employeesGalerie = existsSync(galerie)
    ? classesEmployeesHtml(readFileSync(galerie, 'utf8'))
    : new Set();
const absentes = familles.filter((f) => ![...f.classes].some((c) => employeesGalerie.has(c)));
console.log(`\n③ B — chaque famille est rendue par ${GALERIE_PRIMITIVES} :`);
for (const f of familles) {
    const rendues = [...f.classes].filter((c) => employeesGalerie.has(c)).length;
    console.log(`  ${f.nom} : ${rendues} classe(s) employée(s) sur ${f.classes.size} déclarée(s)`);
}
for (const f of absentes) {
    console.log(`  famille ${f.nom.toUpperCase()} absente de ${GALERIE_PRIMITIVES}`);
}

const echecs = nonDeclarees.length + echecA + absentes.length;
console.log(`\ntotal : ${echecs} écart(s)`);
process.exit(echecs > 0 ? 1 : 0);
