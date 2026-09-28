// Gamepad: polling, conversion to XInput, emission.
//
// The state travels COMPLETE and idempotent, never as a delta: on an
// unreliable and unordered channel, a lost state is repaired by the next message, whereas
// a lost "button released" event would leave a key stuck
// until the end of the game. Hence emission on change PLUS a
// periodic refresh — and it is this refresh, not the
// polling frequency, that carries the self-repair guarantee.
//
// Like pointer.ts and fullscreen.ts, dependencies (gamepad source,
// timer, clock) are INJECTED rather than read from the global objects,
// which makes this module testable without `navigator` or `window`.

import { encodeGamepadState, type GamepadStateFields } from '../../proto/ts/input';

/**
 * Targeted polling period: 4 ms is the browsers' declared floor.
 * Nothing guarantees it holds during an active video session — the
 * probe that would have measured the real cadence under load was not done.
 * The correctness of this module does NOT depend on this value: it is the
 * periodic refresh below, compared with the injected clock, that
 * makes the state self-repairing, whatever cadence is actually obtained.
 */
const PERIODE_MS = 4;

/**
 * Periodic refresh even without change. It is what makes the state
 * self-repairing on an unreliable channel: independent of the timer's real
 * cadence, it merely compares the injected clock with the last send.
 */
const RAFRAICHISSEMENT_MS = 100;

/** Masks of `XINPUT_GAMEPAD.wButtons`, in the order of the `standard` mapping. */
const MASQUES: ReadonlyArray<number> = [
    0x1000, // 0  A
    0x2000, // 1  B
    0x4000, // 2  X
    0x8000, // 3  Y
    0x0100, // 4  LB
    0x0200, // 5  RB
    0, //      6  LT — analogue trigger, not an XInput button
    0, //      7  RT
    0x0020, // 8  Back
    0x0010, // 9  Start
    0x0040, // 10 L3
    0x0080, // 11 R3
    0x0001, // 12 croix haut
    0x0002, // 13 croix bas
    0x0004, // 14 croix gauche
    0x0008, // 15 croix droite
];

/** What this module needs from a gamepad for the XInput conversion. */
export interface GamepadLike {
    buttons: ReadonlyArray<{ pressed: boolean; value: number }>;
    axes: readonly number[];
}

/** What this module needs from `GamepadHapticActuator.playEffect`. */
export interface ActuateurVibration {
    playEffect?(
        type: 'dual-rumble',
        parametres: { duration: number; strongMagnitude: number; weakMagnitude: number },
    ): unknown;
}

/**
 * What the polling loop needs on top of `GamepadLike`: the
 * presence (`Gamepad.connected` stays `true` for a while after unplugging
 * on some browsers, hence the explicit check) and the rumble
 * actuator, optional.
 */
export interface GamepadConnectee extends GamepadLike {
    connected: boolean;
    vibrationActuator?: ActuateurVibration;
}

/** What this module needs from `navigator.getGamepads()`. */
export interface SourceManettes {
    obtenir(): ReadonlyArray<GamepadConnectee | null>;
}

/** What this module needs from `window.setInterval`/`clearInterval`. */
export interface Minuteur {
    poser(fonction: () => void, delaiMs: number): number;
    annuler(id: number): void;
}

/** Injected clock: `performance.now()` in production. */
export type Horloge = () => number;

export const ETAT_NEUTRE: Omit<GamepadStateFields, 'seq'> = {
    buttons: 0,
    leftTrigger: 0,
    rightTrigger: 0,
    thumbLX: 0,
    thumbLY: 0,
    thumbRX: 0,
    thumbRY: 0,
};

function axe(valeur: number | undefined): number {
    // No dead zone is applied: games apply their own, adding
    // one here would dig it twice.
    const borne = Math.max(-32768, Math.min(32767, Math.round((valeur ?? 0) * 32767)));
    // `-(0)` produces `-0` (inverting the vertical axis does so for a
    // gamepad at rest): `+ 0` brings it back to positive `0`, otherwise the neutral
    // state would not be structurally equal to `ETAT_NEUTRE`.
    return borne + 0;
}

