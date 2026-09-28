import { describe, expect, it } from 'vitest';
import { plan, verdict, type Tranche } from './tranches';

/**
 * ⚠️ WHAT JUDGES THIS FILE IS A NAMED MUTATION: replacing `Math.ceil` with
 * `Math.floor` in `plan`. The last chunk disappears, and `verdict`
 * then declares `complet` a TRUNCATED file. The tests that kill it carry the
 * 🔴 marker in their title — they are the ones that tie the split to the
 * `size` through a path INDEPENDENT of `plan`: counting chunks or
 * summing their bytes against the expected number, never against what `plan`
 * has just returned. A test that compared `plan` with itself would stay green
 * under the mutation.
 */

/** Sum of the bytes of a split — the central invariant, computed separately. */
function somme(tranches: Tranche[]): number {
    return tranches.reduce((total, t) => total + t.octets, 0);
}

describe('plan', () => {
    it('returns ZERO chunks for an empty file, never an empty chunk', () => {
        // An empty file is legitimate: it has nothing to drop. Making a
        // zero-byte chunk would force the depositor to send a frame without
        // content to seal a file without content.
        expect(plan(0, 4)).toEqual([]);
    });

    it.each([
        [0, 4],
        [1, 4],
        [4, 4],
        [8, 4],
        [10, 4],
        [10, 1],
        [7, 3],
    ])(
        '🔴 the sum of the bytes is EXACTLY the size (size=%i, step=%i)',
        (size, pas) => {
            // The comparison is on `size`, NOT on `plan`: it is what
            // makes the assertion able to see a lost file tail.
            expect(somme(plan(size, pas))).toBe(size);
        },
    );

    it.each([
        [0, 4, 0],
        [1, 4, 1],
        [4, 4, 1],
        [8, 4, 2],
        [10, 4, 3],
        [9, 3, 3],
    ])(
        '🔴 the number of chunks is the ceiling of the quotient (size=%i, step=%i → %i)',
        (size, pas, combien) => {
            // The expected number is written by hand, never recomputed: a
            // `Math.ceil` in the test would reproduce the defect it looks for.
            expect(plan(size, pas)).toHaveLength(combien);
        },
    );

    it("does NOT make an empty final chunk when the size is an exact multiple of the step", () => {
        const tranches = plan(8, 4);
        expect(tranches).toEqual([
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ]);
        expect(tranches.some((t) => t.octets === 0)).toBe(false);
    });

    it("returns ONE single chunk, of the file size, when the file is smaller than the step", () => {
        expect(plan(3, 4)).toEqual([{ n: 0, octets: 3 }]);
    });

    it('numbers the ranks from ZERO and contiguously, so that the position can be computed', () => {
        const pas = 4;
        const tranches = plan(10, pas);
        expect(tranches.map((t) => t.n)).toEqual([0, 1, 2]);
        // The property the zero base buys: `n * pas` IS the position, without
        // a table or an offset to remember on each side of the bridge.
        const positions = tranches.map((t) => t.n * pas);
        expect(positions).toEqual([0, 4, 8]);
    });

    it('shortens ONLY the last chunk', () => {
        const tranches = plan(10, 4);
        expect(tranches.slice(0, -1).every((t) => t.octets === 4)).toBe(true);
        expect(tranches[tranches.length - 1].octets).toBe(2);
    });

    it.each<[string, number, number]>([
        ['a zero step — which would also make the plan loop forever', 10, 0],
        ['a negative step', 10, -1],
        ['a non-integer step', 10, 2.5],
        ['a NaN step', 10, Number.NaN],
        ['a negative size', -1, 4],
        ['a non-integer size', 2.5, 4],
        ['a NaN size', Number.NaN, 4],
    ])('THROWS on an invalid contract: %s', (_titre, size, pas) => {
        // The contract is held by the caller, not received from the wire: an
        // absurd contract is a program defect, and returning it as a
        // verdict would disguise it as a transfer anomaly.
        expect(() => plan(size, pas)).toThrow(/chunks:/);
    });
});

