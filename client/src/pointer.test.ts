// Tests of the relative mouse under Pointer Lock.
//
// The module is tested by injection: neither `document`, nor `window`, nor a real
// HTMLVideoElement are needed. It is the technique chosen for the
// project's arming modules.

import { describe, expect, it, vi } from 'vitest';
import { attachPointer, createClampReport, sommerDeltas, type CibleVideo } from './pointer';

function faireCibleVideo() {
    const ecouteurs = new Map<string, EventListener[]>();
    let pointerLock = false;
    let permisVerrouiller = false;
    return {
        addEventListener(type: string, ecouteur: EventListener) {
            const list = ecouteurs.get(type) ?? [];
            list.push(ecouteur);
            ecouteurs.set(type, list);
        },
        removeEventListener(type: string, ecouteur: EventListener) {
            const list = (ecouteurs.get(type) ?? []).filter((e) => e !== ecouteur);
            ecouteurs.set(type, list);
        },
        requestPointerLock() {
            // Simulates the real behaviour: the browser only accepts if a
            // transient user activation is in progress. Without activation (default
            // test), the call fails silently.
            if (permisVerrouiller) {
                pointerLock = true;
            }
        },
        style: { cursor: 'auto' },
        declencher(type: string) {
            for (const ecouteur of [...(ecouteurs.get(type) ?? [])]) {
                ecouteur(new Event(type));
            }
        },
        declencherPointerMove(movementX: number, movementY: number, coalesces?: Array<{ movementX: number; movementY: number }>) {
            // Simulates a PointerEvent without using the class (which does not exist in Node)
            const event = new Event('pointermove') as any;
            event.movementX = movementX;
            event.movementY = movementY;
            if (coalesces) {
                event.getCoalescedEvents = () => coalesces;
            }
            for (const ecouteur of [...(ecouteurs.get('pointermove') ?? [])]) {
                ecouteur(event as Event);
            }
        },
        compte(type: string) {
            return (ecouteurs.get(type) ?? []).length;
        },
        estVerrouille() {
            return pointerLock;
        },
        deverrouiller() {
            pointerLock = false;
        },
        autoriserVerrouillage() {
            permisVerrouiller = true;
        },
        interdireVerrouillage() {
            permisVerrouiller = false;
        },
    };
}

function faireCibleDocument() {
    const ecouteurs = new Map<string, EventListener[]>();
    let verrouille: CibleVideo | null = null;
    return {
        get pointerLockElement() {
            return verrouille;
        },
        set pointerLockElement(video: CibleVideo | null) {
            verrouille = video;
        },
        exitPointerLock() {
            verrouille = null;
        },
        addEventListener(type: string, ecouteur: EventListener) {
            const list = ecouteurs.get(type) ?? [];
            list.push(ecouteur);
            ecouteurs.set(type, list);
        },
        removeEventListener(type: string, ecouteur: EventListener) {
            const list = (ecouteurs.get(type) ?? []).filter((e) => e !== ecouteur);
            ecouteurs.set(type, list);
        },
        declencher(type: string) {
            for (const ecouteur of [...(ecouteurs.get(type) ?? [])]) {
                ecouteur(new Event(type));
            }
        },
        compte(type: string) {
            return (ecouteurs.get(type) ?? []).length;
        },
    };
}

describe('summing the coalesced deltas', () => {
    it('sums the intermediate samples', () => {
        expect(sommerDeltas([
            { movementX: 3, movementY: -1 },
            { movementX: 4, movementY: -2 },
        ])).toEqual({ dx: 7, dy: -3 });
    });

    it('returns zero on an empty list', () => {
        expect(sommerDeltas([])).toEqual({ dx: 0, dy: 0 });
    });
});

describe('clamp with carry-over', () => {
    it('lets the values within range through', () => {
        const clamp = createClampReport();
        expect(clamp(10, -10)).toEqual({ dx: 10, dy: -10 });
    });

    it('clamps the overflow and carries it over to the next call', () => {
        // The transmitted sum must stay exact: it is what determines the
        // aim. Clipping while losing the remainder would make the shot drift.
        const clamp = createClampReport();
        expect(clamp(40000, 0)).toEqual({ dx: 32767, dy: 0 });
        expect(clamp(0, 0)).toEqual({ dx: 7233, dy: 0 });
        expect(clamp(0, 0)).toEqual({ dx: 0, dy: 0 });
    });

    it('also carries the negative overflows over', () => {
        const clamp = createClampReport();
        expect(clamp(0, -40000)).toEqual({ dx: 0, dy: -32768 });
        expect(clamp(0, 0)).toEqual({ dx: 0, dy: -7232 });
    });

    it('carries nothing over when nothing overflows', () => {
        const clamp = createClampReport();
        clamp(5, 5);
        expect(clamp(0, 0)).toEqual({ dx: 0, dy: 0 });
    });
});

