import { describe, expect, it } from 'vitest';
import {
    CODES_ECHEC,
    FILES_VERSION,
    FIXED_HEADER_SIZE,
    MAX_FRAME_SIZE,
    ALL_TYPES,
    TYPE_DATA,
    TYPE_ENTREES,
    TYPE_LISTER,
    TYPE_META,
    decoder,
    encoder,
} from './fichiers';

/**
 * The pinned vector, **hardcoded here AND in `proto/src/files/tests.rs`**.
 *
 * ⚠️ It is the only way to see RED an endianness divergence between the
 * two implementations: a TS→TS round trip stays green whatever the
 * byte order, as long as it is the same on both sides of the same file. The
 * bytes below are the source of truth of the format, not a consequence
 * of the code — they must stay identical, byte for byte, to
 * `VECTEUR_EPINGLE` on the Rust side.
 *
 * version 1 | type 66 (`TYPE_DATA`) | correlation 0x0A0B0C0D little-endian |
 * header length 2 little-endian | header `{}` | payload `00 FF 7F 80`.
 */
const VECTEUR_EPINGLE = new Uint8Array([
    1, 66, 0x0d, 0x0c, 0x0b, 0x0a, 2, 0, 0, 0, 0x7b, 0x7d, 0x00, 0xff, 0x7f, 0x80,
]);

function octets(t: ArrayBuffer): Uint8Array {
    return new Uint8Array(t);
}

describe('binary frame of the file bridge', () => {
    it('refuses a frame without a version', () => {
        // Zero bytes do not carry their version: rejection, never completion.
        expect(() => decoder(new ArrayBuffer(0))).toThrow(/truncated/i);
        // …and one byte less than the fixed header is too.
        expect(() => decoder(new ArrayBuffer(FIXED_HEADER_SIZE - 1))).toThrow(/truncated/i);
    });

    it('refuses a version 2 frame', () => {
        const trame = octets(encoder(TYPE_LISTER, 7, {}));
        trame[0] = FILES_VERSION + 1;
        expect(() => decoder(trame.buffer as ArrayBuffer)).toThrow(/version/i);
    });

    it("refuses a header whose length overflows the frame", () => {
        const trame = octets(encoder(TYPE_ENTREES, 1, {}, new Uint8Array([1, 2, 3])));
        new DataView(trame.buffer).setUint32(6, 0xffffffff, true);
        expect(() => decoder(trame.buffer as ArrayBuffer)).toThrow(/header/i);

        // Overflowing by ONE SINGLE byte is refused too: that is where
        // the strict inequality error lives.
        const dun = octets(encoder(TYPE_ENTREES, 1, {}));
        new DataView(dun.buffer).setUint32(6, 3, true);
        expect(() => decoder(dun.buffer as ArrayBuffer)).toThrow(/header/i);
    });

    it('keeps the raw bytes on a round trip', () => {
        // 0x00 and 0xFF are the two bytes a textual encoding damages
        // first; we go through all 256.
        const charge = new Uint8Array(256);
        for (let i = 0; i < 256; i += 1) charge[i] = i;
        const trame = decoder(encoder(TYPE_DATA, 0xdeadbeef, { position: 0 }, charge));
        expect(trame.version).toBe(FILES_VERSION);
        expect(trame.type).toBe(TYPE_DATA);
        expect(trame.correlation).toBe(0xdeadbeef);
        expect(trame.entete).toEqual({ position: 0 });
        expect(Array.from(trame.charge)).toEqual(Array.from(charge));
    });

    it('accepts an empty payload and an empty header', () => {
        const brut = octets(encoder(TYPE_META, 0));
        expect(brut.length).toBe(FIXED_HEADER_SIZE);
        const trame = decoder(brut.buffer as ArrayBuffer);
        expect(trame.entete).toBeUndefined();
        expect(trame.charge.length).toBe(0);
        expect(trame.correlation).toBe(0);
    });

    it('accepts a payload of TAILLE_TRAME_MAX and refuses one more byte', () => {
        // At the EXACT threshold. It is the strict inequality that is tested, not the
        // bound in general.
        const pleine = new Uint8Array(MAX_FRAME_SIZE).fill(0xab);
        expect(decoder(encoder(TYPE_DATA, 1, {}, pleine)).charge.length).toBe(MAX_FRAME_SIZE);

        const trop = new Uint8Array(MAX_FRAME_SIZE + 1).fill(0xab);
        expect(() => decoder(encoder(TYPE_DATA, 1, {}, trop))).toThrow(/payload/i);
    });

    it('decodes what Rust encoded — the pinned vector', () => {
        // ⚠️ THE test of this file. Without it, an endianness divergence between
        // Rust and TypeScript would stay green on both sides.
        const trame = decoder(VECTEUR_EPINGLE.buffer as ArrayBuffer);
        expect(trame.version).toBe(1);
        expect(trame.type).toBe(TYPE_DATA);
        expect(trame.correlation).toBe(0x0a0b0c0d);
        expect(trame.entete).toEqual({});
        expect(Array.from(trame.charge)).toEqual([0x00, 0xff, 0x7f, 0x80]);
        // …and the TypeScript encoder REPRODUCES it to the byte.
        expect(
            Array.from(
                octets(encoder(TYPE_DATA, 0x0a0b0c0d, {}, new Uint8Array([0x00, 0xff, 0x7f, 0x80]))),
            ),
        ).toEqual(Array.from(VECTEUR_EPINGLE));
    });

    it('pins the shape of the failure codes on the wire', () => {
        // ⚠️ The TWO-WORD variants are the ones that break silently:
        // this repository let `battement-recu` pass green across fifty tests
        // because nothing pinned its bytes. These ELEVEN strings must be
        // identical, character for character, to the `#[serde(rename_all =
        // "kebab-case")]` of `CodeEchec` on the Rust side.
        expect(CODES_ECHEC).toEqual([
            'introuvable',
            'chemin-introuvable',
            'acces-refuse',
            'protege-en-ecriture',
            'non-supporte',
            'trop-grand',
            'interne',
            'disque-plein',
            'deja-present',
            'casse-ambigue',
            'repertoire-non-vide',
        ]);
    });

    it('makes no message type overlap', () => {
        // `ALL_TYPES` is DERIVED from the union, not written by hand: it is
        // the structural remedy to the defect of `TYPES_AGENT` (`control.ts:106`),
        // a manual list nothing confronts with its union. Adding a type
        // without registering it in the table breaks `tsc --noEmit`, not only this
        // test.
        expect(new Set(ALL_TYPES).size).toBe(ALL_TYPES.length);
        expect(ALL_TYPES).toContain(TYPE_LISTER);
        expect(ALL_TYPES).toContain(TYPE_DATA);
    });
});
