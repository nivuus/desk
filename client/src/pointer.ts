// Relative mouse under Pointer Lock.
//
// The agent is the SOLE decider of the mode: this module obeys the `pointer` message
// it receives, it decides nothing. It only does two non-trivial
// things — arm the lock on the next click, for lack of a transient user
// activation on a message received over a data channel, and sum the
// deltas without ever losing any.
//
// Dependencies are INJECTED rather than read from the global objects, which
// makes the module testable without a DOM.

import { encodeMouseMoveRelative } from '../../proto/ts/input';
import type { CursorShape } from '../../proto/ts/control';

const MIN_I16 = -32768;
const MAX_I16 = 32767;

/** What this module needs from a video element. */
export interface CibleVideo {
    addEventListener(type: string, ecouteur: EventListener | ((event: PointerEvent) => void)): void;
    removeEventListener(type: string, ecouteur: EventListener | ((event: PointerEvent) => void)): void;
    // Since Chrome 111, `requestPointerLock()` returns a Promise. The
    // typing declares it `void` on purpose: `verrouiller()` below swallows
    // the rejection itself (see its comment), so that this module stays
    // usable even on an older browser that really returns
    // `void`.
    requestPointerLock(): void | Promise<void>;
    style: { cursor: string };
}

/** What this module needs from a document object. */
export interface CibleDocument {
    pointerLockElement: CibleVideo | null;
    exitPointerLock(): void;
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

/** Sum of the moves of a burst of coalesced events. */
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
 * Bounds the deltas to the range of an i16 and CARRIES the rest over to the next
 * call: the sum finally transmitted stays exact. A move of more
 * than 32767 px in a single event does not happen in practice — but if it
 * did, silently clipping would make aiming drift without anything
 * signalling it.
 */
export function createClampReport(): (dx: number, dy: number) => { dx: number; dy: number } {
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
    /** Warns the caller that a lock failed twice in a row. */
    surEchec?: () => void;
}

export interface PointerHandle {
    /** To call on receiving a `pointer` control message. */
    surMessagePointeur(visible: boolean, shape: CursorShape): void;
    detacher(): void;
}

export function attachPointer(options: PointerOptions): PointerHandle {
    const { video, doc, envoyer, surEchec } = options;

    const clamp = createClampReport();
    let arme = false;
    let echecs = 0;

    const verrouiller = (): void => {
        // `requestPointerLock` requires a transient user activation:
        // a message received over a data channel is not one. Hence the arming.
        //
        // Since Chrome 111, the call returns a Promise that rejects
        // precisely in this case — the failure is EXPECTED, `onPointerLockError`
        // and rearming already take over. Without this `catch`, every
        // switch to relative mode would produce an unhandled rejection in the
        // console, even though the mechanism works as intended.
        // `Promise.resolve` wraps a `void` as well as a real Promise:
        // the `catch` stays without effect on a browser that returns nothing.
        void Promise.resolve(video.requestPointerLock()).catch(() => {});
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
        // Exit through Escape while the agent is still relative: we
        // stay armed, the next click locks again.
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

// Default values for use in the real browser (see task 15: wiring).
export function attachPointerAuDOM(options: Omit<PointerOptions, 'video' | 'doc'>): PointerHandle {
    return attachPointer({
        video: document.querySelector('video') as CibleVideo,
        doc: document as unknown as CibleDocument,
        ...options,
    });
}
