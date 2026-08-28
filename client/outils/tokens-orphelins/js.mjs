// LA MOITIÉ « POSÉ PAR LE JS » DU CONTRÔLE §7.6 — sous-bloc A1, tâche 7 du
// chantier `legs-sans-vm` (25 août 2026).
//
// 🔴 CE QUE `tokens-orphelins.mjs` NE POUVAIT PAS VOIR, ET QUI LUI RENDAIT LE
// LEGS D'`accent-dom.ts` INVISIBLE. Le contrôle §7.6 ne connaissait qu'UNE
// façon d'« employer » un token : un `var(--…)` dans du CSS (ou dans un
// `<style>` en ligne d'une surface HTML). Mais `accent-dom.ts` pose
// `--accent-fenetre` par `acces.poserToken(TOKEN_ACCENT, …)`, qui appelle
// `document.documentElement.style.setProperty(nom, valeur)` — AUCUN `var()`
// n'apparaît nulle part dans ce chemin. Un token DÉCLARÉ, posé à l'exécution,
// et référencé par AUCUN `var()`, était donc un ORPHELIN au sens de l'ancien
// contrôle, alors qu'il ne l'était pas : le commentaire d'`accent-dom.ts`
// l'avait vu (D-A1-2, sonde H1 du 21 août 2026) et avait choisi de NE PAS
// déclarer le token plutôt que de vivre avec un contrôle aveugle en
// permanence. Ce fichier ferme ce trou : il donne au contrôle une SECONDE
// façon d'« employer » un token, symétrique de la première.
//
// ═══════════════════════════════════════════════════════════════════════════
// 🔴 RÈGLE DE SÉLECTION, ÉNONCÉE AVANT LE BALAYAGE — ET LA RAISON QUI EXCLUT
// LES TESTS.
//
// Deux passes, sur le MÊME périmètre de fichiers :
//
//   ① COLLECTER LES CONSTANTES DE TOKEN : toute déclaration
//      `const <NOM> = '<--jeton>'` (ou en double quotes), n'importe où dans
//      ce périmètre — c'est le patron `export const TOKEN_ACCENT =
//      '--accent-fenetre';` d'`accent-dom.ts`. La table obtenue associe un
//      IDENTIFIANT (`TOKEN_ACCENT`) à un NOM DE TOKEN (`--accent-fenetre`).
//
//   ② TROUVER LES APPELS : tout `poserToken(` suivi soit d'un littéral
//      `'--jeton'` directement, soit d'un identifiant que ① a résolu. C'est
//      la trace d'un `setProperty` en devenir — la SEULE trace, puisque
//      `poserToken` est un nom de méthode d'interface
//      (`AccesTokens::poserToken`, `accent-dom.ts`), jamais un mot-clé du
//      langage : un futur second appelant (un autre `…-dom.ts`) serait
//      attrapé par la MÊME règle, sans modification.
//
// 🔴 LE PÉRIMÈTRE EST `client/src/**/*.ts`, **MOINS TOUT `*.test.ts`**, ET
// C'EST LE PIÈGE NOMMÉ PAR LE BRIEF DE LA TÂCHE : `accent-dom.test.ts` pose
// SA PROPRE fausse implémentation de `poserToken` pour espionner les appels
// (`poserToken: (nom, valeur) => { … }`, une DÉFINITION de propriété
// d'objet, jamais un APPEL — elle ne matche donc déjà pas ①, `poserToken(`
// exigeant une PARENTHÈSE immédiatement après le nom, là où le test écrit
// `poserToken:`). Mais un futur test pourrait très bien écrire
// `objet.poserToken('--jeton-de-test', 'x')` pour exercer un cas limite, et
// CE SERAIT UN APPEL — la même forme textuelle qu'un vrai. Un contrôle qui
// rougirait sur son PROPRE test serait « pire que pas de contrôle » (brief de
// la tâche) : personne ne le croirait la fois où il aurait raison. D'où
// l'exclusion du périmètre entier, pas une liste de motifs à reconnaître et
// écarter au cas par cas.
//
// ⚠️ CE QUE CETTE RÈGLE NE VOIT PAS, ET C'EST UNE LIMITE ASSUMÉE : un appel
// par un ALIAS (`const p = acces.poserToken; p(TOKEN_ACCENT, …)`) ou par
// DÉSTRUCTURATION (`const { poserToken: pose } = acces; pose(…)`) resterait
// invisible — la règle cherche le TEXTE `poserToken(`, jamais une analyse de
// flux. Le seul appelant de production à ce jour (`accent-dom.ts`) n'emploie
// ni l'un ni l'autre.
// ═══════════════════════════════════════════════════════════════════════════

import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';

/** Tous les `.ts` sous `repertoire`, hors `*.test.ts`. */
function fichiersTs(repertoire, acc = []) {
    if (!existsSync(repertoire)) return acc;
    for (const entree of readdirSync(repertoire, { withFileTypes: true })) {
        const chemin = join(repertoire, entree.name);
        if (entree.isDirectory()) fichiersTs(chemin, acc);
        else if (entree.name.endsWith('.ts') && !entree.name.endsWith('.test.ts')) acc.push(chemin);
    }
    return acc;
}

const RE_CONSTANTE = /\bconst\s+([A-Za-z_$][\w$]*)\s*=\s*['"](--[\w-]+)['"]/g;
const RE_APPEL = /\bposerToken\(\s*(?:['"](--[\w-]+)['"]|([A-Za-z_$][\w$]*))/g;

/**
 * Les tokens posés par `poserToken(...)` dans le périmètre de PRODUCTION
 * (`client/src/**\/*.ts`, `*.test.ts` exclus) → la liste des fichiers,
 * relatifs à `racine`, où l'appel a été trouvé.
 *
 * ⚠️ LA RÉSOLUTION D'IDENTIFIANT EST GLOBALE AU PÉRIMÈTRE, PAS PAR FICHIER :
 * une constante déclarée dans un fichier et un appel dans un autre (un futur
 * import partagé) sont réconciliés. Ce dépôt n'a aujourd'hui qu'un seul
 * appelant et une seule constante, TOUS DEUX dans `accent-dom.ts` — la
 * portée globale n'y change rien, mais la restreindre au fichier romprait le
 * jour où un second module importerait la constante d'un premier.
 */
export function tokensPosesParLeJs(racine) {
    const fichiers = fichiersTs(join(racine, 'client/src'));

    const constantes = new Map();
    for (const chemin of fichiers) {
        const texte = readFileSync(chemin, 'utf8');
        for (const m of texte.matchAll(RE_CONSTANTE)) constantes.set(m[1], m[2]);
    }

    const poses = new Map();
    for (const chemin of fichiers) {
        const relatif = relative(racine, chemin).split('\\').join('/');
        const texte = readFileSync(chemin, 'utf8');
        for (const m of texte.matchAll(RE_APPEL)) {
            const token = m[1] ?? constantes.get(m[2]);
            if (!token) continue;
            if (!poses.has(token)) poses.set(token, []);
            poses.get(token).push(relatif);
        }
    }
    return poses;
}
