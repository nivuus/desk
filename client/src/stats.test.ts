import { describe, expect, it } from 'vitest';

import { suivreMontant } from './stats';

describe('suivreMontant — the mic measurements (work item E)', () => {
    it("tells « no upstream track » from an upstream track at zero", () => {
        // ⚠️ THE DISTINCTION IS THE CRUX OF THE TEST, not a display
        // nicety. `stats.ts` already makes it for DOWNSTREAM audio, and the
        // comment that goes with it says why: without it, a session
        // without a negotiated microphone reads exactly like a microphone whose bitrate is
        // simply zero, and a negotiation defect becomes undiagnosable.
        const absente = suivreMontant(undefined, undefined);
        expect(absente.ligne).toBe('mic absent');

        const posee = { octets: 0, paquets: 0, horodatage: 1000 };
        const zero = suivreMontant(posee, { octets: 0, paquets: 0, horodatage: 2000 });
        expect(zero.ligne).not.toBe('mic absent');
        expect(zero.ligne).toMatch(/^mic 0 kb\/s/);
    });

    it('computes the bitrate on the delta, not on the total', () => {
        const before = { octets: 1_000, paquets: 10, horodatage: 1_000 };
        const apres = { octets: 3_000, paquets: 60, horodatage: 2_000 };

        // 2,000 bytes in 1 s = 16,000 bits/s = 16 kb/s. Displaying the TOTAL
        // would give 24 kb/s here, which would go unnoticed on a first
        // read and would grow without end.
        const { ligne } = suivreMontant(before, apres);
        expect(ligne).toMatch(/^mic 16 kb\/s/);
        expect(ligne).toMatch(/packets 60/);
    });

    it('a first reading claims no bitrate', () => {
        const { ligne, memoire } = suivreMontant(undefined, {
            octets: 5_000,
            paquets: 40,
            horodatage: 1_000,
        });
        expect(ligne).toMatch(/^mic 0 kb\/s/);
        expect(memoire).toEqual({ octets: 5_000, paquets: 40, horodatage: 1_000 });
    });

    it('the disappearance of the track FORGETS the previous snapshot', () => {
        // Same defect as the one already fixed on downstream audio: without
        // forgetting, a track that disappears then comes back (SSRC renegotiated,
        // counters restarted from zero) would compute its first bitrate against the
        // counters of ANOTHER stream — an absurd, even negative, delta.
        const { memoire } = suivreMontant({ octets: 9_000, paquets: 90, horodatage: 1_000 }, undefined);
        expect(memoire).toBeUndefined();
    });

    it('a timestamp that does not advance does not return an infinite bitrate', () => {
        const meme = { octets: 1_000, paquets: 10, horodatage: 5_000 };
        const { ligne } = suivreMontant(meme, { octets: 4_000, paquets: 40, horodatage: 5_000 });
        expect(ligne).toMatch(/^mic 0 kb\/s/);
    });
});
