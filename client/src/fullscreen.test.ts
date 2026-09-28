// Tests of fullscreen and of Keyboard Lock.
//
// As for pointer.ts, the module is tested by injection: neither `document`,
// nor `navigator`, nor a real HTMLElement are needed.

import { describe, expect, it, vi } from 'vitest';
import {
    armerPleinEcran,
    attachFullscreen,
    verrouillerClavier,
    type BoutonPleinEcran,
    type CibleEcran,
    type DocumentPleinEcran,
} from './fullscreen';

describe('keyboard lock', () => {
    it("calls lock when the API is available", async () => {
        const lock = vi.fn().mockResolvedValue(undefined);
        await verrouillerClavier({ keyboard: { lock, unlock: vi.fn() } });
        expect(lock).toHaveBeenCalledOnce();
        // Without an argument: it is game mode, all keys go to the game,
        // and the user exits with a long press on Escape.
        expect(lock).toHaveBeenCalledWith();
    });

    it("does nothing and does not throw when the API is absent", async () => {
        // Firefox and Safari: documented limitation, not fixed.
        await expect(verrouillerClavier({})).resolves.toBeUndefined();
    });

    it('swallows a lock rejection without propagating it', async () => {
        const lock = vi.fn().mockRejectedValue(new Error('refused'));
        await expect(
            verrouillerClavier({ keyboard: { lock, unlock: vi.fn() } }),
        ).resolves.toBeUndefined();
    });
});

/** Test double for the toggle button: a minimal HTMLElement. */
function faireBouton() {
    const ecouteurs = new Map<string, EventListener[]>();
    return {
        dataset: {} as { actif?: string },
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
    } satisfies BoutonPleinEcran & {
        declencher: (type: string) => void;
        compte: (type: string) => number;
    };
}

/** Test double for the fullscreen target: counts the requests received. */
function faireCible(): CibleEcran & { demandes: number } {
    return {
        demandes: 0,
        requestFullscreen(this: { demandes: number }) {
            this.demandes += 1;
            return Promise.resolve();
        },
    };
}

/**
 * Test double for the document: keeps the element currently in
 * fullscreen, as the real `document.fullscreenElement` would, and makes it possible to
 * simulate `fullscreenchange`.
 */