describe('attachPointer', () => {
    it('stays disarmed as long as no pointer message has been received', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        attachPointer({ video: video as any, doc: doc as any, envoyer });

        // A click without arming must not lock (even if allowed)
        video.autoriserVerrouillage();
        video.declencher('click');
        expect(video.estVerrouille()).toBe(false);
    });

    it('arms the lock on a pointer visible=false message, and the click locks', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer });

        // First, the browser refuses the lock (no user activation)
        video.interdireVerrouillage();
        handle.surMessagePointeur(false, 'default');
        // The free attempt tries to lock but fails silently
        expect(video.estVerrouille()).toBe(false);

        // Now, the browser accepts the lock (user activation
        // has just arrived through the click)
        video.autoriserVerrouillage();
        video.declencher('click');
        expect(video.estVerrouille()).toBe(true);

        // This test fails if onClick is empty or if the condition on `arme` is broken.
    });

    it('stays armed after an exit through Escape if the agent is still relative', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer });

        // Arming: allowed to lock from the start
        video.autoriserVerrouillage();
        handle.surMessagePointeur(false, 'default');
        // The free attempt succeeds
        expect(video.estVerrouille()).toBe(true);

        // Simulate that it is really locked in the document
        doc.pointerLockElement = video as any;
        doc.declencher('pointerlockchange');

        // Exit through Escape: the document changes state
        doc.pointerLockElement = null;
        // But the test must also reflect that the browser released the lock
        video.deverrouiller();
        doc.declencher('pointerlockchange');

        // The module must stay armed after pointerlockchange if still in relative mode
        // So the next click must lock again
        video.declencher('click');
        expect(video.estVerrouille()).toBe(true);

        // This test fails if onPointerLockChange disarmed or if onClick was empty.
    });

    it('removes all the listeners on detach', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer });

        // Before detaching: 2 listeners on video
        expect(video.compte('click')).toBe(1);
        expect(video.compte('pointermove')).toBe(1);
        expect(doc.compte('pointerlockerror')).toBe(1);
        expect(doc.compte('pointerlockchange')).toBe(1);

        // Detaching
        handle.detacher();

        // After detaching: no more listeners
        expect(video.compte('click')).toBe(0);
        expect(video.compte('pointermove')).toBe(0);
        expect(doc.compte('pointerlockerror')).toBe(0);
        expect(doc.compte('pointerlockchange')).toBe(0);

        // Later events must do nothing
        video.declencher('click');
        expect(video.estVerrouille()).toBe(false);
    });

    it('calls surEchec after two consecutive lock errors', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const surEchec = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer, surEchec });

        handle.surMessagePointeur(false, 'default');

        // First error
        doc.declencher('pointerlockerror');
        expect(surEchec).not.toHaveBeenCalled();

        // Second consecutive error
        doc.declencher('pointerlockerror');
        expect(surEchec).toHaveBeenCalledTimes(1);

        // A successful lock resets the counter to zero
        doc.pointerLockElement = video as any;
        doc.declencher('pointerlockchange');

        // First error after reset
        doc.declencher('pointerlockerror');
        expect(surEchec).toHaveBeenCalledTimes(1);

        // Second error: surEchec is called again
        doc.declencher('pointerlockerror');
        expect(surEchec).toHaveBeenCalledTimes(2);
    });

    it('falls back to the main event when getCoalescedEvents returns an empty array', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer });

        // Arming and locking
        handle.surMessagePointeur(false, 'default');
        video.declencher('click');
        doc.pointerLockElement = video as any;

        // An event with an empty getCoalescedEvents
        video.declencherPointerMove(10, 20, []);

        // The main event (10, 20) must be used, not a null sum
        expect(envoyer).toHaveBeenCalled();
        // Check that something was sent (the exact payload depends on encodeMouseMoveRelative)
        expect(envoyer.mock.calls[0][0]).toBeInstanceOf(Uint8Array);
    });
});
