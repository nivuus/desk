// Tests de la manette côté client.
//
// Comme pointer.ts et fullscreen.ts, `attachGamepad` reçoit ses dépendances
// par injection : la source de manettes, le minuteur (pose/annulation) et
// l'horloge sont tous des doublures ici, ce qui rend la boucle de sondage
// testable sans `navigator.getGamepads`, `window.setInterval` ni
// `performance.now` réels.

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
    it('rend un état neutre pour une manette au repos', () => {
        expect(versEtatXInput(pad(), 7)).toEqual({ ...ETAT_NEUTRE, seq: 7 });
    });

    it('mappe les boutons sur les masques XInput', () => {
        const boutons = pad().buttons.map((b) => ({ ...b }));
        boutons[0] = { pressed: true, value: 1 }; // A
        boutons[12] = { pressed: true, value: 1 }; // croix haut
        const etat = versEtatXInput(pad({ buttons: boutons }), 0);
        expect(etat.buttons).toBe(0x1000 | 0x0001);
    });

    it('mappe les gâchettes sur 0..255', () => {
        const boutons = pad().buttons.map((b) => ({ ...b }));
        boutons[6] = { pressed: true, value: 1 };
        boutons[7] = { pressed: true, value: 0.5 };
        const etat = versEtatXInput(pad({ buttons: boutons }), 0);
        expect(etat.leftTrigger).toBe(255);
        expect(etat.rightTrigger).toBe(128);
    });

    it("inverse l'axe vertical", () => {
        // La Gamepad API compte Y vers le BAS, XInput vers le HAUT : sans
        // inversion, la visée verticale est à l'envers dans tous les jeux.
        const etat = versEtatXInput(pad({ axes: [0, -1, 0, 1] }), 0);
        expect(etat.thumbLY).toBe(32767);
        expect(etat.thumbRY).toBe(-32767);
    });

    it('borne les axes à la plage i16', () => {
        const etat = versEtatXInput(pad({ axes: [-1, 0, 1, 0] }), 0);
        expect(etat.thumbLX).toBe(-32767);
        expect(etat.thumbRX).toBe(32767);
    });

    it('tolère une manette sans les 16 boutons standard', () => {
        const etat = versEtatXInput(pad({ buttons: [{ pressed: true, value: 1 }] }), 0);
        expect(etat.buttons).toBe(0x1000);
    });
});

describe('détection de changement', () => {
    it('ignore le numéro de séquence', () => {
        // `seq` change à chaque message par construction : le comparer
        // rendrait la détection toujours vraie et annulerait l'économie.
        expect(aChange({ ...ETAT_NEUTRE, seq: 1 }, { ...ETAT_NEUTRE, seq: 2 })).toBe(false);
    });

    it('détecte un changement de bouton', () => {
        expect(
            aChange({ ...ETAT_NEUTRE, seq: 1 }, { ...ETAT_NEUTRE, seq: 1, buttons: 0x1000 }),
        ).toBe(true);
    });

    it("détecte un changement d'axe", () => {
        expect(
            aChange({ ...ETAT_NEUTRE, seq: 1 }, { ...ETAT_NEUTRE, seq: 1, thumbLX: 1 }),
        ).toBe(true);
    });
});

/**
 * Doublure de `navigator.getGamepads()` : une seule manette, branchable et
 * débranchable à la demande, avec un actionneur de vibration observable.
 */
