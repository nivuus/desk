// Tests of unmuting on the first gesture.
//
// The module is tested by injection: neither `document`, nor `window`, nor a real
// HTMLVideoElement are needed. It is the technique chosen for
// `reset-origin.js` in the multi-window spike, which made it possible to test the
// same module under Vitest and in the browser without a build.

import { describe, expect, it, vi } from 'vitest';

import { armerLeSon } from './audio';

function faireCible() {
    const ecouteurs = new Map<string, EventListener[]>();
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
        declencher(type: string) {
            for (const ecouteur of [...(ecouteurs.get(type) ?? [])]) {
                ecouteur(new Event(type));
            }
        },
        /// `new Event(type)` carries no `key` property: Node has no
        /// global `KeyboardEvent` class (unlike a real
        /// browser), so it is simulated by assigning it afterwards on an
        /// ordinary `Event` — enough, `audio.ts` only reads `.type` and
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

/// A media whose unmuting is systematically refused by the browser:
/// `muted` stays stuck at `true` whatever is assigned to it.
function faireMediaRecalcitrant() {
    const media = {};
    Object.defineProperty(media, 'muted', {
        get: () => true,
        set: () => {},
    });
    return media as { muted: boolean };
}

describe('armerLeSon', () => {
    it('leaves the media muted as long as no gesture has come', () => {
        const media = { muted: true };
        const cible = faireCible();
        armerLeSon({ media, cible });
        expect(media.muted).toBe(true);
    });

    it('unmutes on the first click', () => {
        const media = { muted: true };
        const cible = faireCible();
        armerLeSon({ media, cible });
        cible.declencher('pointerdown');
        expect(media.muted).toBe(false);
    });

    it('also unmutes on a keyboard key', () => {
        const media = { muted: true };
        const cible = faireCible();
        armerLeSon({ media, cible });
        cible.declencher('keydown');
        expect(media.muted).toBe(false);
    });

    it('removes all its listeners after the first gesture', () => {
        // Without removal, every later gesture would force `muted = false` again and
        // would overwrite a possible user choice to turn the sound off.
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

    it('reports the state change only once', () => {
        const media = { muted: true };
        const cible = faireCible();
        const surEtat = vi.fn();
        armerLeSon({ media, cible, surEtat });

        expect(surEtat).toHaveBeenCalledWith(false);
        cible.declencher('pointerdown');
        expect(surEtat).toHaveBeenCalledWith(true);
        expect(surEtat).toHaveBeenCalledTimes(2);
    });

    it('cancelling removes the listeners without unmuting', () => {
        const media = { muted: true };
        const cible = faireCible();
        const annuler = armerLeSon({ media, cible });
        annuler();
        cible.declencher('pointerdown');
        expect(media.muted).toBe(true);
        expect(cible.compte('pointerdown')).toBe(0);
    });

    it('pressing Shift alone does not unmute and does not consume the arming', () => {
        const media = { muted: true };
        const cible = faireCible();
        armerLeSon({ media, cible });

        cible.declencherTouche('keydown', 'Shift');
        expect(media.muted).toBe(true);
        // The arming is not consumed: the listeners are still there.
        expect(cible.compte('keydown')).toBe(1);
        expect(cible.compte('pointerdown')).toBe(1);
    });

    it('Control, Alt, Meta and Escape do not count as activation either', () => {
        for (const touche of ['Control', 'Alt', 'Meta', 'Escape']) {
            const media = { muted: true };
            const cible = faireCible();
            armerLeSon({ media, cible });

            cible.declencherTouche('keydown', touche);
            expect(media.muted).toBe(true);
            expect(cible.compte('keydown')).toBe(1);
        }
    });

    it('an ordinary key following a modifier does unmute', () => {
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

    it('a media whose unmute is refused by the browser leaves the listeners in place', () => {
        const media = faireMediaRecalcitrant();
        const cible = faireCible();
        const surEtat = vi.fn();
        armerLeSon({ media, cible, surEtat });

        cible.declencher('pointerdown');

        expect(media.muted).toBe(true);
        expect(cible.compte('pointerdown')).toBe(1);
        expect(cible.compte('keydown')).toBe(1);
        // Does not report a success that did not happen.
        expect(surEtat).not.toHaveBeenCalledWith(true);

        // A later gesture can retry — still refused here, but the
        // sequence does not throw and the listeners stay available.
        cible.declencher('keydown');
        expect(cible.compte('pointerdown')).toBe(1);
    });

    it('cancelling after an unmute that already happened breaks nothing and leaves the sound on', () => {
        // The gesture already removed everything itself (see the previous test):
        // `annuler()` must stay a silent no-op, neither re-muting nor throwing.
        const media = { muted: true };
        const cible = faireCible();
        const annuler = armerLeSon({ media, cible });
        cible.declencher('pointerdown');
        expect(media.muted).toBe(false);

        expect(() => annuler()).not.toThrow();
        expect(media.muted).toBe(false);
    });
});
