import { describe, expect, it } from 'vitest';
import { sessionIdFromParams } from './session-id';

describe('sessionIdDepuisParametres', () => {
    it("returns the identifier carried by the `session` parameter", () => {
        expect(sessionIdFromParams(new URLSearchParams('session=abc'))).toBe('abc');
    });

    // 🔴 THE RED OF THE FIX OF AUGUST 30TH, 2026. Before the extraction,
    // `main.ts` carried `params.get('session') ?? 'demo'`: this test goes red
    // if that form ever comes back, under whatever name — an
    // absent parameter must return `undefined`, never an invented
    // value. Checked red by temporarily substituting the body of
    // `sessionIdFromParams` with the old line (see the report of
    // this batch for the real output): `expect(undefined).toBe('demo')`
    // fails as expected.
    it("does NOT invent a 'demo' session when the parameter is absent", () => {
        expect(sessionIdFromParams(new URLSearchParams())).toBeUndefined();
    });
});