export function versEtatXInput(pad: GamepadLike, seq: number): GamepadStateFields {
    let buttons = 0;
    for (let i = 0; i < MASQUES.length; i += 1) {
        if (pad.buttons[i]?.pressed) buttons |= MASQUES[i];
    }
    return {
        seq,
        buttons,
        leftTrigger: Math.round((pad.buttons[6]?.value ?? 0) * 255),
        rightTrigger: Math.round((pad.buttons[7]?.value ?? 0) * 255),
        thumbLX: axe(pad.axes[0]),
        // The Gamepad API counts the vertical axis downwards, XInput
        // upwards: without inversion, vertical aiming would be upside down in
        // every game.
        thumbLY: axe(-(pad.axes[1] ?? 0)),
        thumbRX: axe(pad.axes[2]),
        thumbRY: axe(-(pad.axes[3] ?? 0)),
    };
}

/** Compares two states WITHOUT taking `seq` into account, which changes at each call. */
export function aChange(a: GamepadStateFields, b: GamepadStateFields): boolean {
    return (
        a.buttons !== b.buttons ||
        a.leftTrigger !== b.leftTrigger ||
        a.rightTrigger !== b.rightTrigger ||
        a.thumbLX !== b.thumbLX ||
        a.thumbLY !== b.thumbLY ||
        a.thumbRX !== b.thumbRX ||
        a.thumbRY !== b.thumbRY
    );
}

export interface GamepadOptions {
    manettes: SourceManettes;
    minuteur: Minuteur;
    horloge: Horloge;
    envoyer: (payload: Uint8Array) => void;
    /** Called when a gamepad appears or disappears, for display. */
    surPresence?: (present: boolean) => void;
}

export interface GamepadHandle {
    /** To call on receiving a `rumble` control message. */
    surVibration(gauche: number, droite: number): void;
    detacher(): void;
}

function manetteBranchee(manettes: SourceManettes): GamepadConnectee | undefined {
    return manettes.obtenir().find((p): p is GamepadConnectee => p !== null && p.connected);
}

export function attachGamepad({
    manettes,
    minuteur,
    horloge,
    envoyer,
    surPresence,
}: GamepadOptions): GamepadHandle {
    let seq = 0;
    let dernier: GamepadStateFields = { ...ETAT_NEUTRE, seq: 0 };
    // Initialised at attach time, not at 0: otherwise the very first round would
    // wrongly be considered an elapsed refresh (see the test dedicated to
    // emission on change, which distinguishes the two paths).
    let dernierEnvoi = horloge();
    let presentPrecedent = false;

    const emettre = (etat: GamepadStateFields): void => {
        dernier = etat;
        dernierEnvoi = horloge();
        envoyer(encodeGamepadState(etat));
    };

    const tour = (): void => {
        const pad = manetteBranchee(manettes);
        const present = pad !== undefined;

        if (present !== presentPrecedent) {
            presentPrecedent = present;
            surPresence?.(present);
            if (!present) {
                // Unplugging: an IMMEDIATE neutral state, without waiting for the
                // refresh — without it, the last pressed key
                // would stay stuck in the game.
                seq = (seq + 1) & 0xffff;
                emettre({ ...ETAT_NEUTRE, seq });
                return;
            }
        }
        if (!pad) return;

        seq = (seq + 1) & 0xffff;
        const etat = versEtatXInput(pad, seq);
        const expire = horloge() - dernierEnvoi >= RAFRAICHISSEMENT_MS;
        if (aChange(dernier, etat) || expire) emettre(etat);
    };

    const id = minuteur.poser(tour, PERIODE_MS);

    return {
        surVibration(gauche, droite) {
            const pad = manetteBranchee(manettes);
            // Absent on gamepads or browsers that do not implement
            // it: silently ignored.
            void pad?.vibrationActuator?.playEffect?.('dual-rumble', {
                // Durée volontairement supérieure à la période de
                // rafraîchissement des vibrations côté agent : chaque message
                // remplace le précédent, et si l'agent se tait, l'effet
                // s'éteint seul plutôt que de rester bloqué.
                duration: 200,
                strongMagnitude: gauche / 255,
                weakMagnitude: droite / 255,
            });
        },
        detacher() {
            minuteur.annuler(id);
        },
    };
}

// Valeurs par défaut pour utilisation dans le navigateur réel (voir tâche 15 : câblage).
export function attachGamepadAuDOM(
    options: Omit<GamepadOptions, 'manettes' | 'minuteur' | 'horloge'>,
): GamepadHandle {
    return attachGamepad({
        manettes: {
            obtenir: () => navigator.getGamepads() as unknown as ReadonlyArray<GamepadConnectee | null>,
        },
        minuteur: {
            poser: (fonction, delaiMs) => window.setInterval(fonction, delaiMs),
            annuler: (id) => window.clearInterval(id),
        },
        horloge: () => performance.now(),
        ...options,
    });
}
