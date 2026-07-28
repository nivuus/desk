// Tests du démutage au premier geste.
//
// Le module est testé par injection : ni `document`, ni `window`, ni un vrai
// HTMLVideoElement ne sont nécessaires. C'est la technique retenue pour
// `reset-origin.js` dans le spike multi-fenêtres, qui a permis de tester le
// même module sous Vitest et dans le navigateur sans build.

import { describe, expect, it, vi } from 'vitest';

import { armerLeSon } from './audio';

function faireCible() {
    const ecouteurs = new Map<string, EventListener[]>();
    return {
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
    };
}

describe('armerLeSon', () => {
    it('laisse le média muet tant qu’aucun geste n’est venu', () => {
        const media = { muted: true };
        const cible = faireCible();
        armerLeSon({ media, cible });
        expect(media.muted).toBe(true);
    });

    it('démute au premier clic', () => {
        const media = { muted: true };
        const cible = faireCible();
        armerLeSon({ media, cible });
        cible.declencher('pointerdown');
        expect(media.muted).toBe(false);
    });

    it('démute aussi sur une touche du clavier', () => {
        const media = { muted: true };
        const cible = faireCible();
        armerLeSon({ media, cible });
        cible.declencher('keydown');
        expect(media.muted).toBe(false);
    });

    it('retire tous ses écouteurs après le premier geste', () => {
        // Sans retrait, chaque geste ultérieur reforcerait `muted = false` et
        // écraserait un éventuel choix de l'utilisateur de couper le son.
        const media = { muted: true };
        const cible = faireCible();
        armerLeSon({ media, cible });
        cible.declencher('pointerdown');
        expect(cible.compte('pointerdown')).toBe(0);
        expect(cible.compte('keydown')).toBe(0);

        media.muted = true;
        cible.declencher('pointerdown');
        expect(media.muted).toBe(true);
    });

    it('signale le changement d’état une seule fois', () => {
        const media = { muted: true };
        const cible = faireCible();
        const surEtat = vi.fn();
        armerLeSon({ media, cible, surEtat });

        expect(surEtat).toHaveBeenCalledWith(false);
        cible.declencher('pointerdown');
        expect(surEtat).toHaveBeenCalledWith(true);
        expect(surEtat).toHaveBeenCalledTimes(2);
    });

    it('l’annulation retire les écouteurs sans démuter', () => {
        const media = { muted: true };
        const cible = faireCible();
        const annuler = armerLeSon({ media, cible });
        annuler();
        cible.declencher('pointerdown');
        expect(media.muted).toBe(true);
        expect(cible.compte('pointerdown')).toBe(0);
    });
});
