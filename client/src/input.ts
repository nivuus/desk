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
import { SCANCODES } from './scancodes';

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
