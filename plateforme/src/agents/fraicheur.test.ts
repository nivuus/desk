// An agent's freshness: `prete` or `injoignable`, decided on `vu_a`.
//
// 🔴 THE CENTRAL RED OF THIS FILE IS THE TRANSITION, not the two states
// taken separately. Spec §4 P3 ④ writes it literally: "a threshold that
// is never reached in the test proves nothing: the test MUST see the
// transition". Two tests each setting their own instant and their own `vu_a`
// could both pass on an implementation that does not read the clock
// at all — it is the trap P2's criterion ② named for tokens, and
// that a FROZEN clock would replay here.
//
// ⚠️ THE CLOCK IS A PARAMETER, never read in the module: that is what makes
// the instants assertable on exact values, and it is the rule of the repository
// (`depot/session.ts`, `identite/jeton.ts`, `signaling/ice.ts`).
//
// ⚠️ THE INSTANTS ARE REAL EPOCHS IN MILLISECONDS, never small
// convenient numbers. It is the lesson of `base/harnais.ts`: a `1_000` fits
// in a 4-byte integer, `Date.now()` does not — and here it also matters
// that the test exercises the order of magnitude the service really handles.

import { describe, expect, it } from 'vitest';
import { SEUIL_INJOIGNABLE_MS, etatDe } from './fraicheur';

/// A real epoch: 19 August 2026, to the millisecond.
const T0 = 1_787_000_000_000;

describe('the freshness of an agent', () => {
    it('a VM that has NEVER beaten is unreachable', () => {
        // 🔴 `null` is not `0`: `depot/agent.ts` already says so of the column.
        // Returning `prete` here would announce ready a VM nobody has ever
        // had the slightest news of — exactly the opposite of what the column
        // means.
        expect(etatDe(null, T0)).toBe('injoignable');
    });

    it('a VM seen at this very instant is ready', () => {
        expect(etatDe(T0, T0)).toBe('prete');
    });

    it('🔴 a VM seen THRESHOLD + 1 ms ago is unreachable', () => {
        // Without comparison to the threshold, an implementation that returned `prete`
        // as soon as `vu_a` is not `null` would pass the two tests above.
        expect(etatDe(T0, T0 + SEUIL_INJOIGNABLE_MS + 1)).toBe('injoignable');
    });

    it('🔴 THE TRANSITION is observed: same `vu_a`, two instants, and the bound is besieged from BOTH sides', () => {
        // 🔴 It is the literal red of the spec's criterion ④. The SAME `vu_a`
        // is judged at two instants, and the two instants frame the bound to
        // one millisecond: a frozen — or ignored — clock would return
        // the same value twice and this test would fail.
        const vuA = T0;
        expect(etatDe(vuA, vuA + SEUIL_INJOIGNABLE_MS)).toBe('prete');
        expect(etatDe(vuA, vuA + SEUIL_INJOIGNABLE_MS + 1)).toBe('injoignable');
    });
});
