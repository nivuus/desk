// Input capture and sending on the binary channel.
//
// Coordinates are normalised over 0..65535 relative to the image area
// actually displayed: `object-fit: contain` leaves black bars that
// must be excluded, otherwise the pointer drifts.

import {
    encodeKey,
    encodeMouseButton,
    encodeMouseMove,
    encodeWheel,
    type MouseButtonCode,
} from '../../proto/ts/input';
import { estUnRaccourciDeCollage } from './raccourcis';
import { SCANCODES } from './scancodes';

/// What this module needs from the window: the two keyboard events, and
/// nothing else. `window` conforms to it.
///
/// **Injected rather than taken from the global**, like `CibleFocus`
/// (`presse-papier-dom.ts`) and `CibleEcran` (`fullscreen.ts`). Without that
/// the paste exception below would be covered by NOTHING: the client's
/// test suite runs in a `node` environment, without a DOM, and a
/// `window.addEventListener` throws there. The predicate would indeed be tested, its BINDING
/// to the listener would not — and it is the binding that carries risk R5.
export interface CibleClavier {
    addEventListener(nom: 'keydown' | 'keyup', rappel: (event: KeyboardEvent) => void): void;
    removeEventListener(nom: 'keydown' | 'keyup', rappel: (event: KeyboardEvent) => void): void;
}

export interface InputOptions {
    video: HTMLVideoElement;
    channel: RTCDataChannel;
    /// The source of keyboard events — `window`, in production.
    clavier: CibleClavier;
    /// Has the agent announced `Capabilities.clipboard`?
    ///
    /// 🔴 **A CLOSURE, never a boolean captured at attach time**, and it
    /// matters: `Capabilities` arrives BEFORE `Ready` but nothing guarantees
    /// it precedes `attachInput`. A boolean frozen at attach time
    /// would be `false` forever if the message arrived a millisecond
    /// later, and paste would be dead without any trace saying so.
    ///
    /// **Without this gate, `PRESSE_PAPIER=0` would produce the WORST OF BOTH WORLDS**:
    /// the client would hold back `Ctrl+V` — it would no longer send it on the
    /// input channel — while no one would inject it on the VM side. The key
    /// would be lost, and the user would see a dead shortcut.
    collageArme: () => boolean;
}

/** Area occupied by the image in the video element, black bars excluded. */
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

export function attachInput({
    video,
    channel,
    clavier,
    collageArme,
}: InputOptions): () => void {
    const send = (payload: Uint8Array): void => {
        // Assertion needed since TypeScript 5.7: `Uint8Array` is
        // now generic over its underlying buffer, by default
        // `ArrayBufferLike` (which includes `SharedArrayBuffer`), whereas
        // `RTCDataChannel.send` specifically requires `ArrayBuffer`. The
        // buffers produced by `proto/ts/input.ts` (`new Uint8Array(n)`)
        // are always backed by a real `ArrayBuffer` in practice — only
        // the typing is too broad.
        if (channel.readyState === 'open') channel.send(payload as Uint8Array<ArrayBuffer>);
    };

    const onPointerMove = (event: PointerEvent): void => {
        // Under Pointer Lock, `pointer.ts` emits the relative deltas: emitting
        // AN absolute position AS WELL would teleport the Windows cursor between
        // two relative moves.
        if (document.pointerLockElement === video) return;
        // getCoalescedEvents restores the intermediate positions the
        // browser grouped: the trace stays faithful at high frequency.
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
        // Windows counts 120 units per notch; deltaMode 0 is in pixels.
        const factor = event.deltaMode === 0 ? -120 / 100 : -120;
        send(encodeWheel(event.deltaX * -factor, event.deltaY * factor));
    };

    const onContextMenu = (event: Event): void => event.preventDefault();

    const onKeyDown = (event: KeyboardEvent): void => {
        // 🔴 **THE NARROW EXCEPTION OF SUB-BLOCK P2, AND THE ONLY ONE OF THE WHOLE
        // WORKSTREAM.** Without `preventDefault`, the browser produces a TRUSTED
        // `paste` event, which `presse-papier-dom` catches — it is the only
        // way to read the user's clipboard without asking them for
        // any permission.
        //
        // **Measured, two runs**, focus on the `<video>`:
        // `docs/superpowers/plans/journaux-presse-papier-p2/p2-paste-video-*.json`.
        // The probe's "product" regime — this unconditional `preventDefault` —
        // returns ZERO `paste` on its eight cells; the
        // "narrow" regime returns one, `isTrusted: true`, `types: ["text/plain"]`,
        // `e.target` = `VIDEO#remote`.
        //
        // **The scancode does NOT go out on the input channel** (D6 point 2): it is
        // not the browser's keystroke that pastes, it is the agent that
        // injects the four keys itself, AFTER writing the VM's
        // clipboard. `ControlLeft`, for its part, goes out normally — holding
        // it back would make the VM lose a modifier the user may be holding
        // for something else, and the probe establishes that its
        // `preventDefault` does NOT prevent the `paste` from arriving.
        if (collageArme() && estUnRaccourciDeCollage(event)) return;
        event.preventDefault();
        const mapped = SCANCODES[event.code];
        if (mapped) send(encodeKey(mapped.scancode, true, mapped.extended));
    };

    const onKeyUp = (event: KeyboardEvent): void => {
        event.preventDefault();
        // The release is held back too — but WITH its
        // `preventDefault`: the `paste` was already born from the `keydown`, and there is
        // no reason to give that release back to the browser.
        //
        // Letting the lone `V`↑ go would show the VM a release without a
        // matching press, which can unblock a keyboard
        // repeat — and it would moreover arrive AFTER the four keys
        // the agent injects, hence at the worst moment.
        if (collageArme() && estUnRaccourciDeCollage(event)) return;
        const mapped = SCANCODES[event.code];
        if (mapped) send(encodeKey(mapped.scancode, false, mapped.extended));
    };

    video.addEventListener('pointermove', onPointerMove);
    video.addEventListener('pointerdown', onPointerDown);
    video.addEventListener('pointerup', onPointerUp);
    video.addEventListener('wheel', onWheel, { passive: false });
    video.addEventListener('contextmenu', onContextMenu);
    clavier.addEventListener('keydown', onKeyDown);
    clavier.addEventListener('keyup', onKeyUp);

    return () => {
        video.removeEventListener('pointermove', onPointerMove);
        video.removeEventListener('pointerdown', onPointerDown);
        video.removeEventListener('pointerup', onPointerUp);
        video.removeEventListener('wheel', onWheel);
        video.removeEventListener('contextmenu', onContextMenu);
        clavier.removeEventListener('keydown', onKeyDown);
        clavier.removeEventListener('keyup', onKeyUp);
    };
}
