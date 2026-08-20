import { describe, expect, it } from 'vitest';
import styleCss from './style.css?raw';
import etatTerminalCss from './session/etat-terminal.css?raw';
import { declarationsDe, preludes, sansCommentaires } from './design/css';

/**
 * ═══════════════════════════════════════════════════════════════════════════
 * LES GARDES DE LA FEUILLE DE SESSION — sous-projet ⑥, sous-bloc S4, tâche 7.
 *
 * 🔴 IL EXISTE POUR TRANSFORMER EN COMMANDE UNE PROPRIÉTÉ QUE LA SPEC PROTÈGE
 * PAR UNE PHRASE. Le §5.2 écrit : « Toute reprise du bouton en S4 doit
 * conserver cette propriété » — `pointer-events: none` sur
 * `#fullscreen[data-actif="true"]` —, « changer sa taille change la zone qu'il
 * occupe, et la justification est écrite en fonction d'elle ». Une phrase de
 * spécification n'attrape rien ; ce fichier, si.
 *
 * ⚠️ IL EST ÉCRIT ET VU ROUGE **AVANT** LE CHANGEMENT DE TAILLE QU'IL PROTÈGE,
 * et l'ordre est portant : un garde écrit APRÈS le changement ne prouve pas
 * qu'il aurait attrapé la régression. C'est la leçon de la tâche 6 de D9 —
 * traiter la marge AVANT l'addition, pas après.
 *
 * 🔴 ANCRÉ SUR LA DÉCLARATION, JAMAIS SUR UNE SOUS-CHAÎNE, et ce dépôt a payé
 * ce piège TROIS FOIS (S1 sur `CLE_THEME`, S2 sur G1 et G5, S3 sur sa rouge
 * n°16). Le commentaire de `style.css` qui justifie cette règle ÉCRIT les mots
 * `pointer-events` : un garde qui les chercherait dans le texte brut serait
 * satisfait par la justification du fichier qu'il analyse, et resterait vert
 * sur une feuille dont la règle a été retirée. Le blanchiment de `./design/css`
 * est donc obligatoire, et la contre-épreuve est jouée : une mutation qui ne
 * touche QUE le commentaire laisse ce garde VERT.
 *
 * 🔴 CE FICHIER NE LIT UN TEXTE NON VIDE QUE GRÂCE À `test: { css: true }` de
 * `client/vite.config.ts`, et il doit être lancé DEPUIS `client/` : depuis la
 * racine du dépôt, la racine Vite change, le CSS est court-circuité EN SILENCE
 * et `styleCss` vaut la chaîne vide. C'est l'assertion d'atteignabilité qui
 * attrape ce cas — voir `design/longueurs.test.ts`, qui l'a payé.
 * ═══════════════════════════════════════════════════════════════════════════
 */

const CSS = sansCommentaires(styleCss);
const SELECTEURS = preludes(CSS).filter((p) => !p.startsWith('@'));
const CSS_TERMINAL = sansCommentaires(etatTerminalCss);

/** Les déclarations du bloc dont le prélude est exactement `selecteur`. */
function declarationsDuBloc(css: string, selecteur: string): string[] {
    const regles = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)];
    return regles
        .filter((r) => r[1].trim() === selecteur)
        .flatMap((r) => declarationsDe(`{${r[2]}}`))
        .map((d) => `${d.propriete}: ${d.valeur}`);
}

