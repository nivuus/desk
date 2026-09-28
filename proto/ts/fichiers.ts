// Binary frame of the file bridge. Must stay strictly aligned with
// proto/src/files.rs — the pinned vector of fichiers.test.ts, hardcoded (policy: allow-fr - file name)
// on both sides, is what checks it.
//
// Format: version u8 | type u8 | correlation u32 | header_length u32 |
// header | payload, integers in LITTLE-ENDIAN (`setUint32(…, true)`).
//
// The payload is NEVER encoded: that is the whole point of the binary format. The
// original bridge serialised the bytes through `Array.from(buffer.slice(…))`,
// about 4 bytes sent per useful byte, on every byte of every read.

export const FILES_VERSION = 1;

/**
 * Maximum size of a frame's PAYLOAD, in bytes.
 *
 * ⚠️ NOT CALIBRATED — set, not measured. And its name says "frame" where the
 * value bounds the payload: a divergence noted in the plan, settled on the side
 * that makes the module consistent with `pont::decoupe`. See the Rust twin's doc.
 */
export const MAX_FRAME_SIZE = 64 * 1024;

/** Version, type, correlation, header length. */
export const FIXED_HEADER_SIZE = 1 + 1 + 4 + 4;

// Bridge → browser requests — they EXPECT an answer.
export const TYPE_LISTER = 1;
export const TYPE_ATTRIBUTS = 2;
export const TYPE_LIRE = 3;
export const TYPE_WRITE = 4; // F2 — `Write` header, the payload carries the bytes
export const TYPE_CREATE = 5; // F2 — `Create` header, empty payload
export const TYPE_RENOMMER = 7; // F3 — `Renommer` header, empty payload
export const TYPE_DELETE = 8; // F3 — `Delete` header, empty payload

// Bridge → browser announcements — they expect NOTHING.
//
// 🔴 THIRD FAMILY. An ANNOUNCEMENT receives no answer: no table
// entry matches it on the bridge side, and not answering it thus leaves nothing in
// flight. THE LIST OF ANNOUNCEMENTS IS CLOSED — that is what prevents this family from
// becoming the silent catch-all arm paid for four times on
// `capteur/pont_media.rs`.
export const TYPE_DUES = 6; // F2 — `Dues` header, empty payload

// BROWSER → BRIDGE announcements — they expect NOTHING.
//
// 🔴 FOURTH FAMILY, and it is the first one that goes UP. Its correlation is
// IGNORED, and that is what makes it dangerous: the bridge decodes every incoming
// frame then looks up its correlation in its table; an unknown correlation
// is DROPPED into a `tracing::debug!`, invisible under `RUST_LOG=info`. An
// upstream announcement that went through this path WOULD DO NOTHING, AND NOTHING WOULD
// SAY SO. F5 routes them BEFORE any correlation lookup, and that is the
// only reason they work. THE LIST IS CLOSED.
export const TYPE_BONJOUR = 68; // F5 — `Bonjour` header, empty payload
export const TYPE_RAFRAICHIR = 69; // F5 — EMPTY header `{}`, empty payload

// Browser → bridge answers.
export const TYPE_ENTREES = 64;
export const TYPE_META = 65;
export const TYPE_DATA = 66;
export const TYPE_FAIT = 67; // F2 — EMPTY header `{}`, empty payload
export const TYPE_ECHEC = 127;

// ✅ 7 AND 8 ARE TAKEN, AND BY THE ONE THEY WERE RESERVED FOR. *(This
// line said "RESERVED for F3".)* Numbering is thus NOT contiguous per
// family: 6 is an ANNOUNCEMENT, 7 and 8 are REQUESTS. It is the named routing of
// `client/src/fichiers/protocole.ts` that tells the family, never the value. (policy: allow-fr - file name)

/** The union of message types. */
export type TypeMessage =
    | typeof TYPE_LISTER
    | typeof TYPE_ATTRIBUTS
    | typeof TYPE_LIRE
    | typeof TYPE_WRITE
    | typeof TYPE_CREATE
    | typeof TYPE_RENOMMER
    | typeof TYPE_DELETE
    | typeof TYPE_DUES
    | typeof TYPE_BONJOUR
    | typeof TYPE_RAFRAICHIR
    | typeof TYPE_ENTREES
    | typeof TYPE_META
    | typeof TYPE_DATA
    | typeof TYPE_FAIT
    | typeof TYPE_ECHEC;

/**
 * ⚠️ Table DERIVED from the `TypeMessage` union, and that is deliberate.
 *
 * `TYPES_AGENT` (`proto/ts/control.ts:106`) is the counter-example: a list
 * written by hand, that NOTHING confronts with the `AgentControl` union it is
 * supposed to mirror. A type added to the union and forgotten in the list goes
 * unnoticed there, and the message is rejected at runtime.
 *
 * Here, `Record<TypeMessage, true>` forces the compiler to demand one entry per
 * member of the union — an omission breaks `tsc --noEmit`, not only a test.
 * The reverse holds too: an entry matching no member is
 * refused as an excess property.
 */