describe('verdict', () => {
    it('returns complete when all the expected chunks are there, at the right size', () => {
        expect(verdict(10, 4, plan(10, 4))).toEqual({ etat: 'complet' });
    });

    it("returns complete whatever the ORDER of arrival of the chunks", () => {
        // Nothing guarantees the depositor emits in order, nor that the
        // network returns them in order.
        const desordre = [...plan(10, 4)].reverse();
        expect(verdict(10, 4, desordre)).toEqual({ etat: 'complet' });
    });

    it('names a hole in the middle', () => {
        expect(verdict(12, 4, [
            { n: 0, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'manquantes', n: [1] });
    });

    it("🔴 refuses to declare complete a TRUNCATED file: the last chunk is missing", () => {
        // It is the exact case the `ceil → floor` mutation makes invisible.
        // With `floor`, the plan of 10 bytes by 4 would only count two
        // chunks, the two below would suffice, and two bytes would be
        // lost WITHOUT ANY TRACE SAYING SO.
        expect(verdict(10, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'manquantes', n: [2] });
    });

    it.each([
        [10, 4],
        [7, 3],
        [1, 4],
        [9, 2],
    ])(
        "🔴 an upload cut off from its last chunk is NEVER complete (size=%i, step=%i)",
        (size, pas) => {
            const ampute = plan(size, pas).slice(0, -1);
            // The dropped sum is strictly lower than the size: it is
            // the most direct formulation of "the file is truncated".
            expect(somme(ampute)).toBeLessThan(size);
            expect(verdict(size, pas, ampute).etat).toBe('manquantes');
        },
    );

    it('names all the chunks when nothing has been dropped', () => {
        expect(verdict(10, 4, [])).toEqual({ etat: 'manquantes', n: [0, 1, 2] });
    });

    it('declares INCONSISTENT a chunk SHORTER than planned, not missing', () => {
        // A truncated transfer: asking for it again would return the same thing.
        expect(verdict(12, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 3 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [1] });
    });

    it('declares INCONSISTENT a chunk LONGER than planned', () => {
        // It would overflow onto its neighbour.
        expect(verdict(12, 4, [
            { n: 0, octets: 5 },
            { n: 1, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('declares INCONSISTENT the last chunk sent at the size of the step', () => {
        // The depositor's natural trap: filling the tail up to the step.
        expect(verdict(10, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [2] });
    });

    it('declares INCONSISTENT a rank beyond the plan', () => {
        expect(verdict(8, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [2] });
    });

    it('declares INCONSISTENT a negative rank', () => {
        expect(verdict(8, 4, [
            { n: -1, octets: 4 },
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [-1] });
    });

    it('declares INCONSISTENT a non-integer rank, and returns it AS IS', () => {
        // The list must show what was really sent, not a cleaned-up
        // value: it is what a log needs to read.
        expect(verdict(8, 4, [{ n: 1.5, octets: 4 }])).toEqual({
            etat: 'incoherentes',
            n: [1.5],
        });
    });

    it('declares INCONSISTENT a non-integer byte count', () => {
        expect(verdict(8, 4, [
            { n: 0, octets: 4.5 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('declares INCONSISTENT a DUPLICATE whose two occurrences agree on the size', () => {
        // 🔴 Two drops for the same rank: one overwrote the other, and nothing here
        // can know which one won. Two frames of the same length do not
        // necessarily carry the same content.
        expect(verdict(8, 4, [
            { n: 0, octets: 4 },
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('declares INCONSISTENT a duplicate whose occurrences diverge', () => {
        expect(verdict(8, 4, [
            { n: 1, octets: 4 },
            { n: 1, octets: 2 },
            { n: 0, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [1] });
    });

    it('🔴 makes incoherentes PREVAIL over manquantes when both are present', () => {
        // Announcing the hole first would make the absent chunk be completed again, then
        // rechecked, then fall back on the same inconsistency — the exact loop
        // the distinction exists to prevent.
        const rendu = verdict(12, 4, [{ n: 0, octets: 9 }]);
        expect(rendu).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('returns complete for an empty file of which nothing has been dropped', () => {
        expect(verdict(0, 4, [])).toEqual({ etat: 'complet' });
    });

    it('declares INCONSISTENT the slightest chunk dropped for an empty file', () => {
        expect(verdict(0, 4, [{ n: 0, octets: 0 }])).toEqual({
            etat: 'incoherentes',
            n: [0],
        });
    });

    it('sorts the ranks NUMERICALLY and without duplicates', () => {
        // ⚠️ Eleven chunks: it is the threshold from which JavaScript's default
        // sort, which compares STRINGS, would return `[0, 10, 2, …]`.
        const rendu = verdict(44, 4, [{ n: 1, octets: 4 }]);
        expect(rendu).toEqual({
            etat: 'manquantes',
            n: [0, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        });
    });

    it('does not repeat an inconsistent rank reported several times', () => {
        const rendu = verdict(12, 4, [
            { n: 2, octets: 1 },
            { n: 2, octets: 1 },
            { n: 2, octets: 1 },
        ]);
        expect(rendu.etat).toBe('incoherentes');
        expect((rendu as { n: number[] }).n).toEqual([2]);
    });

    it.each<[string, number, number]>([
        ['a zero step', 10, 0],
        ['a negative size', -1, 4],
    ])('THROWS on an invalid contract, like plan: %s', (_titre, size, pas) => {
        // The guard is the SAME on both sides: a contract that does not hold must
        // not produce a normal-looking verdict.
        expect(() => verdict(size, pas, [])).toThrow(/chunks:/);
    });

    it('NEVER throws on what comes from the wire, however absurd', () => {
        // A misbehaving or malicious peer must not be able to bring down the
        // checker: everything coming from `presentes` becomes a verdict.
        expect(() =>
            verdict(8, 4, [
                { n: -7, octets: Number.NaN },
                { n: 1e9, octets: -3 },
                { n: 0.5, octets: Infinity },
            ]),
        ).not.toThrow();
    });
});
