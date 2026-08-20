import { describe, expect, it } from 'vitest';
import primitivesCss from './primitives.css?raw';
import boutonCss from './primitives/bouton.css?raw';
import champCss from './primitives/champ.css?raw';
import surfaceCss from './primitives/surface.css?raw';
import messageCss from './primitives/message.css?raw';
import baseCss from './base.css?raw';

/**
 * 🔴 LES GARDES LISENT LES FAMILLES, PAS `primitives.css`, qui n'est plus
 * qu'une liste d'`@import` depuis l'extraction de T4. Le lire seul ferait
 * mesurer ZÉRO règle aux quatre gardes d'absence — c'est G5 qui l'a attrapé,
 * ROUGE, à l'extraction même.
 *
 * ⚠️ TOUTE FAMILLE NEUVE S'AJOUTE ICI **ET** DANS `primitives.css` : G5 compare
 * les deux listes, si bien qu'une cinquième famille importée sans être lue ici
 * — donc hors de portée de G1 à G4 — fait tomber le garde.
 */
const FAMILLES = new Map([
    ['./primitives/bouton.css', boutonCss],
    ['./primitives/champ.css', champCss],
    ['./primitives/surface.css', surfaceCss],
    ['./primitives/message.css', messageCss],
]);

/**
 * LES GARDES DE FORME DES PRIMITIVES — sous-projet ⑥, sous-bloc S2.
 *
 * 🔴 CE FICHIER NE LIT UN TEXTE NON VIDE QUE GRÂCE À `test: { css: true }` de
 * `client/vite.config.ts`. Sans cette ligne, Vitest court-circuite les fichiers
 * CSS — la requête `?raw` comprise — et `primitivesCss` vaut la chaîne VIDE :
 * les gardes ① à ④ ci-dessous, qui sont des tests d'ABSENCE, passeraient tous
 * au vert EN NE MESURANT RIEN. C'est aussi pourquoi il n'y a délibérément pas
 * de `client/vitest.config.ts` : il prendrait le pas sur la configuration Vite
 * sans rien dire.
 *
 * 🔴 ET C'EST EXACTEMENT POURQUOI G5 EXISTE. G1 à G4 cherchent l'absence de
 * quelque chose ; un `primitives.css` réduit à son en-tête les satisferait tous
 * les quatre. G5 est le garde d'ATTEIGNABILITÉ : il exige qu'il y ait quelque
 * chose à mesurer.
 *
 * 🔴 LE BLANCHIMENT DES COMMENTAIRES N'EST PAS UN DÉTAIL. L'en-tête de
 * `primitives.css` explique POURQUOI `outline: none`, `opacity` et les
 * longueurs littérales y sont interdits — il ÉCRIT donc ces chaînes. Un garde
 * qui les chercherait dans le texte brut serait satisfait par sa propre
 * justification, et resterait vert sur un fichier dont la règle a été retirée.
 * C'est mot pour mot ce qui est arrivé au garde de l'amorce en S1.
 *
 * ⚠️ MAIS SA PORTÉE RÉELLE EST PLUS ÉTROITE QUE CELA, ET ELLE EST MESURÉE —
 * l'écrire large serait affirmer au-delà du relevé. Neutraliser le blanchiment
 * fait tomber G1 (et donc G5, qui lit les mêmes préludes) : le balayage
 * `preludes` prend le texte d'un commentaire précédant un `{` pour une liste de
 * sélecteurs, et G1 remonte alors 889 compounds parasites dont `/*` (⚠️ 672 à
 * la tâche 2, quand `bouton.css` était seul : le nombre a grossi avec les trois
 * familles suivantes, et il a été REMESURÉ le 20 août 2026 plutôt que recopié).
 * G2, G3 et
 * G4 restent VERTS sans lui, y compris avec un `outline: none;` écrit dans un
 * commentaire posé À L'INTÉRIEUR d'un bloc (essayé) : ces trois-là ne cherchent
 * pas une sous-chaîne, ils lisent une POSITION DE PROPRIÉTÉ dans une
 * déclaration, et un `/* … outline` n'en est pas une. Le blanchiment reste
 * requis — il est le seul rempart de G1 et de G5 —, et c'est la façon dont G2 à
 * G4 sont écrits qui les met hors d'atteinte du piège, pas lui.
 */

