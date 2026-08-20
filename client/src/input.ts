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
import { estUnRaccourciDeCollage } from './raccourcis';
import { SCANCODES } from './scancodes';

/// Ce dont ce module a besoin de la fenêtre : les deux événements clavier, et
/// rien d'autre. `window` s'y conforme.
///
/// **Injectée plutôt que prise du global**, comme `CibleFocus`
/// (`presse-papier-dom.ts`) et `CibleEcran` (`fullscreen.ts`). Sans cela
/// l'exception de collage ci-dessous ne serait couverte par RIEN : la suite de
/// tests du client tourne en environnement `node`, sans DOM, et un
/// `window.addEventListener` y lève. Le prédicat serait bien testé, sa LIAISON
/// à l'écouteur ne le serait pas — et c'est la liaison qui porte le risque R5.
export interface CibleClavier {
    addEventListener(nom: 'keydown' | 'keyup', rappel: (event: KeyboardEvent) => void): void;
    removeEventListener(nom: 'keydown' | 'keyup', rappel: (event: KeyboardEvent) => void): void;
}

export interface InputOptions {
    video: HTMLVideoElement;
    channel: RTCDataChannel;
    /// La source des événements clavier — `window`, en production.
    clavier: CibleClavier;
    /// L'agent a-t-il annoncé `Capabilities.clipboard` ?
    ///
    /// 🔴 **Une FERMETURE, jamais un booléen capturé à l'attache**, et c'est
    /// portant : `Capabilities` arrive AVANT `Ready` mais rien ne garantit
    /// qu'il précède `attachInput`. Un booléen figé au moment de l'attache
    /// vaudrait `false` à jamais si le message arrivait une milliseconde plus
    /// tard, et le collage serait mort sans qu'aucune trace ne le dise.
    ///
    /// **Sans ce gate, `PRESSE_PAPIER=0` produirait le PIRE DES DEUX MONDES** :
    /// le client retiendrait le `Ctrl+V` — il ne l'enverrait plus sur le canal
    /// d'entrées — alors que personne ne l'injecterait côté VM. La touche
    /// serait perdue, et l'utilisateur verrait un raccourci mort.
    collageArme: () => boolean;
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

export function attachInput({
    video,
    channel,
    clavier,
    collageArme,
}: InputOptions): () => void {
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
        // Sous Pointer Lock, `pointer.ts` émet les deltas relatifs : émettre
        // AUSSI une position absolue téléporterait le curseur Windows entre
        // deux déplacements relatifs.
        if (document.pointerLockElement === video) return;
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
        // 🔴 **L'EXCEPTION ÉTROITE DU SOUS-BLOC P2, ET LA SEULE DE TOUT LE
        // CHANTIER.** Sans `preventDefault`, le navigateur produit un événement
        // `paste` DE CONFIANCE, que `presse-papier-dom` capte — c'est la seule
        // façon de lire le presse-papier de l'utilisateur sans lui demander
        // aucune permission.
        //
        // **Mesuré, deux exécutions**, focus sur le `<video>` :
        // `docs/superpowers/plans/journaux-presse-papier-p2/p2-paste-video-*.json`.
        // Le régime « produit » de la sonde — ce `preventDefault` sans
        // condition — rend ZÉRO `paste` sur ses huit cellules ; le régime
        // « étroit » en rend un, `isTrusted: true`, `types: ["text/plain"]`,
        // `e.target` = `VIDEO#remote`.
        //
        // **Le scancode ne part PAS sur le canal d'entrées** (D6 point 2) : ce
        // n'est pas la frappe du navigateur qui colle, c'est l'agent qui
        // injecte les quatre touches lui-même, APRÈS avoir écrit le
        // presse-papier de la VM. `ControlLeft`, lui, part normalement — le
        // retenir ferait perdre à la VM un modificateur que l'utilisateur tient
        // peut-être pour autre chose, et la sonde établit que son
        // `preventDefault` n'empêche PAS le `paste` d'arriver.
        if (collageArme() && estUnRaccourciDeCollage(event)) return;
        event.preventDefault();
        const mapped = SCANCODES[event.code];
        if (mapped) send(encodeKey(mapped.scancode, true, mapped.extended));
    };

    const onKeyUp = (event: KeyboardEvent): void => {
        event.preventDefault();
        // Le relâchement est retenu lui aussi — mais AVEC son
        // `preventDefault` : le `paste` est déjà né du `keydown`, et il n'y a
        // aucune raison de rendre ce relâchement-là au navigateur.
        //
        // Laisser partir le seul `V`↑ ferait voir à la VM un relâchement sans
        // enfoncement correspondant, ce qui peut débloquer une répétition
        // clavier — et il arriverait de surcroît APRÈS les quatre touches que
        // l'agent injecte, donc au pire moment.
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
