// Tests of the keyboard exception of sub-block P2 — and of it alone.
//
// ⚠️ **This file does NOT cover the pointer, the wheel or the context
// menu**: those paths touch `document.pointerLockElement`,
// `getBoundingClientRect` and `setPointerCapture`, and the client suite runs
// in a `node` environment, without a DOM. They had no coverage before P2 and
// still have none — declared, not hidden.
//
// The keyboard, on the other hand, is covered because P2 **injected its target**
// (`CibleClavier`) instead of taking it from the global: that is what makes the
// LINK between `raccourcis.ts` and the listener testable, and it is the link
// that carries risk R5 of the spec.

import { beforeEach, describe, expect, it, vi } from 'vitest';

import { attachInput, type CibleClavier } from './input';

/// Fake keyboard target: keeps the callbacks and replays them on demand.
class ClavierFactice implements CibleClavier {
    private rappels = new Map<string, (event: KeyboardEvent) => void>();

    addEventListener(nom: 'keydown' | 'keyup', rappel: (event: KeyboardEvent) => void): void {
        this.rappels.set(nom, rappel);
    }
    removeEventListener(nom: 'keydown' | 'keyup', rappel: (event: KeyboardEvent) => void): void {
        if (this.rappels.get(nom) === rappel) this.rappels.delete(nom);
    }
    /// Replays an event, and returns the `preventDefault` spy.
    frapper(nom: 'keydown' | 'keyup', touche: Partial<KeyboardEvent> & { code: string }) {
        const preventDefault = vi.fn();
        const event = {
            ctrlKey: false,
            shiftKey: false,
            altKey: false,
            metaKey: false,
            ...touche,
            preventDefault,
        } as unknown as KeyboardEvent;
        this.rappels.get(nom)?.(event);
        return preventDefault;
    }
    get attaches(): number {
        return this.rappels.size;
    }
}

let clavier: ClavierFactice;
let envois: Uint8Array[];
let arme: boolean;
let detacher: () => void;

function monter(): void {
    clavier = new ClavierFactice();
    envois = [];
    arme = true;
    const channel = {
        readyState: 'open',
        send: (p: Uint8Array) => envois.push(p),
    } as unknown as RTCDataChannel;
    // None of these members is touched by the keyboard paths: the only
    // use of the `video` is the pointer, which this file does not exercise.
    const video = { addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as
        HTMLVideoElement;
    detacher = attachInput({ video, channel, clavier, collageArme: () => arme });
}

beforeEach(monter);

describe("the narrow paste exception", () => {
    // 🔴 **THE CENTRAL RED OF P2.** Until commit `4cf2206`, `onKeyDown`
    // called `preventDefault()` UNCONDITIONALLY: no `paste` event could
    // arise in the session window. §0 of the plan establishes by
    // measurement (two runs) that removing this `preventDefault` on `KeyV`
    // alone is enough to give rise to a trusted `paste` on a focused
    // `<video>` — and this test is what guards the property from now on.
    it('Ctrl+V: neither preventDefault, nor byte sent', () => {
        const pd = clavier.frapper('keydown', { ctrlKey: true, code: 'KeyV' });
        expect(pd).not.toHaveBeenCalled();
        expect(envois).toHaveLength(0);
    });

    // 🔴 **THIS IS RISK R5.** A widened condition would hand the keyboard back to the browser,
    // and `Ctrl+W` would close the session window.
    it.each(['KeyW', 'KeyT', 'KeyN'])('Ctrl+%s: preventDefault AND byte sent', (code) => {
        const pd = clavier.frapper('keydown', { ctrlKey: true, code });
        expect(pd).toHaveBeenCalledOnce();
        expect(envois).toHaveLength(1);
    });

    // The modifier itself goes out normally: holding it back would make
    // the VM lose a `Ctrl` the user may be holding for something else. The
    // probe establishes that its `preventDefault` does NOT prevent the `paste`.
    it('ControlLeft alone: preventDefault AND byte sent', () => {
        const pd = clavier.frapper('keydown', { ctrlKey: true, code: 'ControlLeft' });
        expect(pd).toHaveBeenCalledOnce();
        expect(envois).toHaveLength(1);
    });

    // 🔴 The release is held back WITH its `preventDefault`. Letting only
    // `V`↑ go out would make the VM see a release without a press — which
    // can unblock a keyboard repeat —, and it would arrive AFTER the
    // four keys the agent injects.
    it('keyup of V under Ctrl: preventDefault, but no byte', () => {
        const pd = clavier.frapper('keyup', { ctrlKey: true, code: 'KeyV' });
        expect(pd).toHaveBeenCalledOnce();
        expect(envois).toHaveLength(0);
    });

    it('Shift+Insert is handled like Ctrl+V', () => {
        const pd = clavier.frapper('keydown', { shiftKey: true, code: 'Insert' });
        expect(pd).not.toHaveBeenCalled();
        expect(envois).toHaveLength(0);
    });
});

describe('the gate on Capabilities.clipboard', () => {
    // 🔴 **WITHOUT THIS GATE, `PRESSE_PAPIER=0` WOULD GIVE THE WORST OF BOTH WORLDS**:
    // the client would hold back the `Ctrl+V` while nobody would inject it
    // on the VM side. The key would be lost, and the user would see a dead
    // shortcut.
    it("disarmed, Ctrl+V gets back its pre-P2 behaviour", () => {
        arme = false;
        const pd = clavier.frapper('keydown', { ctrlKey: true, code: 'KeyV' });
        expect(pd).toHaveBeenCalledOnce();
        expect(envois).toHaveLength(1);
    });

    // 🔴 **A CLOSURE, NOT A BOOLEAN CAPTURED AT ATTACH TIME.** `Capabilities`
    // can arrive after `attachInput`; a frozen boolean would be `false`
    // forever, and pasting would be dead without any trace saying so.
    //
    // RED if `collageArme` were read only once, at mount.
    it("the arming is re-read on EACH keystroke, not captured on mount", () => {
        arme = false;
        expect(clavier.frapper('keydown', { ctrlKey: true, code: 'KeyV' })).toHaveBeenCalledOnce();
        arme = true;
        expect(clavier.frapper('keydown', { ctrlKey: true, code: 'KeyV' })).not.toHaveBeenCalled();
    });
});

describe('detaching', () => {
    // RED if `detacher` removed the listeners from `window` whereas they were
    // set on the injected target: they would survive the end of the session.
    it('removes both listeners from the injected target', () => {
        expect(clavier.attaches).toBe(2);
        detacher();
        expect(clavier.attaches).toBe(0);
    });
});
