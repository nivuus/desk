/**
 * LES TROIS BOUTONS DE THÈME — extraits de `galerie.ts` AVANT que la seconde
 * galerie n'existe, et non après.
 *
 * ⚠️ CE MODULE N'EST PAS DU PRODUIT, ET IL N'A PAS DE TEST. C'est du code
 * d'INSTRUMENT : il ne sert que les pages du jugement humain du §8. Ce qu'il
 * aurait de testable — la machine à trois états — vit déjà dans `theme.ts`,
 * qui porte les dix tests du contrôle §7.5. Ce statut est DÉCLARÉ, pas subi :
 * c'est le même que celui de `galerie.ts`, et pour la même raison.
 *
 * ⚠️ CE N'EST PAS LE SÉLECTEUR DE THÈME DU PRODUIT. Celui-là appartient à S3
 * (spec §5.2, famille 3), qui décidera s'il le réemploie tel quel ou le
 * réécrit avec les primitives. Rien ici n'engage ce choix.
 *
 * ⚠️ PUR DE TOUTE GALERIE : il ne connaît ni token, ni couleur, ni mise en
 * page. Il ne sait qu'une chose de son appelant — qu'il veut être rappelé
 * après un changement de thème, quelle qu'en soit l'origine —, et c'est le
 * paramètre `apres`.
 *
 * ⚠️ `Coffre` et `Racine` sont IMPORTÉS de `theme.ts`, jamais redéclarés ici.
 * Deux copies d'un contrat divergent en silence ; c'est ce que le §7.1 refuse
 * aux valeurs, et il n'y a pas de raison de l'accorder aux types.
 */
import { CLE_THEME, appliquer, choisir, surStockageModifie, themeStocke } from './theme';
import type { Coffre, Racine, Theme } from './theme';

/**
 * Pose les trois boutons dans `hote`, applique l'état stocké, et branche
 * l'écoute de `storage`. `apres` est appelé après chaque changement de thème,
 * quelle qu'en soit l'origine — c'est par là que la galerie de tokens redessine
 * les valeurs qu'elle lit par `getComputedStyle`.
 *
 * ⚠️ `hote` N'EST PAS VIDÉ : deux appels sur le même élément y poseraient six
 * boutons. C'est l'appelant qui décide, comme avant l'extraction où
 * `galerie.ts` passait par son `vide('themes')`.
 */
export function installerSelecteurDeTheme(
    hote: HTMLElement,
    racine: Racine,
    coffre: Coffre,
    apres: () => void,
): void {
    const etats: Theme[] = ['systeme', 'clair', 'sombre'];
    const marquer = () => {
        const courant = themeStocke(coffre);
        for (const bouton of hote.querySelectorAll('button')) {
            bouton.setAttribute('aria-pressed', String(bouton.dataset.theme === courant));
        }
    };

    // L'amorce a déjà posé l'attribut avant la première peinture ; ce rappel
    // couvre le cas où le stockage a changé entre l'amorce et l'exécution de ce
    // module.
    appliquer(racine, themeStocke(coffre));

    // La bascule venue d'une AUTRE fenêtre — c'est la moitié que `choisir()` ne
    // peut pas couvrir, `storage` ne se déclenchant jamais chez l'écrivain.
    //
    // ⚠️ IL NE RAPPELLE PAS `marquer()`, ET C'EST LA TRANSPOSITION VERBATIM DU
    // COMPORTEMENT D'AVANT L'EXTRACTION : un `aria-pressed` posé ici reste
    // celui du thème d'avant tant que l'utilisateur ne clique pas dans CETTE
    // fenêtre. Le défaut est réel et il est PRÉEXISTANT ; une tâche
    // d'extraction qui le corrigerait rendrait fausse sa propre preuve que
    // rien n'a bougé.
    window.addEventListener('storage', (evenement) => {
        if (evenement.key !== CLE_THEME) return;
        surStockageModifie(racine, evenement.key, evenement.newValue);
        apres();
    });

    for (const etat of etats) {
        const bouton = document.createElement('button');
        bouton.type = 'button';
        bouton.dataset.theme = etat;
        bouton.textContent = etat;
        bouton.addEventListener('click', () => {
            choisir(coffre, racine, etat);
            marquer();
            apres();
        });
        hote.append(bouton);
    }
    marquer();
}
