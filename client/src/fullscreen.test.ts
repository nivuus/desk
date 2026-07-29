// Tests du plein écran et du Keyboard Lock.
//
// Comme pour pointer.ts, le module est testé par injection : ni `document`,
// ni `navigator`, ni un vrai HTMLElement ne sont nécessaires.

import { describe, expect, it, vi } from 'vitest';
import {
    attachFullscreen,
    verrouillerClavier,
    type BoutonPleinEcran,
    type CibleEcran,
    type DocumentPleinEcran,
} from './fullscreen';

describe('verrouillage du clavier', () => {
    it("appelle lock quand l'API est disponible", async () => {
        const lock = vi.fn().mockResolvedValue(undefined);
        await verrouillerClavier({ keyboard: { lock, unlock: vi.fn() } });
        expect(lock).toHaveBeenCalledOnce();
        // Sans argument : c'est le mode jeu, toutes les touches vont au jeu,
        // et l'utilisateur sort par appui long sur Échap.
        expect(lock).toHaveBeenCalledWith();
    });

    it("ne fait rien et ne lève pas quand l'API est absente", async () => {
        // Firefox et Safari : limite documentée, non corrigée.
        await expect(verrouillerClavier({})).resolves.toBeUndefined();
    });

    it('avale un rejet de lock sans le propager', async () => {
        const lock = vi.fn().mockRejectedValue(new Error('refusé'));
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
            const liste = ecouteurs.get(type) ?? [];
            liste.push(ecouteur);
            ecouteurs.set(type, liste);
        },
        removeEventListener(type: string, ecouteur: EventListener) {
            const liste = (ecouteurs.get(type) ?? []).filter((e) => e !== ecouteur);
            ecouteurs.set(type, liste);
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
            const liste = ecouteurs.get(type) ?? [];
            liste.push(ecouteur);
            ecouteurs.set(type, liste);
        },
        removeEventListener(type: string, ecouteur: EventListener) {
            const liste = (ecouteurs.get(type) ?? []).filter((e) => e !== ecouteur);
            ecouteurs.set(type, liste);
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
    it("un clic sur le bouton demande le plein écran quand on n'y est pas déjà", () => {
        const bouton = faireBouton();
        const cible = faireCible();
        const doc = faireDocument();
        attachFullscreen({ bouton, cible, doc });

        bouton.declencher('click');

        expect(cible.demandes).toBe(1);
        expect(doc.sorties).toBe(0);
    });

    it("un clic sur le bouton quitte le plein écran quand on y est déjà", () => {
        const bouton = faireBouton();
        const cible = faireCible();
        const doc = faireDocument();
        doc.fullscreenElement = cible;
        attachFullscreen({ bouton, cible, doc });

        bouton.declencher('click');

        expect(doc.sorties).toBe(1);
        expect(cible.demandes).toBe(0);
    });

    it("l'entrée en plein écran déclenche le verrouillage clavier", async () => {
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

    it('la sortie du plein écran libère le clavier', async () => {
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

    it('le bouton bascule data-actif dans les deux sens', () => {
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

    it("le bouton ne se marque pas actif quand un AUTRE élément passe en plein écran", () => {
        const bouton = faireBouton();
        const cible = faireCible();
        const autreCible = faireCible();
        const doc = faireDocument();
        attachFullscreen({ bouton, cible, doc });

        doc.fullscreenElement = autreCible;
        doc.declencher('fullscreenchange');

        expect(bouton.dataset.actif).toBe('false');
    });

    it('detacher retire tous les écouteurs posés', () => {
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
