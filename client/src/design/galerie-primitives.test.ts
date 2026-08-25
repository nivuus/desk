import { describe, expect, it } from 'vitest';
import { classesEmployeesHtml } from './classes';
import primitivesHtml from '../../primitives.html?raw';

/**
 * LA LISTE DES CAS QUE LA GALERIE DES PRIMITIVES MONTRE — jamais
 * `galerie-primitives.ts`, qui n'en porte AUCUNE.
 *
 * 🔴 CE FICHIER N'IMPORTE PAS `./galerie-primitives`. Comme `galerie.ts` (voir
 * `galerie.test.ts`), ce module touche `document.getElementById` dès sa
 * première ligne utile, sans dépendance injectée, et `client/` n'a NI jsdom
 * NI happy-dom : l'importer ferait planter le module au chargement.
 *
 * ⚠️ MAIS LA RAISON POUR LAQUELLE UN TEST NE PEUT RIEN DIRE DE PLUS SUR CE
 * FICHIER PRÉCIS N'EST PAS SEULEMENT L'ABSENCE DE DOM. À la différence de
 * `galerie.ts`, dont l'en-tête déclare que la liste des tokens en est
 * PARSÉE, `galerie-primitives.ts` déclare noir sur blanc « il ne lit AUCUN
 * TOKEN » : ses dix-sept lignes ne font qu'un seul branchement du sélecteur
 * de thème (`installerSelecteurDeThemeAuDOM`, DÉJÀ testé en entier par
 * `selecteur-theme.test.ts`, sans aucune dépendance propre à ce module-ci).
 * Il n'y a donc, dans `galerie-primitives.ts` lui-même, ni DOM testable ni
 * liste de cas à figer — les deux raisons pour lesquelles `galerie.test.ts`
 * a quelque chose à dire n'ont ici aucune contrepartie.
 *
 * ⚠️ CE QUE « LA GALERIE DES PRIMITIVES » MONTRE VRAIMENT — quatre familles,
 * bouton/champ/surface/message, chacune avec ses variantes et ses états — est
 * du balisage STATIQUE, écrit à la main dans `client/primitives.html`, jamais
 * engendré par du TypeScript (voir l'en-tête de ce fichier : « la scission a
 * fait naître `client/primitives.html` à côté »). C'est donc LUI qui porte
 * « la liste des cas » de cette galerie — et c'est donc lui que ce fichier
 * fige, pas un module qui n'en sait rien.
 *
 * ⚠️ CE QUI EST DÉJÀ COUVERT AILLEURS, ET N'EST PAS RÉPÉTÉ ICI : que
 * `primitives.css` DÉCLARE bien ces variantes et ces états — les gardes G1 à
 * G7 de `primitives.test.ts`, qui lisent `primitives.css`. Ce fichier-ci
 * vérifie autre chose : que la PAGE DE DÉMONSTRATION les MONTRE. Une
 * primitive déclarée mais absente du balisage ne serait vue par aucun œil
 * humain, et par aucun garde existant non plus — c'est le trou que ce
 * fichier ferme.
 *
 * 🔴 LA ROUGE QUE CE FICHIER SAIT RENDRE : retirer un bloc de démonstration
 * de `primitives.html` (par exemple la ligne `message--danger`) fait tomber
 * le test de sa famille — la primitive a disparu de ce que la galerie
 * montre, et le test le réclame.
 *
 * ⚠️ AUCUN JUGEMENT VISUEL : que le survol se lise, qu'un ton se distingue de
 * son voisin par la seule encre — les jugements humains que l'en-tête de
 * `primitives.html` nomme lui-même (huit de la spec ⑥ §8, plus quatre de S2,
 * quinze à la fin de S3). Lot 4, pas celui-ci.
 *
 * ⚠️ « L'ABSENCE DE DOUBLON » NE FAIT PAS UN TEST SÉPARÉ ICI : les classes
 * sont lues dans un `Set` (aucun doublon possible par construction), et
 * `message--flottant` apparaît deux fois DANS LE BALISAGE — deux longueurs de
 * texte pour la MÊME variante, un choix de démonstration délibéré, pas une
 * redite qu'un test devrait dénoncer.
 */

const CLASSES = classesEmployeesHtml(primitivesHtml);

/** Les noms de `attendues` absents de `CLASSES` — même patron que `etatsManquants` de `primitives.test.ts`. */
function manquantes(attendues: string[]): string[] {
    return attendues.filter((nom) => !CLASSES.has(nom));
}

describe("primitives.html — la galerie des primitives montre bien ce qu'elle promet", () => {
    it('la famille BOUTON : les trois variantes sont dans le balisage', () => {
        expect(
            manquantes(['bouton', 'bouton--principal', 'bouton--secondaire', 'bouton--discret']),
            'variantes de bouton absentes du balisage de démonstration',
        ).toEqual([]);
    });

    it('la famille CHAMP : ses quatre parties, et son état d’erreur', () => {
        expect(
            manquantes([
                'champ',
                'champ--erreur',
                'champ__etiquette',
                'champ__saisie',
                'champ__aide',
                'champ__erreur',
            ]),
            'parties ou état de champ absents du balisage de démonstration',
        ).toEqual([]);
    });

    it('la famille SURFACE : la carte, ses deux parties, et le séparateur', () => {
        expect(
            manquantes(['carte', 'carte__titre', 'carte__corps', 'separateur']),
            'parties de surface absentes du balisage de démonstration',
        ).toEqual([]);
    });

    it('la famille MESSAGE : ses quatre tons, plus la variante flottante', () => {
        expect(
            manquantes([
                'message',
                'message--succes',
                'message--alerte',
                'message--danger',
                'message--flottant',
            ]),
            'tons ou variante de message absents du balisage de démonstration',
        ).toEqual([]);
    });
});
