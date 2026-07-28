import { describe, expect, it } from 'vitest';
import vectors from '../vectors.json';
import {
    PROTOCOL_VERSION,
    encodeGamepadState,
    encodeKey,
    encodeMouseButton,
    encodeMouseMove,
    encodeMouseMoveRelative,
    encodeWheel,
} from './input';

/**
 * Typage explicite des cas de vecteurs partagés : le JSON mélange des champs
 * propres à chaque `kind`, tous optionnels ici puisqu'aucun cas ne les porte
 * tous. Un typage local évite à TypeScript d'inférer une union imprécise
 * (et évite de disperser des `as any` dans le corps du test).
 */
interface VectorCase {
    name: string;
    kind: string;
    bytes: number[];
    x?: number;
    y?: number;
    button?: number;
    pressed?: boolean;
    delta_x?: number;
    delta_y?: number;
    scancode?: number;
    extended?: boolean;
    dx?: number;
    dy?: number;
    seq?: number;
    buttons?: number;
    left_trigger?: number;
    right_trigger?: number;
    thumb_lx?: number;
    thumb_ly?: number;
    thumb_rx?: number;
    thumb_ry?: number;
}

const cases: VectorCase[] = vectors.cases;

describe('encodeur du protocole d\'entrée', () => {
    it('déclare la même version que les vecteurs', () => {
        expect(PROTOCOL_VERSION).toBe(vectors.version);
    });

    it.each(cases)('produit les octets attendus pour « $name »', (testCase) => {
        let actual: Uint8Array;
        switch (testCase.kind) {
            case 'mouse_move':
                actual = encodeMouseMove(testCase.x!, testCase.y!);
                break;
            case 'mouse_button':
                actual = encodeMouseButton(
                    testCase.button! as 0 | 1 | 2,
                    testCase.pressed!,
                    testCase.x!,
                    testCase.y!,
                );
                break;
            case 'wheel':
                actual = encodeWheel(testCase.delta_x!, testCase.delta_y!);
                break;
            case 'key':
                actual = encodeKey(testCase.scancode!, testCase.pressed!, testCase.extended!);
                break;
            case 'mouse_move_relative':
                actual = encodeMouseMoveRelative(testCase.dx!, testCase.dy!);
                break;
            case 'gamepad_state':
                actual = encodeGamepadState({
                    seq: testCase.seq!,
                    buttons: testCase.buttons!,
                    leftTrigger: testCase.left_trigger!,
                    rightTrigger: testCase.right_trigger!,
                    thumbLX: testCase.thumb_lx!,
                    thumbLY: testCase.thumb_ly!,
                    thumbRX: testCase.thumb_rx!,
                    thumbRY: testCase.thumb_ry!,
                });
                break;
            default:
                throw new Error(`type de vecteur inconnu : ${testCase.kind}`);
        }
        expect(Array.from(actual)).toEqual(testCase.bytes);
    });

    it('borne les coordonnées hors plage', () => {
        expect(Array.from(encodeMouseMove(-10, 99999))).toEqual([2, 1, 0, 0, 255, 255]);
    });

    it('borne les deltas de molette hors plage', () => {
        expect(Array.from(encodeWheel(-40000, 40000))).toEqual([2, 3, 0, 128, 255, 127]);
    });

    it('borne les deltas relatifs hors plage', () => {
        expect(Array.from(encodeMouseMoveRelative(-40000, 40000))).toEqual([2, 5, 0, 128, 255, 127]);
    });
});
