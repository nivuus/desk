import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';

import { attacherResizeAuDOM } from './resize-dom';

// 🔴 **WHAT THIS FILE GUARDS, AND WHY IT EXISTS (batch 33).**
//
// The remedy of batch 33 lives in TWO processes of the agent: the SENSOR makes
// the crop and the encoder follow the `Resize` of the control channel, the
// SUPERVISOR makes the Windows window follow the `viewport` of the `postMessage`.
// Both apply the SAME pure rule on the SAME bound — so they only
// fight if they are given two different NUMBERS.
//
// **It is this file that holds the invariant "a single number, two
// recipients".** No Rust test can see it: the divergence would arise
// in the browser, and the two halves would be correct taken
// separately — exactly the class of defect `CLAUDE.md` describes under
// "a per-task review cannot see a defect that crosses a task
// boundary".

// ⚠️ **`client/` has NEITHER jsdom NOR happy-dom**, and it is a property of the repository,
// not a gap: its DOM tests inject their dependencies
// (`accent-dom.test.ts` says so in so many words). `resize-dom.ts`, for its part,
// reaches `window` directly — for its timers and its
// `devicePixelRatio` —, so one has to be set for it. Minimal, and delegating to
// `globalThis` so that Vitest's fake timers still patch it.
function poserUnWindow(dpr: number): void {
    (globalThis as unknown as { window: unknown }).window = {
        devicePixelRatio: dpr,
        setTimeout: (...a: Parameters<typeof setTimeout>) => setTimeout(...a),
        clearTimeout: (h: number) => clearTimeout(h),
        innerWidth: 0,
        innerHeight: 0,
    };
}

class ResizeObserverFactice {
    static dernier: ResizeObserverFactice | undefined;
    declencher: () => void;
    constructor(rappel: () => void) {
        this.declencher = rappel;
        ResizeObserverFactice.dernier = this;
    }
    observe(): void {}
    disconnect(): void {}
}

function monter(largeurCss: number, hauteurCss: number, dpr: number) {
    const envoyes: string[] = [];
    const canal = {
        readyState: 'open',
        send: (o: string) => envoyes.push(o),
        addEventListener: () => {},
    };
    const video = { clientWidth: largeurCss, clientHeight: hauteurCss } as HTMLVideoElement;
    poserUnWindow(dpr);
    const annonces: Array<[number, number]> = [];
    attacherResizeAuDOM(video, { controlChannel: canal } as never, (l, h) =>
        annonces.push([l, h]),
    );
    return { envoyes, annonces };
}

describe('attacherResizeAuDOM — the viewport and the Resize come from the SAME measurement', () => {
    beforeEach(() => {
        vi.useFakeTimers();
        (globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver =
            ResizeObserverFactice;
    });
    afterEach(() => vi.useRealTimers());

    it('emits a viewport for each Resize, with exactly the same numbers', () => {
        const { envoyes, annonces } = monter(778, 491, 1);
        ResizeObserverFactice.dernier!.declencher();
        vi.advanceTimersByTime(250);

        expect(envoyes).toHaveLength(1);
        // `encodeResize` returns JSON: we re-read the message ACTUALLY emitted
        // rather than redoing the computation, otherwise the test would compare its
        // own arithmetic to itself.
        const decode = JSON.parse(envoyes[0]!) as {
            type: string;
            width: number;
            height: number;
        };
        expect(decode.type).toBe('resize');
        // 🔴 THE ASSERTION THAT COUNTS: the viewport announced to the supervisor must
        // carry THE SAME NUMBERS as the `Resize` sent to the sensor. Comparing
        // them to each other would NOT be circular here — they are two distinct
        // code paths (`encodeResize` and the callback), and it is
        // precisely their EQUALITY that is the property, not their value.
        expect(annonces).toEqual([[decode.width, decode.height]]);
    });

    it("announces nothing when the page has no opener (callback absent)", () => {
        // The "page opened by hand" case: `main.ts` then passes no
        // announcer. The `Resize` must go out anyway — the session is alive.
        const envoyes: string[] = [];
        const canal = {
            readyState: 'open',
            send: (o: string) => envoyes.push(o),
            addEventListener: () => {},
        };
        const video = { clientWidth: 800, clientHeight: 600 } as HTMLVideoElement;
        poserUnWindow(1);
        attacherResizeAuDOM(video, { controlChannel: canal } as never);
        ResizeObserverFactice.dernier!.declencher();
        vi.advanceTimersByTime(250);
        expect(envoyes).toHaveLength(1);
    });

    it('multiplies by devicePixelRatio only ONCE', () => {
        // ⚠️ The announcer of `main.ts` does not multiply again: if this module and it
        // both applied the factor, a HiDPI client would request
        // FOUR times the pixels. The announced number must be the one, already
        // in device pixels, that the `Resize` carries.
        const { annonces } = monter(800, 600, 2);
        ResizeObserverFactice.dernier!.declencher();
        vi.advanceTimersByTime(250);
        expect(annonces).toEqual([[1600, 1200]]);
    });
});
