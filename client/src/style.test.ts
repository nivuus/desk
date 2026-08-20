import { describe, expect, it } from 'vitest';
import styleCss from './style.css?raw';
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

/** Les déclarations du bloc dont le prélude est exactement `selecteur`. */
function declarationsDuBloc(selecteur: string): string[] {
    const regles = [...CSS.matchAll(/([^{}]+)\{([^{}]*)\}/g)];
    return regles
        .filter((r) => r[1].trim() === selecteur)
        .flatMap((r) => declarationsDe(`{${r[2]}}`))
        .map((d) => `${d.propriete}: ${d.valeur}`);
}

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
            declarationsDuBloc('#fullscreen[data-actif="true"]'),
            'la règle #fullscreen[data-actif="true"] ne déclare pas pointer-events: none',
        ).toContain('pointer-events: none');
    });
});
