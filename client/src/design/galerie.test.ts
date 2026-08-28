import { describe, expect, it } from 'vitest';
import { lireBlocsDeTheme, tokensDeclares } from './tokens';
import couleursCss from './tokens/couleurs.css?raw';
import echellesCss from './tokens/echelles.css?raw';

/**
 * LA LISTE DES CAS QUE `galerie.ts` EST CENSÉE MONTRER — jamais SON RENDU.
 *
 * 🔴 CE FICHIER N'IMPORTE PAS `./galerie` ET NE L'EXÉCUTERA JAMAIS.
 * `galerie.ts` touche `document.documentElement` (sa toute première ligne
 * exécutable), puis `document.getElementById` et `getComputedStyle`, SANS
 * AUCUNE DÉPENDANCE INJECTÉE — à l'inverse de la convention que ce dépôt
 * applique ailleurs pour rendre un module qui touche le DOM testable
 * (`client/src/accent-dom.ts`, `client/src/presse-papier-dom.ts`, et
 * `installerSelecteurDeThemeAuDOM` dans `./selecteur-theme.ts`, qui séparent
 * chacun une fonction PURE, à dépendances injectées, d'une COUTURE — seule à
 * toucher `document`/`window`/`localStorage` réels). `galerie.ts` n'a pas
 * cette couture, et `client/` n'a NI jsdom NI happy-dom (`client/package.json`
 * ne porte que `typescript`, `vite`, `vitest` — le même relevé que
 * `selecteur-theme.ts` fait pour lui-même). L'importer ici ferait planter le
 * module AU CHARGEMENT (`document is not defined`), avant la moindre
 * assertion. Rendre ce module testable appartient à un refactor que cette
 * tâche n'a pas mandat de faire ; ce fichier le dit plutôt que de forcer un
 * double DOM maison qui ne prouverait rien de plus que le texte qu'on lui
 * aurait donné.
 *
 * ⚠️ CE QU'IL PEUT ÉTABLIR À LA PLACE. `galerie.ts` déclare, dans son propre
 * en-tête : « la liste des tokens n'est pas écrite ici : elle est PARSÉE de
 * `tokens/couleurs.css` et `tokens/echelles.css` ». Le contrat entre le
 * module et `design.html` tient donc tout entier dans CE QUE CES DEUX
 * FICHIERS DÉCLARENT — figer ce contrat, c'est figer les huit sections que
 * `rendre()` construit (relu dans `galerie.ts`, jamais exécuté ici) : la
 * liste explicite des 14 couleurs de thème, et sept familles par préfixe
 * (`--video-`/`--voile-`, `--t-`, `--lh-`, `--e-`, `--r-`, `--trait`,
 * `--police-`). Chaque test compare une liste ATTENDUE, écrite ici EN DUR et
 * INDÉPENDANTE de `galerie.ts` (jamais réimportée depuis lui — sans quoi un
 * nom retiré À LA FOIS de `galerie.ts` et de sa source resterait invisible,
 * exactement le patron d'« une copie qui valide sa copie » que ce dépôt
 * s'interdit), au texte RÉEL des deux fichiers de tokens.
 *
 * 🔴 LA ROUGE QUE CE FICHIER SAIT RENDRE : retirer un token de
 * `tokens/couleurs.css` ou `tokens/echelles.css` fait tomber le test de sa
 * section — la primitive a disparu de ce que la galerie est censée montrer,
 * et le test le réclame.
 *
 * ⚠️ CE QU'IL NE PEUT PAS ÉTABLIR : que `galerie.ts` respecte encore ce
 * contrat aujourd'hui — si son code de sélection divergeait de la liste
 * ci-dessous SANS QUE LES TOKENS EUX-MÊMES NE BOUGENT, rien ici ne le
 * verrait, faute de pouvoir exécuter le module. Et rien ici ne porte de
 * JUGEMENT VISUEL : que la palette soit sobre, que `#7aa2f7` soit le bon
 * bleu, que le ratio 1,2 soit le bon — les huit jugements humains que la
 * spec ⑥ §8 nomme, et qu'aucun sous-bloc n'a rendus mesurables. C'est le
 * lot 4, pas celui-ci.
 */

const tokensCss = `${couleursCss}\n${echellesCss}`;
const TOKENS = tokensDeclares(tokensCss);