function faireDocument() {
    const ecouteurs = new Map<string, EventListener[]>();
    let element: CibleEcran | null = null;
    let sorties = 0;
    return {
        get fullscreenElement() {
            return element;
        },
        set fullscreenElement(v: CibleEcran | null) {
            element = v;
        },
        exitFullscreen() {
            sorties += 1;
            element = null;
            return Promise.resolve();
        },
        get sorties() {
            return sorties;
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
    } satisfies DocumentPleinEcran & {
        sorties: number;
        declencher: (type: string) => void;
        compte: (type: string) => number;
    };
}

describe('attachFullscreen', () => {
    it("a click on the button requests fullscreen when not already in it", () => {
        const bouton = faireBouton();
        const cible = faireCible();
        const doc = faireDocument();
        attachFullscreen({ bouton, cible, doc });

        bouton.declencher('click');

        expect(cible.demandes).toBe(1);
        expect(doc.sorties).toBe(0);
    });

    it("a click on the button leaves fullscreen when already in it", () => {
        const bouton = faireBouton();
        const cible = faireCible();
        const doc = faireDocument();
        doc.fullscreenElement = cible;
        attachFullscreen({ bouton, cible, doc });

        bouton.declencher('click');

        expect(doc.sorties).toBe(1);
        expect(cible.demandes).toBe(0);
    });

    it("entering fullscreen triggers the keyboard lock", async () => {
        const bouton = faireBouton();
        const cible = faireCible();
        const doc = faireDocument();
        const lock = vi.fn().mockResolvedValue(undefined);
        attachFullscreen({ bouton, cible, doc, navigateur: { keyboard: { lock, unlock: vi.fn() } } });

        doc.fullscreenElement = cible;
        doc.declencher('fullscreenchange');
        await Promise.resolve();

        expect(lock).toHaveBeenCalledOnce();
        expect(lock).toHaveBeenCalledWith();
    });

    it('leaving fullscreen releases the keyboard', async () => {
        const bouton = faireBouton();
        const cible = faireCible();
        const doc = faireDocument();
        const unlock = vi.fn();
        const lock = vi.fn().mockResolvedValue(undefined);
        attachFullscreen({ bouton, cible, doc, navigateur: { keyboard: { lock, unlock } } });

        // Entering: locks.
        doc.fullscreenElement = cible;
        doc.declencher('fullscreenchange');
        await Promise.resolve();
        expect(unlock).not.toHaveBeenCalled();

        // Leaving: releases.
        doc.fullscreenElement = null;
        doc.declencher('fullscreenchange');

        expect(unlock).toHaveBeenCalledOnce();
    });

    it('the button toggles data-actif both ways', () => {
        const bouton = faireBouton();
        const cible = faireCible();
        const doc = faireDocument();
        attachFullscreen({ bouton, cible, doc });

        doc.fullscreenElement = cible;
        doc.declencher('fullscreenchange');
        expect(bouton.dataset.actif).toBe('true');

        doc.fullscreenElement = null;
        doc.declencher('fullscreenchange');
        expect(bouton.dataset.actif).toBe('false');

        // This test fails if onChange compares anything other than `fullscreenElement
        // === cible`, for example if it merely checks that
        // fullscreenElement is not null (ANOTHER element in fullscreen
        // would then wrongly mark this button active).
    });

    it("the button does not mark itself active when ANOTHER element goes fullscreen", () => {
        const bouton = faireBouton();
        const cible = faireCible();
        const autreCible = faireCible();
        const doc = faireDocument();
        attachFullscreen({ bouton, cible, doc });

        doc.fullscreenElement = autreCible;
        doc.declencher('fullscreenchange');

        expect(bouton.dataset.actif).toBe('false');
    });

    it('detacher removes all the listeners it set', () => {
        const bouton = faireBouton();
        const cible = faireCible();
        const doc = faireDocument();
        const detacher = attachFullscreen({ bouton, cible, doc });

        expect(bouton.compte('click')).toBe(1);
        expect(doc.compte('fullscreenchange')).toBe(1);

        detacher();

        expect(bouton.compte('click')).toBe(0);
        expect(doc.compte('fullscreenchange')).toBe(0);

        // Later events must no longer trigger anything.
        bouton.declencher('click');
        expect(cible.demandes).toBe(0);
    });
});

/** Test double for an event target. */
function faireEcouteurs() {
    const ecouteurs = new Map<string, EventListener[]>();
    return {
        addEventListener(type: string, e: EventListener) {
            const l = ecouteurs.get(type) ?? [];
            l.push(e);
            ecouteurs.set(type, l);
        },
        removeEventListener(type: string, e: EventListener) {
            ecouteurs.set(type, (ecouteurs.get(type) ?? []).filter((x) => x !== e));
        },
        declencher(type: string) {
            for (const e of ecouteurs.get(type) ?? []) e(new Event(type));
        },
        compte(type: string) {
            return (ecouteurs.get(type) ?? []).length;
        },
    };
}

describe('arming fullscreen', () => {
    it("does not enter fullscreen before a user gesture", () => {
        // `requestFullscreen()` requires transient activation: calling
        // from the message would be rejected by the browser.
        const requestFullscreen = vi.fn().mockResolvedValue(undefined);
        const cible = { requestFullscreen };
        const doc = { fullscreenElement: null, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        armerPleinEcran({ cible, doc, ecouteurs });
        expect(requestFullscreen).not.toHaveBeenCalled();
    });

    it('enters fullscreen on the first pointerdown', () => {
        const requestFullscreen = vi.fn().mockResolvedValue(undefined);
        const cible = { requestFullscreen };
        const doc = { fullscreenElement: null, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        armerPleinEcran({ cible, doc, ecouteurs });
        ecouteurs.declencher('pointerdown');
        expect(requestFullscreen).toHaveBeenCalledOnce();
    });

    it('enters fullscreen on the first keydown, without a click', () => {
        // A player on a gamepad or a keyboard has no reason to click.
        const requestFullscreen = vi.fn().mockResolvedValue(undefined);
        const cible = { requestFullscreen };
        const doc = { fullscreenElement: null, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        armerPleinEcran({ cible, doc, ecouteurs });
        ecouteurs.declencher('keydown');
        expect(requestFullscreen).toHaveBeenCalledOnce();
    });

    it("enters only once, and removes its listeners afterwards", () => {
        const requestFullscreen = vi.fn().mockResolvedValue(undefined);
        const cible = { requestFullscreen };
        const doc = { fullscreenElement: null, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        armerPleinEcran({ cible, doc, ecouteurs });
        ecouteurs.declencher('pointerdown');
        ecouteurs.declencher('pointerdown');
        expect(requestFullscreen).toHaveBeenCalledOnce();
        expect(ecouteurs.compte('pointerdown')).toBe(0);
        expect(ecouteurs.compte('keydown')).toBe(0);
    });

    it("arms nothing if the page is already fullscreen", () => {
        const cible = { requestFullscreen: vi.fn().mockResolvedValue(undefined) };
        const doc = { fullscreenElement: cible, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        armerPleinEcran({ cible, doc, ecouteurs });
        expect(ecouteurs.compte('pointerdown')).toBe(0);
    });

    it('the detach function removes the listeners without having armed', () => {
        // Session end before any gesture: nothing must survive.
        const cible = { requestFullscreen: vi.fn().mockResolvedValue(undefined) };
        const doc = { fullscreenElement: null, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        const detacher = armerPleinEcran({ cible, doc, ecouteurs });
        detacher();
        expect(ecouteurs.compte('pointerdown')).toBe(0);
        ecouteurs.declencher('pointerdown');
        expect(cible.requestFullscreen).not.toHaveBeenCalled();
    });
});
