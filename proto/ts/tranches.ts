/**
 * The SLICING rule of an upload — PURE, and shared
 * between the two ends that depend on it.
 *
 * 🔴 WHY IT LIVES IN `proto/ts/` AND NOT IN THE CLIENT: two
 * independent arithmetics — one that slices and uploads (the browser),
 * the other that checks the slicing is complete before sealing (the
 * platform) — would diverge one day, and the symptom would be a SEALING THAT
 * REFUSES WITHOUT KNOWING WHICH OF THE TWO ENDS IS WRONG. That is exactly the
 * pattern this repository paid for on `TYPES_AGENT` and on the
 * `battement-recu` variant that stayed green over fifty tests: a shape reproduced by
 * hand on each side breaks the bridge without breaking a single test. Here there is
 * only one arithmetic, and both ends import it.
 *
 * ⚠️ NO `node:`, NO DOM, NO DEPENDENCY — this module must load in
 * a browser as in the service. That is the condition for it to be
 * truly shared rather than copied.
 *
 * ⚠️ `size`, `chunkSize` and `octets` are `number`: beyond 2^53
 * the arithmetic would stop being exact, and the two ends would diverge
 * silently. A 9-petabyte upload does not exist; the bound is named,
 * not guarded — same trade-off, and for the same reason, as that of
 * `fichiers-entetes.ts`.
 */

/**
 * A slice: its RANK and the number of bytes it carries.
 *
 * ⚠️ `n` IS A ZERO-BASED RANK, and this choice is load-bearing: it makes the position
 * of the slice in the file computable without a table — it is exactly
 * `n * chunkSize`. A base of 1 would force every caller to subtract one,
 * and the day one of the two forgot, the whole file would be shifted by one
 * slice without any size moving.
 */
export interface Tranche {
    n: number;
    octets: number;
}

/**
 * The verdict on a received slicing.
 *
 * 🔴 `incoherentes` IS NOT `manquantes`, AND CONFUSING THEM WOULD BE AN ENDLESS
 * LOOP. An ABSENT slice is a hole: asking for it again fills it. A
 * slice PRESENT AT THE WRONG SIZE is a PROTOCOL ERROR — the two
 * ends no longer agree on the slicing —, and asking for it again would
 * NEVER repair it: the uploader would send the same thing again, forever. The
 * first case is recoverable, the second must fail the upload and
 * say so.
 */
export type Verdict =
    | { etat: 'complet' }
    | { etat: 'manquantes'; n: number[] }
    | { etat: 'incoherentes'; n: number[] };

/**
 * Checks the CONTRACT of the upload — its size and its slicing step.
 *
 * 🔴 WHAT IS GUARDED HERE THROWS; WHAT COMES FROM THE WIRE NEVER THROWS. The
 * boundary is deliberate, and it is the only one in this module:
 *
 * - `size` and `chunkSize` are the CONTRACT, fixed when the upload is
 *   declared and held by the caller. An absurd contract — a zero step,
 *   a negative size — is a PROGRAM defect, not received data:
 *   returning it as a verdict would disguise it as a transfer anomaly, and
 *   the uploader would spend its life refilling slices that do not exist.
 *   It throws, and it names the faulty value.
 * - `presentes`, on the other hand, is what the wire brought. Nothing throws there:
 *   everything becomes a verdict, because a malicious or broken peer must
 *   not be able to bring the checker down by sending it anything at all.
 *
 * ⚠️ A ZERO slicing step is not merely absurd: it would make
 * `Math.ceil(size / 0)` equal to `Infinity`, and the plan loop would not
 * stop. The guard is thus also what keeps this module from freezing
 * its caller.
 */
function checkContract(size: number, chunkSize: number): void {
    if (!Number.isInteger(size) || size < 0) {
        throw new Error(
            `tranches : taille invalide (${size}) — un entier positif ou nul est attendu`,
        );
    }
    if (!Number.isInteger(chunkSize) || chunkSize <= 0) {
        throw new Error(
            `tranches : tailleTranche invalide (${chunkSize}) — un entier strictement positif est attendu`,
        );
    }
}

/**
 * The EXPECTED slicing of a file of `size` bytes with a step of
 * `chunkSize`.
 *
 * Ranks are contiguous from `0` to `n - 1`, and the sum of the `octets` is
 * EXACTLY `size` — it is the invariant the tests pin, and it is the
 * only one that tells a right plan apart from a truncated plan.
 *
 * 🔴 `Math.ceil` AND NOT `Math.floor`, AND THE DIFFERENCE IS THE TAIL OF THE FILE.
 * With `floor`, a 10-byte file sliced by 4 would yield TWO slices of
 * 4 — eight bytes — and `verdict` would then declare `complet` a file
 * TRUNCATED by two bytes, without any trace saying so. It is the mutation
 * that judges this module: it breaks no exact-multiple case, and it silently
 * damages all the others.
 *
 * ⚠️ `size === 0` YIELDS ZERO SLICES, never one empty slice. An empty file
 * is a legitimate file: it has nothing to upload, and its verdict is `complet`
 * on an empty list. Crafting a zero-byte slice would force the
 * uploader to send a frame with no content to seal a file with no
 * content, and `ceil(0 / step)` is already 0 — the property belongs to
 * the arithmetic, not to a special case added by hand.
 *
 * ⚠️ A SIZE THAT IS AN EXACT MULTIPLE OF THE STEP DOES NOT PRODUCE AN EMPTY FINAL SLICE,
 * for the same reason: `ceil(8 / 4)` is 2, not 3. The last slice is
 * `size - n * chunkSize`, which is zero only if no slice exists.
 */
