import { describe, expect, it } from 'vitest';
import { Sha256, condenserHex } from './sha256';

/** The ORACLE's byte-to-hexadecimal — never that of the module under test. */
function hexDe(tampon: ArrayBuffer): string {
    return Array.from(new Uint8Array(tampon))
        .map((octet) => octet.toString(16).padStart(2, '0'))
        .join('');
}

/** What the BROWSER answers, in one go. It is the independent oracle. */
async function empreinteDeReference(message: Uint8Array): Promise<string> {
    // ⚠️ The copy is not superfluous: `crypto.subtle.digest` requires a
    // `BufferSource` backed by an `ArrayBuffer`, whereas an arbitrary `Uint8Array`
    // can be backed by a `SharedArrayBuffer` — which `tsc` refuses to rule out. A
    // type assertion would silence it; the copy settles it, and the oracle stays an
    // oracle.
    const copie = new Uint8Array(new ArrayBuffer(message.length));
    copie.set(message);
    return hexDe(await crypto.subtle.digest('SHA-256', copie));
}

/**
 * A deterministic generator (xorshift32), and it is not decorative.
 *
 * ⚠️ A message of N identical bytes would let through a fault that
 * swaps two positions of the block — the content being the same everywhere, the swap
 * would be invisible. The content must therefore VARY, and it must be REPRODUCIBLE
 * so that a failure replays identically: hence a fixed seed rather than
 * `Math.random`.
 */
function messageOfSize(size: number): Uint8Array {
    const octets = new Uint8Array(size);
    let etat = 0x9e3779b9 ^ size;
    for (let i = 0; i < size; i += 1) {
        etat ^= etat << 13;
        etat ^= etat >>> 17;
        etat ^= etat << 5;
        octets[i] = etat & 0xff;
    }
    return octets;
}

function texte(chain: string): Uint8Array {
    return new TextEncoder().encode(chain);
}

/** Absorbs `message` in chunks of `size` bytes, then closes. */
function empreinteParMorceaux(message: Uint8Array, size: number): string {
    const empreinte = new Sha256();
    for (let i = 0; i < message.length; i += size) {
        empreinte.absorber(message.subarray(i, Math.min(i + size, message.length)));
    }
    return empreinte.terminer();
}

describe('Sha256, the known-answer vectors', () => {
    /**
     * 🔴 THEY ARE WHAT MAKES THIS MODULE MORE THAN A PROMISE, and they
     * are copied from FIPS 180-4, **not** produced by our code. A
     * mistranscribed constant, a reversed rotation, a padding that forgets
     * its second block: all three fail.
     *
     * ⚠️ They are identical, character for character, to those of the Rust twin
     * (`agent/src/apps/sha256.rs`) — it is what would catch a divergence
     * between the two implementations, which must return the same identity for
     * the same content.
     */
    it('returns the three fingerprints of FIPS 180-4', () => {
        // §D.1: the empty message, whose whole block is padding.
        expect(condenserHex(texte(''))).toBe(
            'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
        );
        // §B.1 : « abc », un seul bloc.
        expect(condenserHex(texte('abc'))).toBe(
            'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
        );
        // §B.2: 448 bits — 56 bytes, so the length does NOT fit in the
        // padding block and a SECOND one is needed. It is the only vector of the
        // standard that exercises this branch.
        expect(
            condenserHex(texte('abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq')),
        ).toBe('248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1');
    });
});

/**
 * The message sizes tested, chosen on the padding boundaries.
 *
 * 55 is the last byte where the length still fits in the block, 56 the
 * first that requires a second one, 64 a full block whose padding takes up
 * a whole extra block. The large sizes, for their part, test the chaining.
 */
const SIZES = [0, 1, 3, 55, 56, 57, 63, 64, 65, 127, 128, 129, 1000, 4096, 100_000];

