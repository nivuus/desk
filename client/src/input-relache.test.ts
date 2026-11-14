// Held keys and buttons are released when the page loses them: the agent
// injects what arrives and never releases on its own, so a key held when the
// window loses focus would stay pressed on the remote machine.
//
// Also: a mouse button the protocol does not map is not injected at all.

import { beforeEach, describe, expect, it, vi } from 'vitest';

import { encodeKey, encodeMouseButton } from '../../proto/ts/input';
import { attachInput, type CibleClavier, type CiblePage } from './input';
import { SCANCODES } from './scancodes';

type Rappel = (event: any) => void;

/// An event target that keeps its callbacks and replays them on demand.
class Cible {
    private rappels = new Map<string, Set<Rappel>>();
    addEventListener(nom: string, rappel: Rappel): void {
        if (!this.rappels.has(nom)) this.rappels.set(nom, new Set());
        this.rappels.get(nom)!.add(rappel);
    }
    removeEventListener(nom: string, rappel: Rappel): void {
        this.rappels.get(nom)?.delete(rappel);
    }
    emettre(nom: string, event: Record<string, unknown> = {}): { preventDefault: ReturnType<typeof vi.fn> } {
        const preventDefault = vi.fn();
        const complet = { preventDefault, ...event };
        for (const rappel of [...(this.rappels.get(nom) ?? [])]) rappel(complet);
        return { preventDefault };
    }
}

class VideoFactice extends Cible {
    videoWidth = 0;
    videoHeight = 0;
    setPointerCapture = vi.fn();
    getBoundingClientRect() {
        return { x: 0, y: 0, width: 65535, height: 65535 };
    }
}

class PageFactice extends Cible implements CiblePage {
    hidden = false;
}

let video: VideoFactice;
let clavier: Cible;
let page: PageFactice;
let envois: Uint8Array[];

beforeEach(() => {
    video = new VideoFactice();
    clavier = new Cible();
    page = new PageFactice();
    envois = [];
    const channel = { readyState: 'open', send: (p: Uint8Array) => envois.push(p) } as unknown as RTCDataChannel;
    attachInput({
        video: video as unknown as HTMLVideoElement,
        channel,
        clavier: clavier as unknown as CibleClavier,
        page,
        collageArme: () => false,
    });
});

const A = SCANCODES['KeyA'];
const FLECHE = SCANCODES['ArrowLeft'];

function toucher(nom: 'keydown' | 'keyup', code: string): void {
    clavier.emettre(nom, { code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false });
}

function presser(button: number, x = 100, y = 200): void {
    video.emettre('pointerdown', { button, pointerId: 1, clientX: x, clientY: y });
}

describe('releasing what is held when the page loses it', () => {
    it('key down then blur sends a key-up for that scancode', () => {
        toucher('keydown', 'KeyA');
        envois = [];
        page.emettre('blur');
        expect(envois).toEqual([encodeKey(A.scancode, false, A.extended)]);
    });

    it('an extended key is released as extended', () => {
        toucher('keydown', 'ArrowLeft');
        envois = [];
        page.emettre('blur');
        expect(envois).toEqual([encodeKey(FLECHE.scancode, false, FLECHE.extended)]);
    });

    it('the page becoming hidden releases the held keys', () => {
        toucher('keydown', 'KeyA');
        envois = [];
        page.hidden = true;
        page.emettre('visibilitychange');
        expect(envois).toEqual([encodeKey(A.scancode, false, A.extended)]);
    });

    it('the page becoming VISIBLE releases nothing', () => {
        toucher('keydown', 'KeyA');
        envois = [];
        page.hidden = false;
        page.emettre('visibilitychange');
        expect(envois).toEqual([]);
    });

    it('a key released normally is not released again', () => {
        toucher('keydown', 'KeyA');
        toucher('keyup', 'KeyA');
        envois = [];
        page.emettre('blur');
        expect(envois).toEqual([]);
    });

    it('button down then pointer cancel sends a button-up for that button', () => {
        presser(0, 100, 200);
        envois = [];
        video.emettre('pointercancel', { button: -1, pointerId: 1, clientX: 100, clientY: 200 });
        expect(envois).toEqual([encodeMouseButton(0, false, 100, 200)]);
    });

    it('button down then lost pointer capture sends a button-up', () => {
        presser(2, 10, 20);
        envois = [];
        video.emettre('lostpointercapture', { pointerId: 1, clientX: 10, clientY: 20 });
        // Button 2 of the browser is the RIGHT button: protocol code 1.
        expect(envois).toEqual([encodeMouseButton(1, false, 10, 20)]);
    });

    it('button down then blur sends a button-up at the last known position', () => {
        presser(0, 30, 40);
        envois = [];
        page.emettre('blur');
        expect(envois).toEqual([encodeMouseButton(0, false, 30, 40)]);
    });

    it('the capture lost after a normal button-up releases nothing more', () => {
        presser(0);
        video.emettre('pointerup', { button: 0, pointerId: 1, clientX: 100, clientY: 200 });
        envois = [];
        video.emettre('lostpointercapture', { pointerId: 1, clientX: 100, clientY: 200 });
        expect(envois).toEqual([]);
    });
});