export function plan(size: number, chunkSize: number): Tranche[] {
    checkContract(size, chunkSize);

    const tranches: Tranche[] = [];
    const combien = Math.ceil(size / chunkSize);
    for (let n = 0; n < combien; n += 1) {
        // The last slice is the only one that may be shorter than the
        // step: `min` bounds it without having to treat its case separately.
        const octets = Math.min(chunkSize, size - n * chunkSize);
        tranches.push({ n, octets });
    }
    return tranches;
}

/**
 * Confronts the RECEIVED slices with the EXPECTED slicing, and returns one of the three
 * verdicts.
 *
 * 🔴 `incoherentes` TAKES PRECEDENCE OVER `manquantes`, AND THE ORDER IS HALF THE
 * RULE. An upload that carried both a hole and a wrongly sized slice
 * must be reported INCONSISTENT: announcing the hole first would refill the
 * absent slice, then check again, then fall back onto the same inconsistency — the
 * exact loop the distinction exists to prevent. We first name what
 * cannot be repaired.
 *
 * ⚠️ THE LISTS ARE SORTED AND FREE OF DUPLICATES, always. A verdict that depended
 * on the arrival order of the slices would be neither comparable from one run to
 * the next, nor readable in a log — and two readings of the same defect
 * would look different.
 *
 * ⚠️ `presentes` IS WIRE DATA, AND ITS SHAPE IS THE CALLER'S.
 * This module does not parse: it assumes the entries have already passed the
 * shape guard, as the `parse*` of `fichiers-entetes.ts` makes them pass before
 * the rule applies. An entry whose `n` is not a valid rank
 * is NOT silently discarded for all that — it falls into
 * `incoherentes`, and the list returns the RECEIVED `n` as is, so that a log
 * shows what was really sent rather than a cleaned-up value.
 */
export function verdict(
    size: number,
    chunkSize: number,
    presentes: Tranche[],
): Verdict {
    checkContract(size, chunkSize);

    const attendu = new Map<number, number>();
    for (const t of plan(size, chunkSize)) attendu.set(t.n, t.octets);

    const incoherentes = new Set<number>();
    const vues = new Set<number>();

    for (const recue of presentes) {
        const { n, octets } = recue;

        // 🔴 A DUPLICATE IS AN INCONSISTENCY, EVEN IF THE TWO OCCURRENCES
        // AGREE ON THE SIZE. Two uploads claiming the same rank
        // mean that one overwrote the other, and NOTHING HERE CAN KNOW
        // WHICH ONE WON: the bytes actually written may come from the
        // second frame as well as from the first, and two frames of the same length
        // do not necessarily carry the same content. Treating the case as
        // harmless would seal a file with an undetermined slice. We
        // refuse it, and we name it.
        if (vues.has(n)) {
            incoherentes.add(n);
            continue;
        }
        vues.add(n);

        // A rank outside the plan — negative, beyond the last slice, or
        // simply not a rank — has no expected size to
        // compare it with: it is inconsistent by itself.
        const prevu = attendu.get(n);
        if (prevu === undefined) {
            incoherentes.add(n);
            continue;
        }

        // The comparison is strict IN BOTH DIRECTIONS: a slice
        // SHORTER than planned is a truncated transfer, a LONGER slice
        // would overflow onto its neighbour. Neither is repaired by
        // asking for it again.
        if (!Number.isInteger(octets) || octets !== prevu) {
            incoherentes.add(n);
        }
    }

    if (incoherentes.size > 0) {
        return { etat: 'incoherentes', n: trier(incoherentes) };
    }

    const manquantes = new Set<number>();
    for (const n of attendu.keys()) {
        if (!vues.has(n)) manquantes.add(n);
    }
    if (manquantes.size > 0) {
        return { etat: 'manquantes', n: trier(manquantes) };
    }

    return { etat: 'complet' };
}

/**
 * Sorts a list of ranks by increasing value.
 *
 * ⚠️ THE COMPARATOR IS EXPLICIT, and it is not an affectation: the default
 * JavaScript sort compares STRINGS, so that `[2, 10]` would
 * come out as `[10, 2]`. A plan of more than ten slices is enough to hit
 * the case.
 */
function trier(rangs: Set<number>): number[] {
    return [...rangs].sort((a, b) => a - b);
}
