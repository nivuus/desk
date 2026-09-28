// The two operation allowlists, tested on their RUNTIME
// VALUES and not on their types alone.
//
// 🔴 WHY THE TWO GUARDS. `satisfies readonly Operation[]` is a
// COMPILE-time guard: it forbids writing a verb that does not exist. It
// does NOT forbid forgetting one. It is the union test below that
// forbids it, and it runs under `vitest` — the doctrine of
// `base/sous-ensemble.test.ts` ("each covers the other's blind spot")
// applied here for the second time.

import { describe, expect, it } from 'vitest';
import { etatDe } from '../agents/fraicheur';
import {
    OPERATIONS,
    OPERATIONS_HORS_HTTP,
    OPERATIONS_HTTP,
    type EtatVm,
} from './interface';

describe('the operation allow-lists', () => {
    it('🔴 their UNION, sorted, is exactly OPERATIONS sorted', () => {
        // 🔴 The red: removing `arreter` from both lists. The union stops
        // equalling `OPERATIONS`, and the test fails. It is the check that
        // forbids adding a verb while forgetting it: a verb added to
        // `OPERATIONS` without a place in one of the two lists no longer has a
        // home, and nobody would see it otherwise.
        const union = [...OPERATIONS_HTTP, ...OPERATIONS_HORS_HTTP].sort();
        expect(union).toEqual([...OPERATIONS].sort());
    });

    it('🔴 their INTERSECTION is empty', () => {
        // 🔴 The red: putting `etat` in both. `it()` DISTINCT from the
        // previous one: `expect` interrupts a test at its first false
        // assertion, and the union would stay right up to duplicates — P2's lesson
        // ①A/①A-bis, where a second assertion was tested by nothing.
        const http = new Set<string>(OPERATIONS_HTTP);
        const communes = OPERATIONS_HORS_HTTP.filter((o) => http.has(o));
        expect(communes).toEqual([]);
    });

    it('🔴 `attribuer` is NOT an HTTP operation', () => {
        // 🔴 The red: adding it to `OPERATIONS_HTTP`. Assigning a VM
        // would become reachable by any authenticated user — there
        // is no administration role in this service (D8), and the
        // route could therefore require nothing more than an ordinary token.
        expect((OPERATIONS_HTTP as readonly string[]).includes('attribuer')).toBe(false);
        expect((OPERATIONS_HORS_HTTP as readonly string[]).includes('attribuer')).toBe(true);
    });

    it('EtatVm is exactly what `etatDe` produces: `prete` and `injoignable`', () => {
        // 🔴 The red: replacing the re-export with the four-member union of
        // spec §3.6 (`arretee`, `demarrage`, `prete`, `injoignable`).
        //
        // ⚠️ AND IT IS DOUBLE, BECAUSE NEITHER HALF IS ENOUGH.
        // Under the mutation alone, `Record<EtatVm, number>` loses two keys:
        // `npm run typecheck` FAILS, `vitest` stays green (esbuild does not
        // typecheck). If the author of the mutation then adds the two
        // missing keys to silence `tsc`, it is `Object.keys`
        // below that fails under `vitest`. BOTH reds were played
        // and recorded; without the second, this test would be a check unable
        // to fail where it is read — see E2.
        const CODES: Record<EtatVm, number> = { prete: 0, injoignable: 1 };
        expect(Object.keys(CODES).sort()).toEqual(['injoignable', 'prete']);

        const MS = 1_787_136_773_742;
        const prete: EtatVm = etatDe(MS, MS);
        const injoignable: EtatVm = etatDe(null, MS);
        expect(prete).toBe('prete');
        expect(injoignable).toBe('injoignable');
    });
});
