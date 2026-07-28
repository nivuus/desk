// Capture des entrées et envoi sur le canal binaire.
//
// Les coordonnées sont normalisées sur 0..65535 par rapport à la zone d'image
// réellement affichée : `object-fit: contain` laisse des bandes noires qu'il
// faut exclure, sans quoi le pointeur dérive.

import {
    encodeKey,
    encodeMouseButton,
    encodeMouseMove,
    encodeWheel,
    type MouseButtonCode,
} from '../../proto/ts/input';

export interface InputOptions {
    video: HTMLVideoElement;
    channel: RTCDataChannel;
}

/** Zone occupée par l'image dans l'élément vidéo, bandes noires exclues. */
function contentRect(video: HTMLVideoElement): DOMRect {
    const element = video.getBoundingClientRect();
    const sourceWidth = video.videoWidth;
    const sourceHeight = video.videoHeight;
    if (!sourceWidth || !sourceHeight) return element;

    const scale = Math.min(element.width / sourceWidth, element.height / sourceHeight);
    const width = sourceWidth * scale;
    const height = sourceHeight * scale;
    return new DOMRect(
        element.x + (element.width - width) / 2,
        element.y + (element.height - height) / 2,
        width,
        height,
    );
}

function normalize(video: HTMLVideoElement, clientX: number, clientY: number): [number, number] {
    const rect = contentRect(video);
    const x = ((clientX - rect.x) / Math.max(1, rect.width)) * 65535;
    const y = ((clientY - rect.y) / Math.max(1, rect.height)) * 65535;
    return [x, y];
}

const BUTTON_MAP: Record<number, MouseButtonCode> = { 0: 0, 1: 2, 2: 1 };

export function attachInput({ video, channel }: InputOptions): () => void {
    const send = (payload: Uint8Array): void => {
        // Assertion nécessaire depuis TypeScript 5.7 : `Uint8Array` est
        // désormais générique sur son tampon sous-jacent, par défaut
        // `ArrayBufferLike` (qui inclut `SharedArrayBuffer`), alors que
        // `RTCDataChannel.send` exige spécifiquement `ArrayBuffer`. Les
        // tampons produits par `proto/ts/input.ts` (`new Uint8Array(n)`)
        // sont toujours adossés à un vrai `ArrayBuffer` en pratique — seul
        // le typage est trop large.
        if (channel.readyState === 'open') channel.send(payload as Uint8Array<ArrayBuffer>);
    };

    const onPointerMove = (event: PointerEvent): void => {
        // getCoalescedEvents restitue les positions intermédiaires que le
        // navigateur a regroupées : le tracé reste fidèle à haute fréquence.
        const events = event.getCoalescedEvents?.() ?? [event];
        for (const sample of events) {
            const [x, y] = normalize(video, sample.clientX, sample.clientY);
            send(encodeMouseMove(x, y));
        }
    };

    const onPointerDown = (event: PointerEvent): void => {
        video.setPointerCapture(event.pointerId);
        const [x, y] = normalize(video, event.clientX, event.clientY);
        send(encodeMouseButton(BUTTON_MAP[event.button] ?? 0, true, x, y));
    };

    const onPointerUp = (event: PointerEvent): void => {
        const [x, y] = normalize(video, event.clientX, event.clientY);
        send(encodeMouseButton(BUTTON_MAP[event.button] ?? 0, false, x, y));
    };

    const onWheel = (event: WheelEvent): void => {
        event.preventDefault();
        // Windows compte 120 unités par cran ; deltaMode 0 est en pixels.
        const factor = event.deltaMode === 0 ? -120 / 100 : -120;
        send(encodeWheel(event.deltaX * -factor, event.deltaY * factor));
    };

    const onContextMenu = (event: Event): void => event.preventDefault();

    const onKeyDown = (event: KeyboardEvent): void => {
        event.preventDefault();
        const mapped = SCANCODES[event.code];
        if (mapped) send(encodeKey(mapped.scancode, true, mapped.extended));
    };

    const onKeyUp = (event: KeyboardEvent): void => {
        event.preventDefault();
        const mapped = SCANCODES[event.code];
        if (mapped) send(encodeKey(mapped.scancode, false, mapped.extended));
    };

    video.addEventListener('pointermove', onPointerMove);
    video.addEventListener('pointerdown', onPointerDown);
    video.addEventListener('pointerup', onPointerUp);
    video.addEventListener('wheel', onWheel, { passive: false });
    video.addEventListener('contextmenu', onContextMenu);
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);

    return () => {
        video.removeEventListener('pointermove', onPointerMove);
        video.removeEventListener('pointerdown', onPointerDown);
        video.removeEventListener('pointerup', onPointerUp);
        video.removeEventListener('wheel', onWheel);
        video.removeEventListener('contextmenu', onContextMenu);
        window.removeEventListener('keydown', onKeyDown);
        window.removeEventListener('keyup', onKeyUp);
    };
}

/** Correspondance `KeyboardEvent.code` → scancode PS/2 (jeu 1).
 *
 * On passe par les scancodes plutôt que par les codes de touches virtuelles :
 * c'est la position physique de la touche qui est transmise, donc la
 * disposition configurée côté Windows s'applique correctement.
 */
