// Tests du plein écran et du Keyboard Lock.
//
// Comme pour pointer.ts, le module est testé par injection : ni `document`,
// ni `navigator`, ni un vrai HTMLElement ne sont nécessaires.

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
        // Sans argument : c'est le mode jeu, toutes les touches vont au jeu,
        // et l'utilisateur sort par appui long sur Échap.
        expect(lock).toHaveBeenCalledWith();
    });

    it("does nothing and does not throw when the API is absent", async () => {
        // Firefox et Safari : limite documentée, non corrigée.
        await expect(verrouillerClavier({})).resolves.toBeUndefined();
    });

    it('swallows a lock rejection without propagating it', async () => {
        const lock = vi.fn().mockRejectedValue(new Error('refused'));
        await expect(
            verrouillerClavier({ keyboard: { lock, unlock: vi.fn() } }),
        ).resolves.toBeUndefined();
    });
});

/** Double de test pour le bouton de bascule : un HTMLElement minimal. */
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

/** Double de test pour la cible du plein écran : compte les demandes reçues. */
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
 * Double de test pour le document : garde l'élément actuellement en plein
 * écran, comme le ferait `document.fullscreenElement` réel, et permet de
 * simuler `fullscreenchange`.
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

        // Entrée : verrouille.
        doc.fullscreenElement = cible;
        doc.declencher('fullscreenchange');
        await Promise.resolve();
        expect(unlock).not.toHaveBeenCalled();

        // Sortie : libère.
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

        // Ce test échoue si onChange compare autre chose que `fullscreenElement
        // === cible`, par exemple s'il se contente de vérifier que
        // fullscreenElement est non nul (un AUTRE élément en plein écran
        // marquerait alors ce bouton actif à tort).
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

        // Les événements ultérieurs ne doivent plus rien déclencher.
        bouton.declencher('click');
        expect(cible.demandes).toBe(0);
    });
});

/** Double de test pour une cible d'événements. */
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
        // `requestFullscreen()` exige une activation transitoire : appeler
        // depuis le message serait rejeté par le navigateur.
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
        // Un joueur à la manette ou au clavier n'a aucune raison de cliquer.
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
        // Fin de session avant tout geste : rien ne doit survivre.
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
