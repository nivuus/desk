// Manette : sondage, conversion vers XInput, émission.
//
// L'état voyage COMPLET et idempotent, jamais en différentiel : sur un canal
// non fiable et non ordonné, un état perdu se répare au message suivant, là
// où un événement « bouton relâché » perdu laisserait une touche coincée
// jusqu'à la fin de la partie. D'où l'émission sur changement PLUS un
// rafraîchissement périodique — et c'est ce rafraîchissement, pas la
// fréquence de sondage, qui porte la garantie d'auto-réparation.
//
// Comme pointer.ts et fullscreen.ts, les dépendances (source de manettes,
// minuteur, horloge) sont INJECTÉES plutôt que lues dans les objets globaux,
// ce qui rend ce module testable sans `navigator` ni `window`.

import { encodeGamepadState, type GamepadStateFields } from '../../proto/ts/input';

/**
 * Période de sondage visée : 4 ms est le plancher déclaré des navigateurs.
 * Rien ne garantit qu'il soit tenu pendant une session vidéo active — la
 * sonde qui aurait mesuré la cadence réelle sous charge n'a pas été faite.
 * La correction de ce module ne dépend PAS de cette valeur : c'est le
 * rafraîchissement périodique ci-dessous, comparé à l'horloge injectée, qui
 * rend l'état auto-réparant, quelle que soit la cadence réellement obtenue.
 */
const PERIODE_MS = 4;

/**
 * Rafraîchissement périodique même sans changement. C'est ce qui rend l'état
 * auto-réparant sur un canal non fiable : indépendant de la cadence réelle du
 * minuteur, il se contente de comparer l'horloge injectée au dernier envoi.
 */
const RAFRAICHISSEMENT_MS = 100;

/** Masques de `XINPUT_GAMEPAD.wButtons`, dans l'ordre du mapping `standard`. */
const MASQUES: ReadonlyArray<number> = [
    0x1000, // 0  A
    0x2000, // 1  B
    0x4000, // 2  X
    0x8000, // 3  Y
    0x0100, // 4  LB
    0x0200, // 5  RB
    0, //      6  LT — gâchette analogique, pas un bouton XInput
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

/** Ce dont ce module a besoin d'une manette pour la conversion XInput. */
export interface GamepadLike {
    buttons: ReadonlyArray<{ pressed: boolean; value: number }>;
    axes: readonly number[];
}

/** Ce dont ce module a besoin de `GamepadHapticActuator.playEffect`. */
export interface ActuateurVibration {
    playEffect?(
        type: 'dual-rumble',
        parametres: { duration: number; strongMagnitude: number; weakMagnitude: number },
    ): unknown;
}

/**
 * Ce dont la boucle de sondage a besoin en plus de `GamepadLike` : la
 * présence (`Gamepad.connected` reste `true` un moment après débranchement
 * sur certains navigateurs, d'où la vérification explicite) et l'actionneur
 * de vibration, optionnel.
 */
export interface GamepadConnectee extends GamepadLike {
    connected: boolean;
    vibrationActuator?: ActuateurVibration;
}

/** Ce dont ce module a besoin de `navigator.getGamepads()`. */
export interface SourceManettes {
    obtenir(): ReadonlyArray<GamepadConnectee | null>;
}

/** Ce dont ce module a besoin de `window.setInterval`/`clearInterval`. */
export interface Minuteur {
    poser(fonction: () => void, delaiMs: number): number;
    annuler(id: number): void;
}

/** Horloge injectée : `performance.now()` en production. */
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
    // Aucune zone morte n'est appliquée : les jeux appliquent la leur, en
    // ajouter une ici la creuserait deux fois.
    const borne = Math.max(-32768, Math.min(32767, Math.round((valeur ?? 0) * 32767)));
    // `-(0)` produit `-0` (l'inversion de l'axe vertical le fait pour une
    // manette au repos) : `+ 0` le ramène à `0` positif, sans quoi l'état
    // neutre ne serait pas structurellement égal à `ETAT_NEUTRE`.
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
        // La Gamepad API compte l'axe vertical vers le bas, XInput vers le
        // haut : sans inversion, la visée verticale serait à l'envers dans
        // tous les jeux.
        thumbLY: axe(-(pad.axes[1] ?? 0)),
        thumbRX: axe(pad.axes[2]),
        thumbRY: axe(-(pad.axes[3] ?? 0)),
    };
}

/** Compare deux états SANS tenir compte de `seq`, qui change à chaque appel. */
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
    /** Appelée quand une manette apparaît ou disparaît, pour l'affichage. */
    surPresence?: (present: boolean) => void;
}

export interface GamepadHandle {
    /** À appeler à réception d'un message de contrôle `rumble`. */
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
    // Initialisé à l'attache, pas à 0 : sans quoi le tout premier tour serait
    // à tort considéré comme un rafraîchissement échu (voir le test dédié à
    // l'émission sur changement, qui distingue les deux chemins).
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
                // Débranchement : un état neutre IMMÉDIAT, sans attendre le
                // rafraîchissement — sans lui, la dernière touche enfoncée
                // resterait coincée dans le jeu.
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
            // Absent sur les manettes ou navigateurs qui ne l'implémentent
            // pas : ignoré silencieusement.
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
