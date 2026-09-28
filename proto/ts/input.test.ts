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
 * Explicit typing of the shared vector cases: the JSON mixes fields
 * specific to each `kind`, all optional here since no case carries them
 * all. A local typing keeps TypeScript from inferring an imprecise union
 * (and avoids scattering `as any` through the test body).
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

describe('encoder of the input protocol', () => {
    it('declares the same version as the vectors', () => {
        expect(PROTOCOL_VERSION).toBe(vectors.version);
    });

    it.each(cases)('produces the expected bytes for « $name »', (testCase) => {
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
                throw new Error(`unknown vector type: ${testCase.kind}`);
        }
        expect(Array.from(actual)).toEqual(testCase.bytes);
    });

    it('bounds out-of-range coordinates', () => {
        expect(Array.from(encodeMouseMove(-10, 99999))).toEqual([2, 1, 0, 0, 255, 255]);
    });

    it('bounds out-of-range wheel deltas', () => {
        expect(Array.from(encodeWheel(-40000, 40000))).toEqual([2, 3, 0, 128, 255, 127]);
    });

    it('bounds out-of-range relative deltas', () => {
        expect(Array.from(encodeMouseMoveRelative(-40000, 40000))).toEqual([2, 5, 0, 128, 255, 127]);
    });

    it('bounds out-of-range triggers', () => {
        const state = encodeGamepadState({
            seq: 0,
            buttons: 0,
            leftTrigger: -10,
            rightTrigger: 999,
            thumbLX: 0,
            thumbLY: 0,
            thumbRX: 0,
            thumbRY: 0,
        });
        expect(Array.from(state)).toEqual([2, 6, 0, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0]);
    });
});
