// Classement des issues d'une tentative d'ouverture de fenêtre.
//
// Trois issues, pas deux. Derrière Pomerium, une session expirée renvoie une
// redirection d'authentification : la fenêtre s'ouvre bel et bien, mais part sur
// l'IdP et ne signale jamais sa vie. Confondre ce cas avec un blocage produirait
// un faux négatif très convaincant — exactement le type de conclusion hâtive que
// les rondes de diagnostic du jalon 1 ont dû défaire quatre fois.

// Durée approximative de l'activation utilisateur transitoire dans Chromium.
export const SEUIL_ACTIVATION_MS = 5000;

/**
 * @param {{ poigneeNulle: boolean | 'sans-objet',
 *           vivante: boolean,
 *           msDepuisGeste: number,
 *           gesteAttendu?: boolean }} observation
 * @returns {'bloque' | 'ouverte-mais-perdue' | 'succes' | 'non-concluant'}
 */
export function classer(observation) {
    const { poigneeNulle, vivante, msDepuisGeste, gesteAttendu = false } = observation;

    // La variante 3 s'appuie délibérément sur un clic : pour elle seule, un geste
    // récent est le mécanisme testé et non un biais.
    if (!gesteAttendu && msDepuisGeste <= SEUIL_ACTIVATION_MS) return 'non-concluant';

    if (poigneeNulle === true) return 'bloque';
    return vivante ? 'succes' : 'ouverte-mais-perdue';
}