/** Les noms déclarés dont le nom commence par un des préfixes, triés. */
function parPrefixe(...prefixes: string[]): string[] {
    return [...TOKENS].filter((n) => prefixes.some((p) => n.startsWith(p))).sort();
}

describe("galerie.ts — la liste des cas qu'elle est censée montrer (voir l'en-tête)", () => {
    it('« Couleurs » : les quatorze tokens redéclarés par les TROIS blocs de thème', () => {
        // Définition STRUCTURELLE, pas une copie de `galerie.ts::COULEURS` :
        // un token « de thème » est celui que les trois blocs redéclarent
        // TOUS — c'est la règle que l'en-tête de `tokens/couleurs.css` écrit
        // lui-même (« la palette CLAIRE est écrite deux fois »). Les tokens
        // hors thème (`--voile-*`, `--sur-voile`, `--accent-fenetre`) et les
        // échelles ne vivent que dans le bloc `racine` : ils n'entrent jamais
        // dans cette intersection.
        const blocs = lireBlocsDeTheme(tokensCss);
        const parBloc = new Map(blocs.map((b) => [b.nom, new Set(b.tokens.keys())]));
        const racine = parBloc.get('racine');
        const mediaClair = parBloc.get('media-clair');
        const attributClair = parBloc.get('attribut-clair');
        if (!racine || !mediaClair || !attributClair) {
            throw new Error('tokens/couleurs.css ne porte plus les trois blocs de thème attendus');
        }
        const themes = [...racine].filter((n) => mediaClair.has(n) && attributClair.has(n)).sort();

        const ATTENDUS = [
            '--accent',
            '--accent-survol',
            '--alerte',
            '--bord',
            '--bord-fort',
            '--danger',
            '--fond-0',
            '--fond-1',
            '--fond-2',
            '--succes',
            '--sur-accent',
            '--texte',
            '--texte-faible',
            '--texte-fort',
        ].sort();
        expect(themes, 'les 14 couleurs de thème ont changé de nom ou de nombre').toEqual(ATTENDUS);
    });

    it('« Voiles » : six tokens hors thème, `--video-` et `--voile-`', () => {
        const ATTENDUS = [
            '--video-letterbox',
            '--voile-bouton',
            '--voile-bouton-survol',
            '--voile-flottant',
            '--voile-micro-actif',
            '--voile-micro-refuse',
        ].sort();
        expect(parPrefixe('--video-', '--voile-'), 'la section « voiles » de design.html').toEqual(
            ATTENDUS,
        );
    });

    it('« Typographie » : sept crans, `--t-`', () => {
        const ATTENDUS = ['--t-2xl', '--t-3xl', '--t-l', '--t-m', '--t-s', '--t-xl', '--t-xs'].sort();
        expect(parPrefixe('--t-'), 'la section « typographie » de design.html').toEqual(ATTENDUS);
    });

    it('« Interlignes » : trois, `--lh-`', () => {
        const ATTENDUS = ['--lh-large', '--lh-normal', '--lh-serre'].sort();
        expect(parPrefixe('--lh-'), 'la section « interlignes » de design.html').toEqual(ATTENDUS);
    });

    it('« Espacement » : huit crans, `--e-`', () => {
        const ATTENDUS = [
            '--e-1',
            '--e-2',
            '--e-3',
            '--e-4',
            '--e-5',
            '--e-6',
            '--e-7',
            '--e-8',
        ].sort();
        expect(parPrefixe('--e-'), 'la section « espacement » de design.html').toEqual(ATTENDUS);
    });

    it('« Rayons » : quatre, `--r-`', () => {
        const ATTENDUS = ['--r-1', '--r-2', '--r-3', '--r-plein'].sort();
        expect(parPrefixe('--r-'), 'la section « rayons » de design.html').toEqual(ATTENDUS);
    });

    it('« Traits » : deux épaisseurs, préfixe `--trait` (sans tiret : couvre aussi `--trait-focus`)', () => {
        const ATTENDUS = ['--trait', '--trait-focus'].sort();
        expect(parPrefixe('--trait'), 'la section « traits » de design.html').toEqual(ATTENDUS);
    });

    it('« Polices » : deux piles système, `--police-`', () => {
        const ATTENDUS = ['--police-mono', '--police-ui'].sort();
        expect(parPrefixe('--police-'), 'la section « polices » de design.html').toEqual(ATTENDUS);
    });
});
