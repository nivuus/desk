import { encodeVisibility } from '../../proto/ts/control';

/**
 * What this module needs from the document. Reduced to its simplest expression
 * to be simulable: the real `document` in production, a bare object in tests.
 */
export interface CibleVisibilite {
    // Not `readonly`: the double use as the return type of the test
    // factory (`cibleFactice`, which mutates these fields between two event
    // firings) would otherwise forbid it — `document`, for its part, only satisfies
    // the interface through `get` accessors, never in writing.
    hidden: boolean;
    focalisee: boolean;
    addEventListener(nom: string, rappel: () => void): void;
    removeEventListener(nom: string, rappel: () => void): void;
}

/**
 * Announces visibility and focus to the agent, and returns a way to detach.
 *
 * **Why focus on top of visibility.** The agent's encoder pool
 * arbitrates by recency; among ten windows all visible,
 * visibility alone would not order them and eviction would be arbitrary.
 *
 * **What `visibilityState` does NOT report**: on Chrome/Linux, a window
 * entirely covered by another stays `visible`. Sleep is therefore triggered
 * on minimisation and on a hidden tab, not on covering.
 *
 * **`envoyer` returns a boolean**: `true` if the send really took place,
 * `false` if it was abandoned (control channel not open yet, most
 * often at the very first call, made BEFORE any WebRTC connection). `dernier`
 * is only remembered on a successful send — without this guard, a lost first
 * send would mark the current state as already announced, and as long as
 * visibility did not change afterwards, it would NEVER be re-emitted:
 * the window would stay asleep on the agent side forever, without any `WARN`
 * to signal it.
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
