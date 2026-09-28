import { describe, expect, it } from 'vitest';
import { RejeuResize } from './resize';

describe('RejeuResize', () => {
    it("returns the observed size when nothing was emitted yet", () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        expect(r.aEmettre()).toEqual({ largeur: 1280, hauteur: 720 });
    });

    it('returns nothing when the emitted size is already the right one', () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        r.confirmer({ largeur: 1280, hauteur: 720 });
        expect(r.aEmettre()).toBeUndefined();
    });

    it("returns the observed size as long as no emission was confirmed", () => {
        // ⚠️ This test exercises NO channel state: `RejeuResize` is pure and
        // knows none (D9's legacy item no. 11 — its title announced "when the
        // channel was closed at the moment of the gesture", a state it could not
        // reach, and it returned the same verdict for any other
        // reason of non-confirmation). The channel state lives in `main.ts`,
        // in `emettreSiPossible`, and that is where it is tested — not here.
        //
        // MOTIVATION of the mechanism, and not what this test exercises: the case of
        // legacy item 10 is the one where the ResizeObserver saw the size but
        // `readyState !== 'open'` made the send give up. When the channel
        // opens, the size must go out again — otherwise it is lost forever,
        // the observer only firing again on a NEW change.
        const r = new RejeuResize();
        r.observer({ largeur: 1920, hauteur: 1080 });
        // no `confirmer`: the send has not happened
        expect(r.aEmettre()).toEqual({ largeur: 1920, hauteur: 1080 });
    });

    it('returns the LAST observed size, not the first', () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        r.observer({ largeur: 1920, hauteur: 1080 });
        expect(r.aEmettre()).toEqual({ largeur: 1920, hauteur: 1080 });
    });

    it("returns nothing as long as nothing was observed", () => {
        expect(new RejeuResize().aEmettre()).toBeUndefined();
    });
});
