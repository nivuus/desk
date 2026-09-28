// Classification of the outcomes of a window opening attempt.
//
// Three outcomes, not two. Behind Pomerium, an expired session returns an
// authentication redirect: the window does open, but goes off to
// the IdP and never signals it is alive. Confusing this case with a block would produce
// a very convincing false negative — exactly the kind of hasty conclusion that
// the milestone 1 diagnostic rounds had to undo four times.

// Approximate duration of transient user activation in Chromium.
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

    // Variant 3 deliberately relies on a click: for it alone, a recent
    // gesture is the mechanism under test and not a bias.
    if (!gesteAttendu && msDepuisGeste <= SEUIL_ACTIVATION_MS) return 'non-concluant';

    if (poigneeNulle === true) return 'bloque';
    return vivante ? 'succes' : 'ouverte-mais-perdue';
}
