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
        /// `new Event(type)` ne porte pas de propriété `key` : Node n'a pas
        /// de classe `KeyboardEvent` globale (contrairement à un vrai
        /// navigateur), donc on la simule en l'assignant après coup sur un
        /// `Event` ordinaire — suffisant, `audio.ts` ne lit que `.type` et
        /// `.key`.
        declencherTouche(type: string, touche: string) {
            const evenement = new Event(type) as Event & { key: string };
            evenement.key = touche;
            for (const ecouteur of [...(ecouteurs.get(type) ?? [])]) {
                ecouteur(evenement);
            }
        },
        compte(type: string) {
            return (ecouteurs.get(type) ?? []).length;
        },
    };
}

/// Un média dont le démutage est systématiquement refusé par le navigateur :
/// `muted` reste bloqué à `true` quoi qu'on lui assigne.
function faireMediaRecalcitrant() {
    const media = {};
    Object.defineProperty(media, 'muted', {
        get: () => true,
        set: () => {},
    });
    return media as { muted: boolean };
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

    it('un appui sur Shift seul ne démute pas et ne consomme pas l’armement', () => {
        const media = { muted: true };
        const cible = faireCible();
        armerLeSon({ media, cible });

        cible.declencherTouche('keydown', 'Shift');
        expect(media.muted).toBe(true);
        // L'armement n'est pas consommé : les écouteurs sont toujours là.
        expect(cible.compte('keydown')).toBe(1);
        expect(cible.compte('pointerdown')).toBe(1);
    });

    it('Control, Alt, Meta et Escape ne valent pas non plus activation', () => {
        for (const touche of ['Control', 'Alt', 'Meta', 'Escape']) {
            const media = { muted: true };
            const cible = faireCible();
            armerLeSon({ media, cible });

            cible.declencherTouche('keydown', touche);
            expect(media.muted).toBe(true);
            expect(cible.compte('keydown')).toBe(1);
        }
    });

    it('une touche ordinaire qui suit un modificateur démute bien', () => {
        const media = { muted: true };
        const cible = faireCible();
        armerLeSon({ media, cible });

        cible.declencherTouche('keydown', 'Shift');
        expect(media.muted).toBe(true);

        cible.declencherTouche('keydown', ' ');
        expect(media.muted).toBe(false);
        expect(cible.compte('keydown')).toBe(0);
        expect(cible.compte('pointerdown')).toBe(0);
    });

    it('un média dont le démutage est refusé par le navigateur laisse les écouteurs en place', () => {
        const media = faireMediaRecalcitrant();
        const cible = faireCible();
        const surEtat = vi.fn();
        armerLeSon({ media, cible, surEtat });

        cible.declencher('pointerdown');

        expect(media.muted).toBe(true);
        expect(cible.compte('pointerdown')).toBe(1);
        expect(cible.compte('keydown')).toBe(1);
        // Ne signale pas un succès qui n'a pas eu lieu.
        expect(surEtat).not.toHaveBeenCalledWith(true);

        // Un geste ultérieur peut retenter — toujours refusé ici, mais la
        // séquence ne lève pas et les écouteurs restent disponibles.
        cible.declencher('keydown');
        expect(cible.compte('pointerdown')).toBe(1);
    });

    it('l’annulation après un démutage déjà survenu ne casse rien et laisse le son actif', () => {
        // Le geste a déjà tout retiré lui-même (voir le test précédent) :
        // `annuler()` doit rester un no-op silencieux, pas remuter ni lever.
        const media = { muted: true };
        const cible = faireCible();
        const annuler = armerLeSon({ media, cible });
        cible.declencher('pointerdown');
        expect(media.muted).toBe(false);

        expect(() => annuler()).not.toThrow();
        expect(media.muted).toBe(false);
    });
});
