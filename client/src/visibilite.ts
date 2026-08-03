import { encodeVisibility } from '../../proto/ts/control';

/**
 * Ce dont ce module a besoin du document. Réduit à sa plus simple expression
 * pour être simulable : `document` réel en production, objet nu en test.
 */
export interface CibleVisibilite {
    // Non `readonly` : le double-emploi comme type de retour de la fabrique
    // de test (`cibleFactice`, qui mute ces champs entre deux déclenchements
    // d'événement) l'interdirait sinon — `document`, lui, ne satisfait
    // l'interface qu'au travers d'accesseurs `get`, jamais en écriture.
    hidden: boolean;
    focalisee: boolean;
    addEventListener(nom: string, rappel: () => void): void;
    removeEventListener(nom: string, rappel: () => void): void;
}

/**
 * Annonce visibilité et focus à l'agent, et rend de quoi se détacher.
 *
 * **Pourquoi le focus en plus de la visibilité.** Le vivier d'encodeurs de
 * l'agent arbitre par récence ; entre dix fenêtres toutes visibles, la
 * visibilité seule ne les ordonnerait pas et l'éviction serait arbitraire.
 *
 * **Ce que `visibilityState` ne rapporte PAS** : sur Chrome/Linux, une fenêtre
 * entièrement recouverte par une autre reste `visible`. Le sommeil s'y déclenche
 * donc à la minimisation et à l'onglet caché, pas au recouvrement.
 *
 * **`envoyer` rend un booléen** : `true` si l'envoi a réellement eu lieu,
 * `false` s'il a été abandonné (canal de contrôle pas encore ouvert, le plus
 * souvent au tout premier appel, fait AVANT toute connexion WebRTC). `dernier`
 * n'est mémorisé que sur un envoi réussi — sans cette garde, un premier envoi
 * perdu marquerait l'état courant comme déjà annoncé, et tant que la
 * visibilité ne changerait pas ensuite, elle ne serait plus JAMAIS réémise :
 * la fenêtre resterait endormie côté agent pour toujours, sans aucun `WARN`
 * pour le signaler.
 */
export function attachVisibilite(
    cible: CibleVisibilite,
    envoyer: (charge: string) => boolean,
): () => void {
    let dernier: string | undefined;

    const annoncer = () => {
        const charge = encodeVisibility(!cible.hidden, cible.focalisee);
        // Le canal de contrôle est fiable et ordonné : réémettre un état
        // inchangé n'apporte rien, et un navigateur émet volontiers plusieurs
        // événements pour un seul geste.
        if (charge === dernier) return;
        if (envoyer(charge)) dernier = charge;
    };

    cible.addEventListener('visibilitychange', annoncer);
    cible.addEventListener('focus', annoncer);
    cible.addEventListener('blur', annoncer);
    annoncer();

    return () => {
        cible.removeEventListener('visibilitychange', annoncer);
        cible.removeEventListener('focus', annoncer);
        cible.removeEventListener('blur', annoncer);
    };
}
