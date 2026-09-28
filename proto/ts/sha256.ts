// **INCREMENTAL** SHA-256 (FIPS 180-4 §6.2), in pure JavaScript — the twin of
// `agent/src/apps/sha256.rs`, with one more reason to exist.
//
// 🔴 WHY THIS MODULE EXISTS WHEN THE BROWSER ALREADY KNOWS HOW TO HASH:
// the Web Crypto API only exposes `crypto.subtle.digest(algo, buffer)`, which takes
// the **WHOLE** message at once. It has no `update`, no incremental
// `digest`, nothing close to it — that is a gap in the API, not an
// oversight on our part. Fingerprinting an 800 MB installer that way
// would thus require holding 800 MB in a tab, on top of what reading
// the file already consumes.
//
// 🔵 THE PRICE OF THE REMEDY IS MEASURED, NOT ASSUMED (Node 24.9.0):
//
//   | path                                  | throughput    |
//   | ------------------------------------- | ------------- |
//   | this module, pure JS                  |   74.7 MB/s   |
//   | `crypto.subtle.digest`, native        | 1160.6 MB/s   |
//
// That is **≈ 11 s for 800 MB**. It is the trade-off, written rather than suffered:
// **11 s of waiting is affordable, 800 MB in memory is not.** The day
// a browser exposes an incremental digest, this module should
// give way — and this sentence is here so that we know it.
//
// ⚠️ THIS TABLE IS THE PLAN'S READING, AND A SECOND MEASUREMENT CONFIRMS
// ONLY ONE LINE OF IT. Replayed on this module once, when it was written: **73.9 MB/s**
// here (so 10.8 s for 800 MB — the line that carries the decision holds), but
// only **462.4 MB/s** for `crypto.subtle`, against 1160.6 announced. One
// run each, without warm-up: no rate, and the gap is NOT
// explained. **It changes nothing in the trade-off** — memory decides,
// not the throughput ratio —, but it is declared rather than smoothed over.
//
// 🔴 NO DEPENDENCY, NO `node:`, NO DOM: it is an invariant that
// sub-blocks G1 and G2 both hold, and this module does not dent it. It
// loads identically in a browser and under Node — which is also the
// condition for the comparison to `crypto.subtle` in the test file to be
// an INDEPENDENT oracle, and not our own code reread twice.
//
// ⚠️ IT IS NOT A SECURITY PRIMITIVE HERE: the fingerprint serves as a stable
// identity for uploaded content, never to authenticate anything. The
// day something authenticating depends on it, this module must give
// way to an audited implementation.
//
// The proof rests on the FIPS 180-4 known-answer vectors AND on the
// comparison with `crypto.subtle` over IRREGULAR splits: a
// wrong algorithm fails the former, a wrong residual buffer fails the latter.

/**
 * The 64 round constants, §4.2.3.
 *
 * ⚠️ `Int32Array` and not a plain array, and it is not an affectation:
 * it forces each constant into a signed 32-bit integer, which makes the arithmetic
 * of the rest of the file homogeneous. A plain array would keep `0x428a2f98`
 * as a positive float, and mixing the two representations is
 * exactly where the bugs of a hand-written SHA-256 are born.
 */