const TYPES_CONNUS: Readonly<Record<TypeMessage, true>> = {
    [TYPE_LISTER]: true,
    [TYPE_ATTRIBUTS]: true,
    [TYPE_LIRE]: true,
    [TYPE_WRITE]: true,
    [TYPE_CREATE]: true,
    [TYPE_RENOMMER]: true,
    [TYPE_DELETE]: true,
    [TYPE_DUES]: true,
    [TYPE_BONJOUR]: true,
    [TYPE_RAFRAICHIR]: true,
    [TYPE_ENTREES]: true,
    [TYPE_META]: true,
    [TYPE_DATA]: true,
    [TYPE_FAIT]: true,
    [TYPE_ECHEC]: true,
};

export const ALL_TYPES: readonly TypeMessage[] = Object.keys(TYPES_CONNUS).map(
    Number,
) as TypeMessage[];

/**
 * The ELEVEN failure causes, in their EXACT form on the wire.
 *
 * ⚠️ Must match character for character the `#[serde(rename_all =
 * "kebab-case")]` of `CodeEchec` on the Rust side. The two-word variants are
 * the ones that break silently — the THREE new ones from F2 are among them, and the
 * only one from F3, `repertoire-non-vide`, has THREE words.
 *
 * 🔵 `repertoire-non-vide` IS DIAGNOSTIC, and that is what sets it apart:
 * receiving it means the local machine holds entries the VM does not
 * know about — F3 deletes WITHOUT `recursive`, on purpose.
 *
 * 🔴 `disque-plein` REACHES NO WINDOWS APPLICATION: it is born from a
 * write push, hence AFTER the application has closed its handle. It
 * serves the log and the pending write counter, never an `HRESULT`.
 */
export const CODES_ECHEC = [
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
] as const;

export type CodeEchec = (typeof CODES_ECHEC)[number];

export interface Trame {
    version: number;
    type: number;
    correlation: number;
    /** The JSON header already parsed, or `undefined` if it is empty. */
    entete: unknown;
    /** Raw bytes, never encoded. */
    charge: Uint8Array;
}

/**
 * Encodes a frame whose header is ALREADY serialised.
 *
 * ⚠️ It is the EXACT twin of `proto::files::encoder`, whose Rust
 * signature takes `entete: &str`. The variant that follows, `encoder`, takes an object and
 * serialises it: handy for tests, but it lets `JSON.stringify`
 * decide the key order. The product thus goes through here, with the
 * functions of `fichiers-entetes.ts` whose order is pinned by
 * `fichiers-vectors.json` — otherwise, those functions would be pinned with no
 * caller, and the vector would guarantee nothing about what actually goes out.
 */
export function encoderTexte(
    type: number,
    correlation: number,
    enteteJson: string,
    charge?: Uint8Array,
): ArrayBuffer {
    const enteteOctets = new TextEncoder().encode(enteteJson);
    const chargeOctets = charge ?? new Uint8Array(0);
    const sortie = new Uint8Array(FIXED_HEADER_SIZE + enteteOctets.length + chargeOctets.length);
    const vue = new DataView(sortie.buffer);
    sortie[0] = FILES_VERSION;
    sortie[1] = type;
    vue.setUint32(2, correlation >>> 0, true);
    vue.setUint32(6, enteteOctets.length, true);
    sortie.set(enteteOctets, FIXED_HEADER_SIZE);
    sortie.set(chargeOctets, FIXED_HEADER_SIZE + enteteOctets.length);
    return sortie.buffer;
}

/**
 * Encodes a frame by serialising `entete` to JSON. `undefined` produces a
 * zero-length header, which is legal.
 */
export function encoder(
    type: number,
    correlation: number,
    entete?: unknown,
    charge?: Uint8Array,
): ArrayBuffer {
    return encoderTexte(
        type,
        correlation,
        entete === undefined ? '' : JSON.stringify(entete),
        charge,
    );
}

/** Decodes a frame, or throws saying precisely why it is refused. */
export function decoder(octets: ArrayBuffer): Trame {
    const brut = new Uint8Array(octets);
    if (brut.length < FIXED_HEADER_SIZE) {
        throw new Error(
            `truncated frame: ${brut.length} bytes received, ${FIXED_HEADER_SIZE} expected`,
        );
    }
    const version = brut[0];
    if (version !== FILES_VERSION) {
        throw new Error(`unsupported files version: ${version}`);
    }
    const vue = new DataView(brut.buffer, brut.byteOffset, brut.byteLength);
    const correlation = vue.getUint32(2, true);
    const headerLength = vue.getUint32(6, true);
    const disponible = brut.length - FIXED_HEADER_SIZE;
    if (headerLength > disponible) {
        throw new Error(
            `header of ${headerLength} bytes announced, ${disponible} available`,
        );
    }
    const debutCharge = FIXED_HEADER_SIZE + headerLength;
    const charge = brut.subarray(debutCharge);
    if (charge.length > MAX_FRAME_SIZE) {
        throw new Error(`payload of ${charge.length} bytes, maximum ${MAX_FRAME_SIZE}`);
    }
    const entete =
        headerLength === 0
            ? undefined
            : JSON.parse(new TextDecoder().decode(brut.subarray(FIXED_HEADER_SIZE, debutCharge)));
    return { version, type: brut[1], correlation, entete, charge };
}