/** Retire les commentaires `/* … *​/` — voir l'en-tête. */
function sansCommentaires(css: string): string {
    return css.replace(/\/\*[\s\S]*?\*\//g, ' ');
}

/** Tout ce qui précède un `{`. Les at-rules (`@media …`) commencent par `@`. */
function preludes(css: string): string[] {
    return [...css.matchAll(/([^{}]+)\{/g)].map((m) => m[1].trim()).filter(Boolean);
}

interface Declaration {
    propriete: string;
    valeur: string;
}

/**
 * Les déclarations des blocs les plus intérieurs. `[^{}]*` ne franchit ni `{`
 * ni `}` : le corps d'un `@media` n'est donc jamais pris pour une déclaration.
 */
function declarationsDe(css: string): Declaration[] {
    const sortie: Declaration[] = [];
    for (const bloc of css.matchAll(/\{([^{}]*)\}/g)) {
        for (const morceau of bloc[1].split(';')) {
            const coupe = morceau.indexOf(':');
            if (coupe === -1) continue;
            sortie.push({
                propriete: morceau.slice(0, coupe).trim(),
                valeur: morceau.slice(coupe + 1).trim(),
            });
        }
    }
    return sortie;
}

/** Le corps du bloc qui suit `index`, accolades appariées. */
function blocApres(css: string, index: number): string {
    const debut = css.indexOf('{', index);
    if (debut === -1) return '';
    let profondeur = 0;
    for (let i = debut; i < css.length; i += 1) {
        if (css[i] === '{') profondeur += 1;
        else if (css[i] === '}') {
            profondeur -= 1;
            if (profondeur === 0) return css.slice(debut + 1, i);
        }
    }
    return '';
}

const CSS = sansCommentaires([...FAMILLES.values()].join('\n'));
const SELECTEURS = preludes(CSS).filter((p) => !p.startsWith('@'));
const DECLARATIONS = declarationsDe(CSS);

/** `.carte > .carte__titre` rend `['.carte', '.carte__titre']`. */
const compounds = (selecteur: string) => selecteur.split(/[\s>+~]+/).filter(Boolean);

/** Les listes de sélecteurs qui citent une famille — l'outil de G5 et de G6. */
const famille = (nom: string) => SELECTEURS.filter((s) => s.includes(`.${nom}`));

/**
 * G6 — les états et les parties qu'une famille doit déclarer.
 *
 * 🔴 IL LIT DES SÉLECTEURS, PAS DE LA PROSE. Une règle retirée fait tomber ce
 * garde ; le COMMENTAIRE qui la mentionne, blanchi d'entrée, ne le retient pas.
 * C'est la seule façon de savoir qu'il mesure du code — S1 a payé deux fois un
 * garde satisfait par sa propre justification.
 *
 * ⚠️ CE QU'IL NE DIT PAS : que l'état soit BIEN DIT. Qu'un champ en erreur se
 * distingue, qu'un désactivé se lise comme inerte — ce sont des jugements
 * humains du §8, et aucun ne deviendra une mesure.
 */
function etatsManquants(nom: string, attendus: string[]): string[] {
    // 🔴 `:not(…)` EST RETIRÉ AVANT LA RECHERCHE, ET CE N'EST PAS UNE FINESSE :
    // sans cela, `.champ__saisie:hover:not(:disabled)` contient la sous-chaîne
    // `:disabled`, et le garde ne peut plus échouer quand la RÈGLE
    // `.champ__saisie:disabled` disparaît. Mesuré : la rouge de T3 n'a d'abord
    // rien fait tomber, sur une règle réellement retirée. C'est le patron du
    // contrôle vacueux, attrapé ici sur le garde lui-même.
    const selecteurs = famille(nom)
        .map((s) => s.replace(/:not\([^)]*\)/g, ''))
        .join('  ');
    return attendus.filter((etat) => !selecteurs.includes(etat));
}

describe('primitives.css — les gardes de forme', () => {
    it('G1 — aucun sélecteur d’élément nu : tout compound porte une classe', () => {
        // 🔴 C'EST CE GARDE QUI TIENT LA NEUTRALITÉ D'`index.html`. La fenêtre
        // de session porte cinq éléments sans aucune classe de primitive, dont
        // DEUX `<button>` : un `button { … }` écrit ici changerait son
        // apparence sans qu'aucun des huit contrôles ne le dise.
        const nus: string[] = [];
        for (const liste of SELECTEURS) {
            for (const selecteur of liste.split(',')) {
                for (const compound of compounds(selecteur.trim())) {
                    if (!compound.includes('.')) nus.push(compound);
                }
            }
        }
        expect(nus, 'sélecteurs d’élément nus dans primitives.css').toEqual([]);
    });

    it('G2 — l’anneau de focus n’est jamais effacé', () => {
        // `base.css` pose `:focus-visible` GLOBALEMENT : aucune primitive n'a
        // à le déclarer, et le seul risque est qu'une d'elles l'efface « pour
        // faire propre ». Aucun des huit contrôles ne le verrait.
        const effacements = DECLARATIONS.filter(
            (d) =>
                /^outline(-(width|style))?$/i.test(d.propriete) &&
                /^(none|0|0px|0rem|0em)$/i.test(d.valeur),
        ).map((d) => `${d.propriete}: ${d.valeur}`);
        expect(effacements, 'effacements de l’anneau de focus').toEqual([]);
    });

    it('G3 — aucun état ne se dit par une composition d’exécution', () => {
        // `opacity` et `filter` composent la couleur AU RENDU : la teinte
        // effective échappe alors aux 52 paires du contrôle §7.1. Un état
        // désactivé exprimé par une opacité serait le seul état du produit
        // dont le contraste ne serait mesuré par rien.
        const compositions = DECLARATIONS.filter((d) =>
            ['opacity', 'filter', 'backdrop-filter'].includes(d.propriete.toLowerCase()),
        ).map((d) => `${d.propriete}: ${d.valeur}`);
        expect(compositions, 'compositions d’exécution dans primitives.css').toEqual([]);
    });

    it('G4 — aucune longueur hors échelle : toute unité passe par un token', () => {
        // ⚠️ CE GARDE NE DIT PAS QUE LE BON TOKEN A ÉTÉ CHOISI. Il dit
        // qu'aucune longueur ne s'écrit hors des échelles du §4.4 — ce
        // qu'aucun des huit contrôles ne mesure, puisque aucun ne mesure une
        // longueur.
        const hors: string[] = [];
        for (const d of DECLARATIONS) {
            const reste = d.valeur.replace(/var\(\s*--[a-z0-9-]+\s*\)/gi, ' ');
            const trouve = reste.match(/(\d+(?:\.\d+)?)(px|rem|em|ms|s|pt|ch|vw|vh)\b/);
            if (trouve) hors.push(`${d.propriete}: ${d.valeur} → « ${trouve[0]} » hors token`);
        }
        expect(hors, 'longueurs littérales dans primitives.css').toEqual([]);
    });

    it('G5 — atteignabilité : le fichier déclare des règles, dont la famille bouton', () => {
        // 🔴 SANS CE GARDE, LES QUATRE PRÉCÉDENTS NE PROUVENT RIEN : ce sont
        // des tests d'absence, et un fichier vide les satisfait tous.
        expect(
            SELECTEURS.length,
            'primitives.css ne déclare AUCUNE règle : G1 à G4 sont alors verts en ne mesurant rien',
        ).toBeGreaterThan(0);
        expect(famille('bouton'), 'la famille .bouton est absente de primitives.css').not.toEqual(
            [],
        );
        // 🔴 ET QUE CE FICHIER LISE BIEN TOUT CE QUE `primitives.css` IMPORTE :
        // une famille importée mais absente de `FAMILLES` échapperait à G1, G2,
        // G3 et G4 sans qu'aucune commande ne le dise.
        const importees = [...primitivesCss.matchAll(/@import\s+'([^']+)'/g)].map((m) => m[1]);
        expect(importees.sort(), 'les familles importées et celles que ce test lit divergent').toEqual(
            [...FAMILLES.keys()].sort(),
        );
    });

    it('G6 — la famille CHAMP déclare ses états et ses parties', () => {
        expect(
            etatsManquants('champ', [
                '.champ__etiquette',
                '.champ__saisie',
                '.champ__aide',
                '.champ__erreur',
                '::placeholder',
                ':disabled',
                '.champ--erreur',
            ]),
            'états ou parties absents de la famille champ',
        ).toEqual([]);
    });

    it('G6 — la famille SURFACE déclare ses parties, et le séparateur', () => {
        expect(
            etatsManquants('carte', ['.carte__titre', '.carte__corps']),
            'parties absentes de la famille carte',
        ).toEqual([]);
        expect(famille('separateur'), 'le séparateur est absent de primitives').not.toEqual([]);
    });

    it('G6 — la famille MESSAGE déclare ses quatre tons', () => {
        // Le ton NEUTRE est `.message` elle-même : les trois autres ne
        // changent que l'encre et le trait.
        expect(
            etatsManquants('message', ['.message--succes', '.message--alerte', '.message--danger']),
            'tons absents de la famille message',
        ).toEqual([]);
    });

    it('G7 — base.css neutralise les transitions sous prefers-reduced-motion', () => {
        // 🔴 LE BLANCHIMENT EST ICI STRICTEMENT NÉCESSAIRE : l'en-tête de la
        // règle RECOPIE la commande de mesure qui l'a imposée, donc la chaîne
        // `@media (prefers-reduced-motion: reduce)` en toutes lettres, ET le
        // bloc `{ :root { --duree-1: 0.01ms; } }` qui la suit dans le relevé.
        //
        // ❌ « Un garde qui la chercherait dans le texte brut resterait VERT sur
        // un `base.css` dont la règle a été retirée » — écrit ici par la tâche 6
        // et RÉFUTÉ PAR MESURE le 20 août 2026 (revue transverse, journal
        // `journaux-design-s2/rouges-rejouees.log`). Il ne reste pas vert : sa
        // PREMIÈRE assertion est bien satisfaite par le commentaire, mais la
        // SECONDE lit alors le bloc du relevé et tombe —
        //   `expected [ '--duree-1' ] to include 'transition-duration'`.
        // ⚠️ ET C'EST PIRE QUE CE QUE L'ÉNONCÉ FAUX DÉCRIVAIT : sans blanchiment,
        // ce garde rend CE MÊME ROUGE que la règle soit PRÉSENTE ou ABSENTE —
        // il cesse de discriminer, et devient un faux positif sur un `base.css`
        // parfaitement correct. Le blanchiment n'est pas ce qui l'empêche d'être
        // vert à tort : c'est ce qui le rend capable de dire quoi que ce soit.
        const base = sansCommentaires(baseCss);
        const debut = base.search(/@media\s*\(\s*prefers-reduced-motion\s*:\s*reduce\s*\)/);
        expect(
            debut,
            'aucune requête @media (prefers-reduced-motion: reduce) dans base.css, commentaires blanchis',
        ).toBeGreaterThan(-1);
        expect(
            declarationsDe(blocApres(base, debut)).map((d) => d.propriete),
            'la requête de mouvement réduit ne porte aucune déclaration',
        ).toContain('transition-duration');
    });
});