const K = new Int32Array([
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);

/** The initial state, §5.3.3. */
const ETAT_INITIAL = new Int32Array([
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
]);

/**
 * The largest message size this module accepts.
 *
 * ⚠️ The length travels in the padding **in BITS**, hence `octets * 8`. Beyond
 * `MAX_SAFE_INTEGER / 8` (2^50, i.e. 1 PiB) this multiplication stops
 * being exact in floating point and the padding would carry a WRONG
 * length — without anything saying so. The bound is thus GUARDED, not only
 * named: `absorber` throws. No browser upload comes near it.
 */
const OCTETS_MAX = Math.floor(Number.MAX_SAFE_INTEGER / 8);

/** The right rotation of §3.2, on 32 bits. */
function rotr(x: number, n: number): number {
    return (x >>> n) | (x << (32 - n));
}

/**
 * One compression pass over the 64 bytes at `decalage` in `octets`.
 *
 * ⚠️ The block is read **in place**, by offset, rather than sliced with
 * `subarray`: at 74.7 MB/s that is over a million objects per second
 * avoided on a large file. `w` is supplied by the caller for the same
 * reason — reallocating it per block would dominate the cost.
 */
function comprimer(etat: Int32Array, w: Int32Array, octets: Uint8Array, decalage: number): void {
    for (let i = 0; i < 16; i += 1) {
        const d = decalage + i * 4;
        w[i] = (octets[d] << 24) | (octets[d + 1] << 16) | (octets[d + 2] << 8) | octets[d + 3];
    }
    for (let i = 16; i < 64; i += 1) {
        const x = w[i - 15];
        const y = w[i - 2];
        const s0 = rotr(x, 7) ^ rotr(x, 18) ^ (x >>> 3);
        const s1 = rotr(y, 17) ^ rotr(y, 19) ^ (y >>> 10);
        w[i] = (w[i - 16] + s0 + w[i - 7] + s1) | 0;
    }

    let a = etat[0];
    let b = etat[1];
    let c = etat[2];
    let d = etat[3];
    let e = etat[4];
    let f = etat[5];
    let g = etat[6];
    let h = etat[7];
    for (let i = 0; i < 64; i += 1) {
        const s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
        const ch = (e & f) ^ (~e & g);
        // Five signed 32-bit integers: their sum stays below 2^34, hence exact
        // in floating point, and the `| 0` brings it back modulo 2^32 — it is
        // the equivalent of the Rust twin's `wrapping_add`.
        const t1 = (h + s1 + ch + K[i] + w[i]) | 0;
        const s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
        const maj = (a & b) ^ (a & c) ^ (b & c);
        const t2 = (s0 + maj) | 0;
        h = g;
        g = f;
        f = e;
        e = (d + t1) | 0;
        d = c;
        c = b;
        b = a;
        a = (t1 + t2) | 0;
    }

    etat[0] = (etat[0] + a) | 0;
    etat[1] = (etat[1] + b) | 0;
    etat[2] = (etat[2] + c) | 0;
    etat[3] = (etat[3] + d) | 0;
    etat[4] = (etat[4] + e) | 0;
    etat[5] = (etat[5] + f) | 0;
    etat[6] = (etat[6] + g) | 0;
    etat[7] = (etat[7] + h) | 0;
}

/**
 * A SHA-256 digest fed chunk by chunk.
 *
 * ```ts
 * const empreinte = new Sha256();
 * for await (const morceau of flux) empreinte.absorber(morceau);
 * const hex = empreinte.terminer();
 * ```
 *
 * 🔴 THE CHUNKS HAVE NO IMPOSED SIZE, and that is the whole point: a file
 * `ReadableStream` yields what it wants, never multiples of 64.
 * What absorbs this is the residual buffer below, and it is the only part
 * of this module the FIPS vectors do NOT exercise — hence the comparison
 * with `crypto.subtle` over irregular splits, on the test side.
 */
export class Sha256 {
    private readonly etat = Int32Array.from(ETAT_INITIAL);
    /** The message schedule, allocated once for the whole life of the object. */
    private readonly w = new Int32Array(64);
    /** The bytes received that have not yet completed a 64-byte block. */
    private readonly residu = new Uint8Array(64);
    private residueLength = 0;
    /** The TOTAL number of bytes absorbed — this is what the padding records. */
    private octets = 0;
    /** The fingerprint, once `terminer` has been called. `null` until then. */
    private empreinte: string | null = null;

    /**
     * Absorbs a chunk, of any size, including empty.
     *
     * ⚠️ **Throws if `terminer` has already been called.** The internal state was destroyed
     * by the padding: continuing would yield a silently wrong fingerprint,
     * and a wrong fingerprint that does not flag itself is the worse of the two evils
     * for a content identity.
     */
    absorber(bloc: Uint8Array): void {
        if (this.empreinte !== null) {
            throw new Error('Sha256.absorber after Sha256.terminer: the digest is closed');
        }
        if (this.octets + bloc.length > OCTETS_MAX) {
            throw new Error(`Sha256: message of more than ${OCTETS_MAX} bytes, length not representable`);
        }
        this.octets += bloc.length;

        let i = 0;
        // First complete the residue, if there is one: as long as it is not
        // full, no block of the input is aligned on a 64-byte boundary.
        if (this.residueLength > 0) {
            const pris = Math.min(64 - this.residueLength, bloc.length);
            this.residu.set(bloc.subarray(0, pris), this.residueLength);
            this.residueLength += pris;
            i = pris;
            if (this.residueLength < 64) return;
            comprimer(this.etat, this.w, this.residu, 0);
            this.residueLength = 0;
        }
        // Then the full blocks, read straight from the input: no copy.
        for (; i + 64 <= bloc.length; i += 64) {
            comprimer(this.etat, this.w, bloc, i);
        }
        // What remains waits for the next chunk, or the padding.
        if (i < bloc.length) {
            this.residu.set(bloc.subarray(i), 0);
            this.residueLength = bloc.length - i;
        }
    }

    /**
     * Closes the digest and returns the 64 **lowercase** hexadecimal characters.
     *
     * ⚠️ **Idempotent**: the fingerprint is kept, and a second call returns the
     * same value rather than throwing. That is the intended asymmetry with `absorber`
     * — rereading a result is harmless, continuing a closed computation is
     * not.
     */
    terminer(): string {
        if (this.empreinte !== null) return this.empreinte;

        // FIPS 180-4 §5.1.1 padding: the byte 0x80, zeros, then the
        // length in BITS on 64 big-endian bits. If the residue reaches
        // 56 bytes, the length no longer fits in this block and a
        // SECOND one is needed — that is the case the third FIPS vector exercises, and the
        // only place in this file where a bound error (`<` versus `<=`)
        // would stay invisible on short messages.
        const queue = new Uint8Array(128);
        queue.set(this.residu.subarray(0, this.residueLength), 0);
        queue[this.residueLength] = 0x80;
        const size = this.residueLength < 56 ? 64 : 128;

        const bits = this.octets * 8;
        const haut = Math.floor(bits / 4294967296);
        const bas = bits - haut * 4294967296;
        for (let i = 0; i < 4; i += 1) {
            queue[size - 8 + i] = (haut >>> ((3 - i) * 8)) & 0xff;
            queue[size - 4 + i] = (bas >>> ((3 - i) * 8)) & 0xff;
        }

        for (let debut = 0; debut < size; debut += 64) {
            comprimer(this.etat, this.w, queue, debut);
        }

        let sortie = '';
        for (let i = 0; i < 8; i += 1) {
            sortie += (this.etat[i] >>> 0).toString(16).padStart(8, '0');
        }
        this.empreinte = sortie;
        return sortie;
    }
}

/**
 * The fingerprint of a message held whole, as 64 lowercase hexadecimal
 * characters — the convenience matching `hex` of the Rust twin.
 *
 * ⚠️ Only to be used on what already fits in memory. For a file, it is
 * `Sha256` chunk by chunk, otherwise this module loses its reason to exist.
 */
export function condenserHex(message: Uint8Array): string {
    const empreinte = new Sha256();
    empreinte.absorber(message);
    return empreinte.terminer();
}
