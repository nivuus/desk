// Encodeur binaire des messages d'entrée. Doit rester strictement aligné sur
// proto/src/input.rs — les vecteurs de vectors.json vérifient les deux côtés.

export const PROTOCOL_VERSION = 2;

const TYPE_MOUSE_MOVE = 1;
const TYPE_MOUSE_BUTTON = 2;
const TYPE_WHEEL = 3;
const TYPE_KEY = 4;
const TYPE_MOUSE_MOVE_RELATIVE = 5;
const TYPE_GAMEPAD_STATE = 6;

export type MouseButtonCode = 0 | 1 | 2; // gauche, droit, milieu

/** État complet d'une manette, calqué sur XINPUT_GAMEPAD. */
export interface GamepadStateFields {
    seq: number;
    buttons: number;
    leftTrigger: number;
    rightTrigger: number;
    thumbLX: number;
    thumbLY: number;
    thumbRX: number;
    thumbRY: number;
}

function clampU16(value: number): number {
    return Math.max(0, Math.min(65535, Math.round(value)));
}

function clampI16(value: number): number {
    return Math.max(-32768, Math.min(32767, Math.round(value)));
}

function clampU8(value: number): number {
    return Math.max(0, Math.min(255, Math.round(value)));
}

export function encodeMouseMove(x: number, y: number): Uint8Array {
    const buffer = new Uint8Array(6);
    const view = new DataView(buffer.buffer);
    buffer[0] = PROTOCOL_VERSION;
    buffer[1] = TYPE_MOUSE_MOVE;
    view.setUint16(2, clampU16(x), true);
    view.setUint16(4, clampU16(y), true);
    return buffer;
}

export function encodeMouseButton(
    button: MouseButtonCode,
    pressed: boolean,
    x: number,
    y: number,
): Uint8Array {
    const buffer = new Uint8Array(8);
    const view = new DataView(buffer.buffer);
    buffer[0] = PROTOCOL_VERSION;
    buffer[1] = TYPE_MOUSE_BUTTON;
    buffer[2] = button;
    buffer[3] = pressed ? 1 : 0;
    view.setUint16(4, clampU16(x), true);
    view.setUint16(6, clampU16(y), true);
    return buffer;
}

export function encodeWheel(deltaX: number, deltaY: number): Uint8Array {
    const buffer = new Uint8Array(6);
    const view = new DataView(buffer.buffer);
    buffer[0] = PROTOCOL_VERSION;
    buffer[1] = TYPE_WHEEL;
    view.setInt16(2, clampI16(deltaX), true);
    view.setInt16(4, clampI16(deltaY), true);
    return buffer;
}

export function encodeKey(scancode: number, pressed: boolean, extended: boolean): Uint8Array {
    const buffer = new Uint8Array(6);
    const view = new DataView(buffer.buffer);
    buffer[0] = PROTOCOL_VERSION;
    buffer[1] = TYPE_KEY;
    view.setUint16(2, clampU16(scancode), true);
    buffer[4] = pressed ? 1 : 0;
    buffer[5] = extended ? 1 : 0;
    return buffer;
}

export function encodeMouseMoveRelative(dx: number, dy: number): Uint8Array {
    const buffer = new Uint8Array(6);
    const view = new DataView(buffer.buffer);
    buffer[0] = PROTOCOL_VERSION;
    buffer[1] = TYPE_MOUSE_MOVE_RELATIVE;
    view.setInt16(2, clampI16(dx), true);
    view.setInt16(4, clampI16(dy), true);
    return buffer;
}

export function encodeGamepadState(state: GamepadStateFields): Uint8Array {
    const buffer = new Uint8Array(16);
    const view = new DataView(buffer.buffer);
    buffer[0] = PROTOCOL_VERSION;
    buffer[1] = TYPE_GAMEPAD_STATE;
    view.setUint16(2, clampU16(state.seq), true);
    view.setUint16(4, clampU16(state.buttons), true);
    buffer[6] = clampU8(state.leftTrigger);
    buffer[7] = clampU8(state.rightTrigger);
    view.setInt16(8, clampI16(state.thumbLX), true);
    view.setInt16(10, clampI16(state.thumbLY), true);
    view.setInt16(12, clampI16(state.thumbRX), true);
    view.setInt16(14, clampI16(state.thumbRY), true);
    return buffer;
}
