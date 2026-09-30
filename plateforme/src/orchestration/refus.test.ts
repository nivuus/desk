// The refusal reasons, and the code table that cannot be incomplete.
//
// 🔴 TWO GUARDS, AND BOTH ARE WANTED. `Record<Motif,
// number>` catches at COMPILE time a reason added without an HTTP code; the
// first test below also catches it under `vitest`, for whoever reads the
// suite's green without reading that of `npm run typecheck`. And `tsc` would
// NOT see an EXTRA key set by an `as any` — the test would. It is the doctrine of
// `base/sous-ensemble.test.ts`: each covers the other's blind spot.

import { describe, expect, it } from 'vitest';
import { BACKEND_HOTE, BACKEND_STATIQUE, CODE_HTTP, MOTIFS, refuser, type Outcome } from './refus';

describe('the refusal reasons', () => {
    it('🔴 CODE_HTTP carries EXACTLY the reasons of MOTIFS, no more no less', () => {
        // 🔴 The red: removing an entry from `CODE_HTTP`. The test fails, AND
        // `tsc` fails — both were played. The symmetrical red, the one
        // `tsc` can NOT see, is an extra key set by a cast:
        // it was played too.
        expect(Object.keys(CODE_HTTP).sort()).toEqual([...MOTIFS].sort());
    });

    it('🔴 `non-supporte` is 501, never 500', () => {
        // 🔴 The red: setting 500. A 500 reads as a FAILURE of the service;
        // a backend that admits it does not know how is not failing, and
        // confusing the two would make one look for a defect where there is none.
        expect(CODE_HTTP['non-supporte']).toBe(501);
        // And the other codes, each named rather than deduced.
        expect(CODE_HTTP['vm-inconnue']).toBe(404);
        expect(CODE_HTTP['vm-deja-attribuee']).toBe(409);
        expect(CODE_HTTP['utilisateur-servi']).toBe(409);
        expect(CODE_HTTP['aucune-vm']).toBe(409);
        expect(CODE_HTTP['agent-injoignable']).toBe(503);
    });

    it('🔴 `refuser` says WHAT was refused, and BY WHOM', () => {
        // 🔴 The red: omitting `operation`. A refusal that does not say which
        // operation was refused does not inform — and the day a second
        // backend exists, a refusal without `backend` would not say who refuses.
        expect(refuser('non-supporte', 'instantane')).toEqual({
            ok: false,
            motif: 'non-supporte',
            operation: 'instantane',
            backend: BACKEND_STATIQUE,
        });
        expect(BACKEND_STATIQUE).toBe('inventaire-statique');
    });

    it('🔴 a refusal is NEVER a success, and a success carries NO reason', () => {
        // 🔴 The red: making `refuser` return `{ ok: true, motif }`. The type
        // forbids it; this test says so to whoever reads the runtime code. A refusal
        // that read `ok:true` would be the exact silent failure spec
        // §3.6 names — "a `Promise<void>` that does nothing".
        //
        // ⚠️ `it()` DISTINCT from the previous ones: `expect` interrupts a test at its
        // first false assertion (P2's lesson ①A/①A-bis).
        const succes: Outcome = { ok: true };
        expect('motif' in succes).toBe(false);
        for (const motif of MOTIFS) {
            const r = refuser(motif, 'demarrer');
            expect(r.ok).toBe(false);
            // The discriminant does its job: outside the branch, `motif`
            // is not even readable.
            if (r.ok) throw new Error('a refusal declared itself a success');
            expect(r.motif).toBe(motif);
        }
    });
});

describe('the host backend vocabulary', () => {
    it("'hote-inaccessible' is a reason, rendered as 503", () => {
        expect(MOTIFS).toContain('hote-inaccessible');
        expect(CODE_HTTP['hote-inaccessible']).toBe(503);
    });

    it('a host refusal carries the `hote` backend', () => {
        expect(refuser('hote-inaccessible', 'demarrer', BACKEND_HOTE)).toEqual({
            ok: false,
            motif: 'hote-inaccessible',
            operation: 'demarrer',
            backend: 'hote',
        });
    });
});