/**
 * ═══════════════════════════════════════════════════════════════════════════
 * LE GARDE DU WINDOW CONTROLS OVERLAY — sous-bloc S4, tâche 10.
 *
 * 🔴 IL PROUVE QUE LA RÈGLE WCO EST INERTE AUJOURD'HUI, ET RIEN D'AUTRE. Il
 * n'existe AUCUN manifeste dans ce dépôt — donc `display_override:
 * ["window-controls-overlay"]` n'est déclaré nulle part, les variables
 * `titlebar-area-*` ne sont jamais définies, et AUCUN ÉTAT ATTEIGNABLE ne fait
 * agir la règle. Un critère de recette qui prétendrait l'exercer serait vacueux
 * PAR CONSTRUCTION, et pas faute d'effort : c'est pourquoi S4 n'en prescrit
 * aucun. Ce que ce garde tient est une propriété de FORME dont la rouge, elle,
 * a une conséquence RÉELLE — écrire `env(titlebar-area-height, 8px)` descend le
 * bandeau de 8 px MAINTENANT, sur le produit tel qu'il tourne. Ce n'est donc
 * pas un contrôle qui valide sa propre écriture.
 *
 * 🔴 CE QU'IL NE PROUVE PAS : rien du comportement SOUS WCO. La règle n'a jamais
 * été rendue dans une fenêtre à barre de titre superposée. Le destinataire de ce
 * legs est NOMMÉ : la recette du sous-bloc G5 de la gestion d'apps, celui qui
 * pose le manifeste — c'est à elle de regarder la fenêtre de session sous une
 * barre superposée.
 *
 * ── POURQUOI ② INTERDIT UNE REQUÊTE MÉDIA PLUTÔT QUE DE LA VÉRIFIER ────────
 * Un repli neutralise un `env()` ; RIEN ne neutralise un bloc `@media`. Une
 * règle conditionnelle au WCO écrite en `@media (display-mode:
 * window-controls-overlay)` pourrait donc changer la mise en page d'AUJOURD'HUI
 * sans qu'aucune commande ne le dise. L'interdire rend la propriété « inerte
 * aujourd'hui » TOTALE au lieu de partielle.
 *
 * ⚠️ LA PORTÉE EST DÉRIVÉE, JAMAIS ÉNUMÉRÉE : toutes les `*.css` de
 * `client/src/`, socle et primitives compris. Une feuille neuve entre donc dans
 * ce garde sans qu'une ligne d'ici ne change, et une liste recopiée ne peut pas
 * diverger de ce qu'elle décrit.
 *
 * ⚠️ TOUT EST LU APRÈS BLANCHIMENT, dans les DEUX SENS : un `env(titlebar-area-
 * height, 8px)` écrit dans un COMMENTAIRE ne fait pas rougir ①, et un
 * `env(titlebar-area-` qui ne vivrait que dans un commentaire ne satisfait PAS
 * l'atteignabilité ③. C'est le piège que ce dépôt a payé trois fois, et
 * l'encadré de `./style.css` écrit précisément ces chaînes-là.
 * ═══════════════════════════════════════════════════════════════════════════
 */
const FEUILLES_SRC = import.meta.glob<string>('./**/*.css', {
    query: '?raw',
    import: 'default',
    eager: true,
});

/** `./session/etat-terminal.css` → `client/src/session/etat-terminal.css`. */
const cheminSrc = (cle: string) => cle.replace(/^\.\//, 'client/src/');

/** Tout `env(titlebar-area-…)` des feuilles, avec sa queue d'arguments. */
const envs: { fichier: string; texte: string; repli: string }[] = [];
/** Tout prélude d'at-rule conditionnel au WCO. */
const requetesWco: { fichier: string; prelude: string }[] = [];

for (const [cle, brut] of Object.entries(FEUILLES_SRC).sort()) {
    const css = sansCommentaires(brut);
    for (const m of css.matchAll(/env\(\s*(titlebar-area-[a-z-]+)\s*([^)]*)\)/g)) {
        envs.push({ fichier: cheminSrc(cle), texte: `env(${m[1]}${m[2]})`, repli: m[2].trim() });
    }
    for (const p of preludes(css)) {
        if (/display-mode\s*:\s*window-controls-overlay/.test(p)) {
            requetesWco.push({ fichier: cheminSrc(cle), prelude: p });
        }
    }
}

// Le relevé, toujours imprimé, succès compris — « un contrôle de dérive dont on
// ne lit jamais la valeur ne sert qu'à passer » (`poids-css.mjs`).
console.log(`garde WCO  feuilles de client/src/ : ${Object.keys(FEUILLES_SRC).length}`);
console.log(`           env(titlebar-area-*) lus : ${envs.length}`);
for (const e of envs) console.log(`           ${e.fichier}  ${e.texte}`);

