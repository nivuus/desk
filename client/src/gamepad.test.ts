// Tests of the gamepad on the client side.
//
// Like pointer.ts and fullscreen.ts, `attachGamepad` receives its dependencies
// by injection: the gamepad source, the timer (set/cancel) and
// the clock are all stand-ins here, which makes the polling loop
// testable without the real `navigator.getGamepads`, `window.setInterval` or
// `performance.now`.

import { describe, expect, it, vi } from 'vitest';
import { encodeGamepadState } from '../../proto/ts/input';
import {
    ETAT_NEUTRE,
    aChange,
    attachGamepad,
    versEtatXInput,
    type GamepadConnectee,
    type GamepadLike,
} from './gamepad';

function pad(overrides: Partial<GamepadLike> = {}): GamepadLike {
    return {
        buttons: Array.from({ length: 16 }, () => ({ pressed: false, value: 0 })),
        axes: [0, 0, 0, 0],
        ...overrides,
    };
}

describe('conversion Gamepad API → XInput', () => {
    it('returns a neutral state for an idle gamepad', () => {
        expect(versEtatXInput(pad(), 7)).toEqual({ ...ETAT_NEUTRE, seq: 7 });
    });

    it('maps the buttons onto the XInput masks', () => {
        const boutons = pad().buttons.map((b) => ({ ...b }));
        boutons[0] = { pressed: true, value: 1 }; // A
        boutons[12] = { pressed: true, value: 1 }; // croix haut
        const etat = versEtatXInput(pad({ buttons: boutons }), 0);
        expect(etat.buttons).toBe(0x1000 | 0x0001);
    });

    it('maps the triggers onto 0..255', () => {
        const boutons = pad().buttons.map((b) => ({ ...b }));
        boutons[6] = { pressed: true, value: 1 };
        boutons[7] = { pressed: true, value: 0.5 };
        const etat = versEtatXInput(pad({ buttons: boutons }), 0);
        expect(etat.leftTrigger).toBe(255);
        expect(etat.rightTrigger).toBe(128);
    });

    it("inverse l'axe vertical", () => {
        // The Gamepad API counts Y DOWNWARDS, XInput UPWARDS: without
        // inversion, vertical aim is upside down in every game.
        const etat = versEtatXInput(pad({ axes: [0, -1, 0, 1] }), 0);
        expect(etat.thumbLY).toBe(32767);
        expect(etat.thumbRY).toBe(-32767);
    });

    it('clamps the axes to the i16 range', () => {
        const etat = versEtatXInput(pad({ axes: [-1, 0, 1, 0] }), 0);
        expect(etat.thumbLX).toBe(-32767);
        expect(etat.thumbRX).toBe(32767);
    });

    it('tolerates a gamepad without the 16 standard buttons', () => {
        const etat = versEtatXInput(pad({ buttons: [{ pressed: true, value: 1 }] }), 0);
        expect(etat.buttons).toBe(0x1000);
    });
});

describe('change detection', () => {
    it('ignores the sequence number', () => {
        // `seq` changes on every message by construction: comparing it
        // would make detection always true and cancel the saving.
        expect(aChange({ ...ETAT_NEUTRE, seq: 1 }, { ...ETAT_NEUTRE, seq: 2 })).toBe(false);
    });

    it('detects a button change', () => {
        expect(
            aChange({ ...ETAT_NEUTRE, seq: 1 }, { ...ETAT_NEUTRE, seq: 1, buttons: 0x1000 }),
        ).toBe(true);
    });

    it("detects an axis change", () => {
        expect(
            aChange({ ...ETAT_NEUTRE, seq: 1 }, { ...ETAT_NEUTRE, seq: 1, thumbLX: 1 }),
        ).toBe(true);
    });
});

/**
 * Stand-in for `navigator.getGamepads()`: a single gamepad, pluggable and
 * unpluggable on demand, with an observable vibration actuator.
 */
function faireSourceManettes() {
    let manette: GamepadConnectee | null = null;
    const effets: Array<{ type: string; params: unknown }> = [];
    return {
        get(): ReadonlyArray<GamepadConnectee | null> {
            return [manette];
        },
        brancher(overrides: Partial<GamepadConnectee> = {}): GamepadConnectee {
            manette = {
                connected: true,
                buttons: Array.from({ length: 16 }, () => ({ pressed: false, value: 0 })),
                axes: [0, 0, 0, 0],
                vibrationActuator: {
                    playEffect: (type, params) => {
                        effets.push({ type, params });
                        return Promise.resolve();
                    },
                },
                ...overrides,
            };
            return manette;
        },
        debrancher(): void {
            manette = null;
        },
        effets,
    };
}

