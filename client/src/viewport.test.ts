import { describe, expect, it } from 'vitest';
import { viewportPair } from './viewport';

describe('viewportPair', () => {
    it('leaves already even dimensions intact', () => {
        expect(viewportPair(1280, 720)).toEqual({ largeur: 1280, hauteur: 720 });
    });

    // The mundane case: a browser pop-up commonly announces an odd
    // height. 1280×713 was measured in acceptance D1, and made opening
    // the window impossible — the agent aligns its regions to even values for NV12,
    // so the requested size could never equal the one obtained.
    it('rounds odd dimensions down', () => {
        expect(viewportPair(1281, 713)).toEqual({ largeur: 1280, hauteur: 712 });
    });

    it('rounds the fractions before making it even', () => {
        expect(viewportPair(1280.6, 713.4)).toEqual({ largeur: 1280, hauteur: 712 });
    });

    // A virtual output with a zero dimension cannot be captured: better
    // a floor than a 0×0 monitor whose defect would show much
    // further away, on the swap chain of a test pattern or on the scale factor.
    it('never goes under a floor of two pixels', () => {
        expect(viewportPair(1, 0)).toEqual({ largeur: 2, hauteur: 2 });
    });
});