describe('le Window Controls Overlay — la règle est livrée, et elle est INERTE', () => {
    it('③ atteignabilité : il existe au moins un env(titlebar-area-) à lire', () => {
        // 🔴 SANS CETTE ASSERTION, ① EST VERTE EN NE MESURANT RIEN — et ② l'est
        // de toute façon, puisqu'elle nie. Sa rouge se joue en VIDANT les
        // feuilles qui portent la règle.
        // ⚠️ ET IL FAUT LES VIDER TOUTES : la portée est dérivée sur toutes les
        // `*.css` de `client/src/`, donc vider `style.css` SEULE laisse
        // `session/etat-terminal.css` porter son `env()` et l'atteignabilité
        // reste VERTE, à juste titre. C'est le défaut de prescription que
        // `design/longueurs.test.ts` a déjà mesuré sur sa propre rouge.
        expect(
            envs.length,
            'aucun env(titlebar-area-) dans client/src/ : le garde WCO est vert en ne mesurant rien',
        ).toBeGreaterThan(0);
    });

    it('① tout env(titlebar-area-*) porte le repli 0px', () => {
        expect(
            envs.filter((e) => e.repli !== ', 0px').map((e) => `${e.fichier}  ${e.texte}`),
            'un env(titlebar-area-*) sans le repli « , 0px » change la mise en page AUJOURD’HUI',
        ).toEqual([]);
    });

    it('② aucune requête @media conditionnelle au WCO', () => {
        // Un repli neutralise un `env()` ; rien ne neutralise un bloc `@media`.
        expect(
            requetesWco.map((r) => `${r.fichier}  ${r.prelude}`),
            'une @media (display-mode: window-controls-overlay) peut changer la mise en page sans qu’aucune commande ne le dise',
        ).toEqual([]);
    });
});

describe('style.css — les gardes de la fenêtre de session', () => {
    it('② atteignabilité : la feuille déclare des règles', () => {
        // 🔴 SANS CETTE ASSERTION, LA SUIVANTE EST VERTE SUR UNE FEUILLE VIDE.
        // C'est G5 de `primitives.test.ts`, et le piège que S2 a mesuré :
        // « quatre gardes sur cinq ne prouveraient rien ». Sa rouge se joue en
        // VIDANT `client/src/style.css`, jamais en y ajoutant quelque chose.
        expect(
            SELECTEURS.length,
            'style.css ne déclare AUCUNE règle : le garde ① est alors vert en ne mesurant rien',
        ).toBeGreaterThan(0);
    });

    it('① le bouton plein écran actif est HORS du flux d’événements', () => {
        // La raison, écrite dans `style.css` et reprise ici pour qu'elle
        // survive à une lecture de ce seul fichier : sans cette déclaration,
        // une zone de coin reste dans le flux d'événements et avale les clics
        // destinés au jeu (minimap, boutique…) en plein écran absolu. La sortie
        // du plein écran reste possible par appui long sur Échap (Keyboard
        // Lock), donc retirer le bouton du flux ne piège personne.
        //
        // ⚠️ CE GARDE TIENT LA PROPRIÉTÉ, PAS LA GÉOMÉTRIE. Aucune page n'est
        // ouverte, aucun pixel n'est mesuré, et l'avance du glyphe `⛶` reste
        // inconnue de tous ici. Que la zone occupée soit la bonne est un
        // JUGEMENT HUMAIN (spec §8), et il n'a pas été porté.
        expect(
            declarationsDuBloc(CSS, '#fullscreen[data-actif="true"]'),
            'la règle #fullscreen[data-actif="true"] ne déclare pas pointer-events: none',
        ).toContain('pointer-events: none');
    });

    it('③ l’écran terminal reste MASQUÉ tant qu’il porte `hidden`', () => {
        // 🔴 `[hidden]` PERD CONTRE UNE RÈGLE D'AUTEUR. Le
        // `[hidden] { display: none }` qui rend l'attribut efficace vit dans la
        // feuille de l'agent utilisateur, et la cascade compare l'ORIGINE avant
        // la spécificité : `.ecran { display: grid }` l'emporte, fût-elle moins
        // spécifique. Sans la règle explicite que cette assertion exige,
        // l'écran plein cadre serait VISIBLE DÈS LE CHARGEMENT, sur toutes les
        // sessions, par-dessus la vidéo — et aucun autre contrôle ne le verrait.
        //
        // ⚠️ ANCRÉE SUR LA RÈGLE ENTIÈRE : le sélecteur est comparé par égalité
        // exacte, et la déclaration est lue APRÈS blanchiment. Un `.ecran[hidden]`
        // écrit dans un commentaire ne satisfait donc pas ce garde — c'est le
        // piège que ce dépôt a payé trois fois.
        expect(
            declarationsDuBloc(CSS_TERMINAL, '.ecran[hidden]'),
            'session/etat-terminal.css ne déclare pas .ecran[hidden] { display: none }',
        ).toContain('display: none');
    });
});