function faireSourceManettes() {
    let manette: GamepadConnectee | null = null;
    const effets: Array<{ type: string; parametres: unknown }> = [];
    return {
        obtenir(): ReadonlyArray<GamepadConnectee | null> {
            return [manette];
        },
        brancher(overrides: Partial<GamepadConnectee> = {}): GamepadConnectee {
            manette = {
                connected: true,
                buttons: Array.from({ length: 16 }, () => ({ pressed: false, value: 0 })),
                axes: [0, 0, 0, 0],
                vibrationActuator: {
                    playEffect: (type, parametres) => {
                        effets.push({ type, parametres });
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

/** Doublure de `window.setInterval`/`clearInterval` : pilotée à la main. */
function faireMinuteur() {
    let prochainId = 1;
    const fonctions = new Map<number, () => void>();
    const annules: number[] = [];
    return {
        poser(fonction: () => void): number {
            const id = prochainId;
            prochainId += 1;
            fonctions.set(id, fonction);
            return id;
        },
        annuler(id: number): void {
            annules.push(id);
            fonctions.delete(id);
        },
        /** Déclenche le (seul) tour posé — `attachGamepad` n'en pose qu'un. */
        declencher(): void {
            for (const fonction of fonctions.values()) fonction();
        },
        annules,
        get nombrePoses(): number {
            return fonctions.size + annules.length;
        },
    };
}

/** Doublure de `performance.now()` : avance uniquement sur commande. */
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
    it('ne fait rien tant qu\'aucune manette n\'a jamais été branchée', () => {
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge();
        const envoyer = vi.fn();
        attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        minuteur.declencher();
        minuteur.declencher();

        expect(envoyer).not.toHaveBeenCalled();
    });

    it("émet un état seulement quand quelque chose change, pas à chaque tour", () => {
        // Contre-preuve : si la garde `aChange(...) || expire` du code testé
        // était remplacée par un envoi inconditionnel, ce test échouerait
        // (envoyer serait appelé dès le premier tour, alors qu'il ne doit
        // l'être qu'au second, une fois le bouton pressé).
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge(1000);
        const envoyer = vi.fn();
        const pad = manettes.brancher();
        attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        // Premier tour : rien n'a changé depuis l'état initial, et le
        // rafraîchissement (100 ms) n'est pas encore échu.
        minuteur.declencher();
        expect(envoyer).not.toHaveBeenCalled();

        // Un bouton se presse, sans que le temps avance : seul le changement
        // doit déclencher l'émission.
        (pad.buttons as Array<{ pressed: boolean; value: number }>)[0] = { pressed: true, value: 1 };
        minuteur.declencher();

        expect(envoyer).toHaveBeenCalledTimes(1);
        const attendu = encodeGamepadState({ ...ETAT_NEUTRE, seq: 2, buttons: 0x1000 });
        expect(envoyer).toHaveBeenCalledWith(attendu);
    });

    it('rafraîchit périodiquement même sans aucun changement', () => {
        // Contre-preuve : sans la clause `expire`, ce test échouerait — c'est
        // justement ce qui distingue le rafraîchissement de la détection de
        // changement, et c'est lui qui porte la garantie d'auto-réparation
        // sur un canal non fiable.
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge(1000);
        const envoyer = vi.fn();
        manettes.brancher();
        attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        minuteur.declencher();
        expect(envoyer).not.toHaveBeenCalled();

        // Moins de 100 ms : toujours rien.
        horloge.avancer(50);
        minuteur.declencher();
        expect(envoyer).not.toHaveBeenCalled();

        // 100 ms franchies depuis le dernier envoi (qui n'a jamais eu lieu,
        // donc depuis l'attache) : le rafraîchissement doit émettre l'état
        // neutre inchangé.
        horloge.avancer(50);
        minuteur.declencher();
        expect(envoyer).toHaveBeenCalledTimes(1);
    });

    it('émet un état neutre immédiat au débranchement, sans attendre le rafraîchissement', () => {
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge(1000);
        const envoyer = vi.fn();
        const surPresence = vi.fn();
        const pad = manettes.brancher();
        attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer, surPresence });

        // Établit la présence.
        minuteur.declencher();
        expect(surPresence).toHaveBeenCalledWith(true);
        envoyer.mockClear();

        // Une touche reste enfoncée au moment du débranchement...
        (pad.buttons as Array<{ pressed: boolean; value: number }>)[0] = { pressed: true, value: 1 };
        manettes.debrancher();
        // ...et le temps n'avance PAS : la garantie ne doit rien à
        // `RAFRAICHISSEMENT_MS`.
        minuteur.declencher();

        expect(surPresence).toHaveBeenCalledWith(false);
        expect(envoyer).toHaveBeenCalledTimes(1);
        const attendu = encodeGamepadState({ ...ETAT_NEUTRE, seq: 2 });
        expect(envoyer).toHaveBeenCalledWith(attendu);
    });

    it('arrête le minuteur au détachement', () => {
        // Contre-preuve : si `detacher` oubliait d'appeler `minuteur.annuler`,
        // `annules` resterait vide et l'assertion échouerait.
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge();
        const envoyer = vi.fn();
        const handle = attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        expect(minuteur.annules).toEqual([]);
        handle.detacher();
        expect(minuteur.annules).toEqual([1]);
    });

    it('surVibration relaie les magnitudes normalisées à la manette branchée', () => {
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
                parametres: { duration: 200, strongMagnitude: 1, weakMagnitude: 128 / 255 },
            },
        ]);
    });

    it("surVibration ne lève pas quand aucune manette n'est branchée", () => {
        const manettes = faireSourceManettes();
        const minuteur = faireMinuteur();
        const horloge = faireHorloge();
        const envoyer = vi.fn();
        const handle = attachGamepad({ manettes, minuteur, horloge: horloge.maintenant, envoyer });

        expect(() => handle.surVibration(255, 255)).not.toThrow();
    });
});
