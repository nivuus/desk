// Souris relative sous Pointer Lock.
//
// L'agent est SEUL décideur du mode : ce module obéit au message `pointer`
// qu'il reçoit, il ne décide de rien. Il ne fait que deux choses non
// triviales — armer le verrouillage sur le prochain clic, faute d'activation
// utilisateur transitoire sur un message reçu par data channel, et sommer les
// deltas sans jamais en perdre.
//
// Les dépendances sont INJECTÉES plutôt que lues dans les objets globaux, ce
// qui rend le module testable sans DOM.

import { encodeMouseMoveRelative } from '../../proto/ts/input';
import type { CursorShape } from '../../proto/ts/control';

const MIN_I16 = -32768;
const MAX_I16 = 32767;

/** Ce dont ce module a besoin d'un élément vidéo. */
export interface CibleVideo {
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
    requestPointerLock(): void;
    style: { cursor: string };
}

/** Ce dont ce module a besoin d'un objet document. */
export interface CibleDocument {
    pointerLockElement: CibleVideo | null;
    exitPointerLock(): void;
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

/** Somme des déplacements d'une rafale d'événements coalescés. */
export function sommerDeltas(
    evenements: ReadonlyArray<{ movementX: number; movementY: number }>,
): { dx: number; dy: number } {
    let dx = 0;
    let dy = 0;
    for (const e of evenements) {
        dx += e.movementX;
        dy += e.movementY;
    }
    return { dx, dy };
}

/**
 * Borne les deltas à la plage d'un i16 et REPORTE le reste sur l'appel
 * suivant : la somme finalement transmise reste exacte. Un mouvement de plus
 * de 32767 px en un seul événement ne se produit pas en pratique — mais s'il
 * se produisait, écrêter en silence ferait dériver la visée sans que rien ne
 * le signale.
 */
export function creerClampReport(): (dx: number, dy: number) => { dx: number; dy: number } {
    let restX = 0;
    let restY = 0;
    return (dx, dy) => {
        const totalX = dx + restX;
        const totalY = dy + restY;
        const sortieX = Math.max(MIN_I16, Math.min(MAX_I16, totalX));
        const sortieY = Math.max(MIN_I16, Math.min(MAX_I16, totalY));
        restX = totalX - sortieX;
        restY = totalY - sortieY;
        return { dx: sortieX, dy: sortieY };
    };
}

export interface PointerOptions {
    video: CibleVideo;
    doc: CibleDocument;
    envoyer: (payload: Uint8Array) => void;
    /** Prévient l'appelant qu'un verrouillage a échoué deux fois de suite. */
    surEchec?: () => void;
}

export interface PointerHandle {
    /** À appeler à réception d'un message de contrôle `pointer`. */
    surMessagePointeur(visible: boolean, shape: CursorShape): void;
    detacher(): void;
}

export function attachPointer(options: PointerOptions): PointerHandle {
    const { video, doc, envoyer, surEchec } = options;

    const clamp = creerClampReport();
    let arme = false;
    let echecs = 0;

    const verrouiller = (): void => {
        // `requestPointerLock` exige une activation utilisateur transitoire :
        // un message reçu sur data channel n'en est pas une. D'où l'armement.
        video.requestPointerLock();
    };

    const onClick = (): void => {
        if (arme && doc.pointerLockElement !== video) verrouiller();
    };

    const onPointerLockError = (): void => {
        echecs += 1;
        if (echecs >= 2) surEchec?.();
    };

    const onPointerLockChange = (): void => {
        if (doc.pointerLockElement === video) echecs = 0;
        // Sortie par Échap alors que l'agent est toujours en relatif : on
        // reste armé, le prochain clic reverrouille.
    };

    const onPointerMove = (event: PointerEvent): void => {
        if (doc.pointerLockElement !== video) return;
        const coalesces = event.getCoalescedEvents?.() ?? [];
        const brut = coalesces.length > 0 ? sommerDeltas(coalesces) : sommerDeltas([event]);
        const { dx, dy } = clamp(brut.dx, brut.dy);
        if (dx !== 0 || dy !== 0) envoyer(encodeMouseMoveRelative(dx, dy));
    };

    video.addEventListener('click', onClick);
    video.addEventListener('pointermove', onPointerMove);
    doc.addEventListener('pointerlockerror', onPointerLockError);
    doc.addEventListener('pointerlockchange', onPointerLockChange);

    return {
        surMessagePointeur(visible, shape) {
            video.style.cursor = visible ? shape : 'none';
            if (visible) {
                arme = false;
                if (doc.pointerLockElement === video) doc.exitPointerLock();
            } else {
                arme = true;
                if (doc.pointerLockElement !== video) verrouiller();
            }
        },
        detacher() {
            video.removeEventListener('click', onClick);
            video.removeEventListener('pointermove', onPointerMove);
            doc.removeEventListener('pointerlockerror', onPointerLockError);
            doc.removeEventListener('pointerlockchange', onPointerLockChange);
        },
    };
}

// Valeurs par défaut pour utilisation dans le navigateur réel (voir tâche 15 : câblage).
export function attachPointerAuDOM(options: Omit<PointerOptions, 'video' | 'doc'>): PointerHandle {
    return attachPointer({
        video: document.querySelector('video') as CibleVideo,
        doc: document as unknown as CibleDocument,
        ...options,
    });
}