const SCANCODES: Record<string, { scancode: number; extended: boolean }> = {
    Escape: { scancode: 0x01, extended: false },
    Digit1: { scancode: 0x02, extended: false },
    Digit2: { scancode: 0x03, extended: false },
    Digit3: { scancode: 0x04, extended: false },
    Digit4: { scancode: 0x05, extended: false },
    Digit5: { scancode: 0x06, extended: false },
    Digit6: { scancode: 0x07, extended: false },
    Digit7: { scancode: 0x08, extended: false },
    Digit8: { scancode: 0x09, extended: false },
    Digit9: { scancode: 0x0a, extended: false },
    Digit0: { scancode: 0x0b, extended: false },
    Minus: { scancode: 0x0c, extended: false },
    Equal: { scancode: 0x0d, extended: false },
    Backspace: { scancode: 0x0e, extended: false },
    Tab: { scancode: 0x0f, extended: false },
    KeyQ: { scancode: 0x10, extended: false },
    KeyW: { scancode: 0x11, extended: false },
    KeyE: { scancode: 0x12, extended: false },
    KeyR: { scancode: 0x13, extended: false },
    KeyT: { scancode: 0x14, extended: false },
    KeyY: { scancode: 0x15, extended: false },
    KeyU: { scancode: 0x16, extended: false },
    KeyI: { scancode: 0x17, extended: false },
    KeyO: { scancode: 0x18, extended: false },
    KeyP: { scancode: 0x19, extended: false },
    BracketLeft: { scancode: 0x1a, extended: false },
    BracketRight: { scancode: 0x1b, extended: false },
    Enter: { scancode: 0x1c, extended: false },
    ControlLeft: { scancode: 0x1d, extended: false },
    KeyA: { scancode: 0x1e, extended: false },
    KeyS: { scancode: 0x1f, extended: false },
    KeyD: { scancode: 0x20, extended: false },
    KeyF: { scancode: 0x21, extended: false },
    KeyG: { scancode: 0x22, extended: false },
    KeyH: { scancode: 0x23, extended: false },
    KeyJ: { scancode: 0x24, extended: false },
    KeyK: { scancode: 0x25, extended: false },
    KeyL: { scancode: 0x26, extended: false },
    Semicolon: { scancode: 0x27, extended: false },
    Quote: { scancode: 0x28, extended: false },
    Backquote: { scancode: 0x29, extended: false },
    ShiftLeft: { scancode: 0x2a, extended: false },
    Backslash: { scancode: 0x2b, extended: false },
    KeyZ: { scancode: 0x2c, extended: false },
    KeyX: { scancode: 0x2d, extended: false },
    KeyC: { scancode: 0x2e, extended: false },
    KeyV: { scancode: 0x2f, extended: false },
    KeyB: { scancode: 0x30, extended: false },
    KeyN: { scancode: 0x31, extended: false },
    KeyM: { scancode: 0x32, extended: false },
    Comma: { scancode: 0x33, extended: false },
    Period: { scancode: 0x34, extended: false },
    Slash: { scancode: 0x35, extended: false },
    ShiftRight: { scancode: 0x36, extended: false },
    AltLeft: { scancode: 0x38, extended: false },
    Space: { scancode: 0x39, extended: false },
    CapsLock: { scancode: 0x3a, extended: false },
    F1: { scancode: 0x3b, extended: false },
    F2: { scancode: 0x3c, extended: false },
    F3: { scancode: 0x3d, extended: false },
    F4: { scancode: 0x3e, extended: false },
    F5: { scancode: 0x3f, extended: false },
    F6: { scancode: 0x40, extended: false },
    F7: { scancode: 0x41, extended: false },
    F8: { scancode: 0x42, extended: false },
    F9: { scancode: 0x43, extended: false },
    F10: { scancode: 0x44, extended: false },
    F11: { scancode: 0x57, extended: false },
    F12: { scancode: 0x58, extended: false },
    IntlBackslash: { scancode: 0x56, extended: false },
    // Touches étendues : préfixe 0xE0 côté matériel, indicateur `extended` ici.
    ControlRight: { scancode: 0x1d, extended: true },
    AltRight: { scancode: 0x38, extended: true },
    NumpadEnter: { scancode: 0x1c, extended: true },
    NumpadDivide: { scancode: 0x35, extended: true },
    Home: { scancode: 0x47, extended: true },
    ArrowUp: { scancode: 0x48, extended: true },
    PageUp: { scancode: 0x49, extended: true },
    ArrowLeft: { scancode: 0x4b, extended: true },
    ArrowRight: { scancode: 0x4d, extended: true },
    End: { scancode: 0x4f, extended: true },
    ArrowDown: { scancode: 0x50, extended: true },
    PageDown: { scancode: 0x51, extended: true },
    Insert: { scancode: 0x52, extended: true },
    Delete: { scancode: 0x53, extended: true },
    MetaLeft: { scancode: 0x5b, extended: true },
    MetaRight: { scancode: 0x5c, extended: true },
};