/** Stand-in for `window.setInterval`/`clearInterval`: driven by hand. */
function faireMinuteur() {
    let prochainId = 1;
    const fonctions = new Map<number, () => void>();
    const annules: number[] = [];
    return {
        poser(callback: () => void): number {
            const id = prochainId;
            prochainId += 1;
            fonctions.set(id, callback);
            return id;
        },
        annuler(id: number): void {
            annules.push(id);
            fonctions.delete(id);
        },
        /** Fires the (only) round set — `attachGamepad` only sets one. */
        declencher(): void {
            for (const callback of fonctions.values()) callback();
        },
        annules,
        get scheduledCount(): number {
            return fonctions.size + annules.length;
        },
    };
}

/** Test double of `performance.now()`: advances only on command. */
function faireHorloge(depart = 0) {
    let t = depart;
    return {
        maintenant: (): number => t,
        avancer(ms: number): void {
            t += ms;
        },
    };
}

describe('attachGamepad', () => {
    it('does nothing as long as no gamepad has ever been plugged in', () => {
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge();
        const envoyer = vi.fn();
        attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        minuteur.declencher();
        minuteur.declencher();

        expect(envoyer).not.toHaveBeenCalled();
    });

    it("emits a state only when something changes, not on every tick", () => {
        // Counter-proof: if the `aChange(...) || expire` guard of the tested code
        // were replaced by an unconditional send, this test would fail
        // (send would be called from the first round, whereas it must
        // only be called on the second, once the button is pressed).
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge(1000);
        const envoyer = vi.fn();
        const pad = manettes.brancher();
        attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        // First round: nothing has changed since the initial state, and the
        // refresh (100 ms) is not due yet.
        minuteur.declencher();
        expect(envoyer).not.toHaveBeenCalled();

        // A button is pressed, without time moving forward: only the change
        // must trigger the emission.
        (pad.buttons as Array<{ pressed: boolean; value: number }>)[0] = { pressed: true, value: 1 };
        minuteur.declencher();

        expect(envoyer).toHaveBeenCalledTimes(1);
        const attendu = encodeGamepadState({ ...ETAT_NEUTRE, seq: 2, buttons: 0x1000 });
        expect(envoyer).toHaveBeenCalledWith(attendu);
    });

    it('refreshes periodically even without any change', () => {
        // Counter-proof: without the `expire` clause, this test would fail — it is
        // precisely what tells the refresh from change detection, and
        // it is what carries the self-healing guarantee
        // on an unreliable channel.
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge(1000);
        const envoyer = vi.fn();
        manettes.brancher();
        attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        minuteur.declencher();
        expect(envoyer).not.toHaveBeenCalled();

        // Less than 100 ms: still nothing.
        horloge.avancer(50);
        minuteur.declencher();
        expect(envoyer).not.toHaveBeenCalled();

        // 100 ms passed since the last send (which never happened,
        // so since attaching): the refresh must emit the unchanged neutral
        // state.
        horloge.avancer(50);
        minuteur.declencher();
        expect(envoyer).toHaveBeenCalledTimes(1);
    });

    it('emits an immediate neutral state on unplug, without waiting for the refresh', () => {
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge(1000);
        const envoyer = vi.fn();
        const surPresence = vi.fn();
        const pad = manettes.brancher();
        attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer, surPresence });

        // Establishes presence.
        minuteur.declencher();
        expect(surPresence).toHaveBeenCalledWith(true);
        envoyer.mockClear();

        // A key stays pressed at the moment of unplugging...
        (pad.buttons as Array<{ pressed: boolean; value: number }>)[0] = { pressed: true, value: 1 };
        manettes.debrancher();
        // ...and time does NOT move forward: the guarantee owes nothing to
        // `RAFRAICHISSEMENT_MS`.
        minuteur.declencher();

        expect(surPresence).toHaveBeenCalledWith(false);
        expect(envoyer).toHaveBeenCalledTimes(1);
        const attendu = encodeGamepadState({ ...ETAT_NEUTRE, seq: 2 });
        expect(envoyer).toHaveBeenCalledWith(attendu);
    });

    it('stops the timer on detach', () => {
        // Counter-proof: if `detacher` forgot to call `minuteur.annuler`,
        // `annules` would stay empty and the assertion would fail.
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge();
        const envoyer = vi.fn();
        const handle = attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        expect(minuteur.annules).toEqual([]);
        handle.detacher();
        expect(minuteur.annules).toEqual([1]);
    });

    it('surVibration relays the normalised magnitudes to the plugged gamepad', () => {
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge();
        const envoyer = vi.fn();
        manettes.brancher();
        const handle = attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        handle.surVibration(255, 128);

        expect(manettes.effets).toEqual([
            {
                type: 'dual-rumble',
                params: { duration: 200, strongMagnitude: 1, weakMagnitude: 128 / 255 },
            },
        ]);
    });

    it("surVibration does not throw when no gamepad is plugged in", () => {
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge();
        const envoyer = vi.fn();
        const handle = attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        expect(() => handle.surVibration(255, 255)).not.toThrow();
    });
});