/**
 * 🔴 THE SPLITS, AND IT IS THE HEART OF THIS FILE.
 *
 * The residual buffer of `Sha256` is the only part of the module that the FIPS
 * vectors do NOT test: `condenserHex` absorbs everything at once, so never
 * leaves a residue to carry from one chunk to the next. Yet that is
 * exactly where the faults of an incremental digest live — and it is
 * exactly what a file stream will produce, since a `ReadableStream` returns
 * whatever it wants and never multiples of 64.
 *
 * A test that only absorbed in blocks of 64 would therefore prove almost nothing:
 * it would short-circuit the residue at each chunk. The sizes below are
 * chosen so that it is crossed in all its regimes — smaller than a
 * block (1, 63), exactly a block (64), straddling (65), much larger (1000).
 */
const DECOUPES = [1, 63, 64, 65, 1000];

describe('Sha256 against crypto.subtle', () => {
    /**
     * 🔵 THE ORACLE IS INDEPENDENT OF OUR CODE, and it is what gives this test
     * its weight: `crypto.subtle.digest` is the engine's implementation, written
     * by others, natively. Comparing our digest with its own cannot be
     * satisfied by a fault we would have committed twice — unlike
     * a round trip of our code against itself.
     */
    it.each(SIZES)('agrees on a message of %i bytes absorbed in one go', async (size) => {
        const message = messageOfSize(size);
        expect(condenserHex(message)).toBe(await empreinteDeReference(message));
    });

    it.each(DECOUPES)('agrees on all the messages absorbed in chunks of %i bytes', async (decoupe) => {
        for (const size of SIZES) {
            const message = messageOfSize(size);
            expect(empreinteParMorceaux(message, decoupe), `size ${size}, chunks of ${decoupe}`).toBe(
                await empreinteDeReference(message),
            );
        }
    });

    /**
     * The IRREGULAR split, the one no fixed size reproduces: the
     * chunks change length from one call to the next, as a real stream
     * would. It is the only case where the residue is picked up at offsets
     * different each time.
     */
    it('agrees on a split with variable lengths', async () => {
        const lengths = [1, 7, 64, 2, 63, 65, 128, 3, 55, 56, 1, 200, 9];
        for (const size of SIZES) {
            const message = messageOfSize(size);
            const empreinte = new Sha256();
            let i = 0;
            let n = 0;
            while (i < message.length) {
                const pris = Math.min(lengths[n % lengths.length], message.length - i);
                empreinte.absorber(message.subarray(i, i + pris));
                i += pris;
                n += 1;
            }
            expect(empreinte.terminer(), `size ${size}`).toBe(await empreinteDeReference(message));
        }
    });

    it('absorbs an empty chunk without changing anything', async () => {
        const message = messageOfSize(200);
        const empreinte = new Sha256();
        empreinte.absorber(new Uint8Array(0));
        empreinte.absorber(message.subarray(0, 70));
        empreinte.absorber(new Uint8Array(0));
        empreinte.absorber(message.subarray(70));
        empreinte.absorber(new Uint8Array(0));
        expect(empreinte.terminer()).toBe(await empreinteDeReference(message));
    });
});

describe('Sha256, the contract of the object', () => {
    it('throws when absorbing after finishing', () => {
        const empreinte = new Sha256();
        empreinte.absorber(texte('abc'));
        empreinte.terminer();
        // Going on would return a silently wrong hash: it must
        // throw, never lie.
        expect(() => empreinte.absorber(texte('def'))).toThrow(/closed/);
    });

    it('returns the same fingerprint at each call of terminer', () => {
        const empreinte = new Sha256();
        empreinte.absorber(texte('abc'));
        const premier = empreinte.terminer();
        expect(empreinte.terminer()).toBe(premier);
        expect(premier).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
    });

    it('returns 64 lowercase hexadecimal characters', () => {
        for (const size of SIZES) {
            expect(condenserHex(messageOfSize(size))).toMatch(/^[0-9a-f]{64}$/);
        }
    });

    it('returns distinct fingerprints for 130 distinct lengths', () => {
        // A broken padding — a `<` for a `<=`, a length written at the
        // wrong offset — would make two neighbouring sizes collide.
        const all = new Set<string>();
        for (let n = 0; n < 130; n += 1) all.add(condenserHex(new Uint8Array(n).fill(0x61)));
        expect(all.size).toBe(130);
    });
});
