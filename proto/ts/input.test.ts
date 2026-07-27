import { describe, expect, it } from 'vitest';
import vectors from '../vectors.json';
import {
    PROTOCOL_VERSION,
    encodeKey,
    encodeMouseButton,
    encodeMouseMove,
    encodeWheel,
} from './input';

describe('encodeur du protocole d\'entrée', () => {
    it('déclare la même version que les vecteurs', () => {
        expect(PROTOCOL_VERSION).toBe(vectors.version);
    });

    it.each(vectors.cases)('produit les octets attendus pour « $name »', (testCase) => {
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
            default:
                throw new Error(`type de vecteur inconnu : ${testCase.kind}`);
        }
        expect(Array.from(actual)).toEqual(testCase.bytes);
    });

    it('borne les coordonnées hors plage', () => {
        expect(Array.from(encodeMouseMove(-10, 99999))).toEqual([1, 1, 0, 0, 255, 255]);
    });

    it('borne les deltas de molette hors plage', () => {
        expect(Array.from(encodeWheel(-40000, 40000))).toEqual([1, 3, 0, 128, 255, 127]);
    });
});
