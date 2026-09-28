// The JSON headers of the file bridge frames — the TypeScript twin of
// `proto/src/fichiers/entetes.rs`.
//
// ⚠️ THE SPLIT IS THE SAME ON BOTH SIDES: the frame in `files.{rs,ts}`,
// the headers here and in `fichiers/entetes.rs`. An asymmetric split
// would make the pairing harder to read than to write. This file is
// separate from `fichiers.ts` for the same reason its Rust twin is separate from
// `fichiers.rs` — and because the two together would cross the 250-line
// gate the plan arms on `fichiers.ts`.
//
// 🔴 WHAT CATCHES A RENAME: `proto/fichiers-vectors.json`, read by
// `fichiers-entetes.test.ts` HERE and by `fichiers/entetes/tests.rs` OVER THERE.
// As long as these shapes lived in the agent alone, this file would have had to
// reproduce them by hand, and a field renamed on one side would have broken the bridge without
// breaking a single test — the exact pattern of `TYPES_AGENT` (`control.ts`) and of
// the `battement-recu` variant that stayed green over fifty tests.
//
// ⚠️ `position`, `size` and `length` are 64-bit integers on the Rust side and
// `number` here: beyond 2^53 the two implementations would diverge
// silently. The bridge serves a local directory opened through the File System Access
// API, where a 9-petabyte file does not exist; the bound is named, not
// guarded.
//
// ⚠️ *This sentence said "F1 is read-only": F2 opened writing,
// and the bound now also applies to the `position` of a written chunk and
// to the `octets` of a pending write. It is no more guarded than before.*

import { CODES_ECHEC, type CodeEchec } from './fichiers';

/** The header of `TYPE_LISTER` and of `TYPE_ATTRIBUTS`. */
export interface EnteteChemin {
    /** Logical path, components separated by `/`. Empty = the root. */
    chemin: string;
}

/** The header of `TYPE_LIRE`. The range is **one chunk**, never the file. */
export interface EnteteLire {
    chemin: string;
    position: number;
    longueur: number;
}

/** A directory entry, in the `TYPE_ENTREES` answer. */
export interface EntreeJson {
    nom: string;
    repertoire: boolean;
    taille: number;
    /** `File.lastModified`: milliseconds since the Unix epoch, **signed**. */
    modifie: number;
}

/** The header of `TYPE_ENTREES`. **Empty** binary payload. */
export interface EnteteEntrees {
    entrees: EntreeJson[];
}

/**
 * The header of `TYPE_META`. **Empty** binary payload.
 *
 * 🔴 `nom` IS THE CANONICAL NAME — THE ONE THAT IS STORED, never the one
 * the application typed (F3). Empty for the ROOT, which has no name.
 */
export interface EnteteMeta {
    nom: string;
    repertoire: boolean;
    taille: number;
    modifie: number;
}

/** The header of `TYPE_DATA`. **The payload carries the bytes.** */
export interface DataHeader {
    position: number;
    longueur: number;
}

/**
 * The header of `TYPE_WRITE`. **The payload carries the bytes.**
 *
 * ⚠️ `premier` and `last` CANNOT BE DEDUCED from `position` and
 * `length`: it is `premier` that commands opening the stream WITHOUT
 * `keepExistingData`, and `last` that triggers the `close()`, hence the
 * COMMIT.
 */
export interface WriteHeader {
    chemin: string;
    position: number;
    longueur: number;
    premier: boolean;
    dernier: boolean;
}

/** The header of `TYPE_CREATE`. **Empty** binary payload. */
export interface CreateHeader {
    chemin: string;
    repertoire: boolean;
}

/**
 * The header of `TYPE_RENOMMER`. **Empty** binary payload.
 *
 * 🔴 THE ORDER OF THE TWO FIELDS IS THE DIRECTION OF THE OPERATION, AND GETTING IT WRONG
 * DESTROYS. `de` is the source, `vers` the destination. Swapping the two would
 * produce no error: the rename would take place, backwards.
 *
 * ⚠️ `repertoire` is CARRIED, never rediscovered: it is the `isdirectory`
 * the ProjFS callback receives from the system. The browser would ask for it again at
 * the cost of a round trip, and would get it wrong for an entry that vanished in between.
 */
export interface EnteteRenommer {
    de: string;
    vers: string;
    repertoire: boolean;
}

/**
 * The header of `TYPE_DELETE`. **Empty** binary payload.
 *
 * ⚠️ Deletion is NOT recursive on the browser side, against the letter of
 * spec §3.5. A gesture in the VM must not trigger a recursive
 * destruction of the local machine's disk, on the strength of a mirror no proof
 * says is up to date.
 */
export interface DeleteHeader {
    chemin: string;
    repertoire: boolean;
}

/** A PENDING write: bytes that live on the VM and not yet here. */
export interface Due {
    chemin: string;
    octets: number;
}

/**
 * The header of `TYPE_DUES`. **Empty** binary payload.
 *
 * ⚠️ It is an ANNOUNCEMENT: it expects NO answer.
 */
export interface EnteteDues {
    dues: Due[];
    /**
     * **F5** — true when the bridge has pending writes and **refuses to
     * push them**, the browser having announced a root whose name differs from
     * the remembered one (`Bonjour`, spec §6.4 case 2).
     *
     * 🔴 **REQUIRED FIELD**, like every field of this module. A default of `false`
     * would mean "the bridge pushes", that is the opposite of what `Bonjour`
     * exists to prevent. **A default must fall on the safe side, or not
     * exist.**
     */
    retenues: boolean;
}

/**
 * The header of `TYPE_BONJOUR`. **Empty** binary payload.
 *
 * ⚠️ It is an ANNOUNCEMENT, and it goes UP: from the browser to the bridge, without
 * the latter having asked for it, and its correlation is IGNORED.
 *
 * ⚠️ `racine` is a HINT, never a proof: `isSameEntry()` compares two
 * LIVE handles, never a handle to a memory (spec §6.4 case 2). Two
 * same-named directories on two different disks would pass for one.
 * v1 compares the name for lack of anything better.
 */
export interface EnteteBonjour {
    racine: string;
    forcer: boolean;
}

/* ⚠️ `TYPE_RAFRAICHIR` has NO header of its own: its frame carries `{}`, like
   `TYPE_FAIT`. Giving it an empty interface would make a shape to pin that
   pins nothing, and a shared vector that cannot break. */

/** The header of `TYPE_ECHEC`. */
export interface EnteteEchec {
    code: CodeEchec;
}

/* ── ENCODING ─────────────────────────────────────────────────────────────
   ⚠️ THE KEY ORDER IS THAT OF THE RUST DECLARATION, and it matters: the
   vectors freeze the exact string, `JSON.stringify` follows insertion order,
   `serde_json` that of the declaration. Reordering a literal here turns the
   vector red — that is intended. */

export function encodeChemin(chemin: string): string {
    return JSON.stringify({ chemin } satisfies EnteteChemin);
}

export function encodeLire(chemin: string, position: number, length: number): string {
    return JSON.stringify({ chemin, position, longueur: length } satisfies EnteteLire);
}

export function encodeEntrees(entrees: EntreeJson[]): string {
    // Each entry is rebuilt field by field: an object coming from
    // the caller could carry extra keys, or keys in another order.
    return JSON.stringify({
        entrees: entrees.map((e) => ({
            nom: e.nom,
            repertoire: e.repertoire,
            taille: e.taille,
            modifie: e.modifie,
        })),
    } satisfies EnteteEntrees);
}

export function encodeMeta(
    nom: string,
    repertoire: boolean,
    size: number,
    modified: number,
): string {
    // ⚠️ THE KEY ORDER IS THAT OF THE RUST DECLARATION, and the vector
    // freezes it: `nom` comes FIRST.
    return JSON.stringify({ nom, repertoire, taille: size, modifie: modified } satisfies EnteteMeta);
}

export function encodeData(position: number, length: number): string {
    return JSON.stringify({ position, longueur: length } satisfies DataHeader);
}

export function encodeEchec(code: CodeEchec): string {
    return JSON.stringify({ code } satisfies EnteteEchec);
}

export function encodeWrite(
    chemin: string,
    position: number,
    length: number,
    premier: boolean,
    last: boolean,
): string {
    return JSON.stringify({
        chemin,
        position,
        longueur: length,
        premier,
        dernier: last,
    } satisfies WriteHeader);
}

export function encodeCreate(chemin: string, repertoire: boolean): string {
    return JSON.stringify({ chemin, repertoire } satisfies CreateHeader);
}

export function encodeRenommer(de: string, vers: string, repertoire: boolean): string {
    return JSON.stringify({ de, vers, repertoire } satisfies EnteteRenommer);
}

export function encodeDelete(chemin: string, repertoire: boolean): string {
    return JSON.stringify({ chemin, repertoire } satisfies DeleteHeader);
}

export function encodeBonjour(racine: string, forcer: boolean): string {
    return JSON.stringify({ racine, forcer } satisfies EnteteBonjour);
}

export function encodeDues(dues: Due[], retenues: boolean): string {
    // Each pending write is rebuilt field by field, like `encodeEntrees`: an
    // object coming from the caller could carry extra keys, or keys in another
    // order — and the order is what the vector freezes.
    return JSON.stringify({
        dues: dues.map((d) => ({ chemin: d.chemin, octets: d.octets })),
        retenues,
    } satisfies EnteteDues);
}

/* ── PARSING ──────────────────────────────────────────────────────────────
   No field is filled in by default: an incomplete header is REJECTED. The
   version doctrine of `control`, applied to headers — and the Rust twin
   has no `#[serde(default)]` for the same reason.

   The TYPE is checked as much as the PRESENCE: a `size` given as a string would pass
   a presence check and would give ProjFS an absurd file size. */

function objet(value: unknown, forme: string): Record<string, unknown> {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
        throw new Error(`en-tête ${forme} : objet attendu, reçu ${typeof value}`);
    }
    return value as Record<string, unknown>;
}

function chain(o: Record<string, unknown>, cle: string, forme: string): string {
    const v = o[cle];
    if (typeof v !== 'string') {
        throw new Error(`en-tête ${forme} : champ « ${cle} » absent ou non textuel`);
    }
    return v;
}

function entier(o: Record<string, unknown>, cle: string, forme: string): number {
    const v = o[cle];
    if (typeof v !== 'number' || !Number.isInteger(v)) {
        throw new Error(`en-tête ${forme} : champ « ${cle} » absent ou non entier`);
    }
    return v;
}

function booleen(o: Record<string, unknown>, cle: string, forme: string): boolean {
    const v = o[cle];
    if (typeof v !== 'boolean') {
        throw new Error(`en-tête ${forme} : champ « ${cle} » absent ou non booléen`);
    }
    return v;
}

export function parseChemin(brut: unknown): EnteteChemin {
    const o = objet(brut, 'Chemin');
    return { chemin: chain(o, 'chemin', 'Chemin') };
}

export function parseLire(brut: unknown): EnteteLire {
    const o = objet(brut, 'Lire');
    return {
        chemin: chain(o, 'chemin', 'Lire'),
        position: entier(o, 'position', 'Lire'),
        longueur: entier(o, 'longueur', 'Lire'),
    };
}

export function parseEntrees(brut: unknown): EnteteEntrees {
    const o = objet(brut, 'Entrees');
    const list = o.entrees;
    if (!Array.isArray(list)) {
        throw new Error('en-tête Entrees : champ « entrees » absent ou non tableau');
    }
    return {
        entrees: list.map((e) => {
            const item = objet(e, 'EntreeJson');
            return {
                nom: chain(item, 'nom', 'EntreeJson'),
                repertoire: booleen(item, 'repertoire', 'EntreeJson'),
                taille: entier(item, 'taille', 'EntreeJson'),
                modifie: entier(item, 'modifie', 'EntreeJson'),
            };
        }),
    };
}

export function parseMeta(brut: unknown): EnteteMeta {
    const o = objet(brut, 'Meta');
    return {
        nom: chain(o, 'nom', 'Meta'),
        repertoire: booleen(o, 'repertoire', 'Meta'),
        taille: entier(o, 'taille', 'Meta'),
        modifie: entier(o, 'modifie', 'Meta'),
    };
}

export function parseData(brut: unknown): DataHeader {
    const o = objet(brut, 'Donnees');
    return {
        position: entier(o, 'position', 'Donnees'),
        longueur: entier(o, 'longueur', 'Donnees'),
    };
}

export function parseWrite(brut: unknown): WriteHeader {
    const o = objet(brut, 'Ecrire');
    return {
        chemin: chain(o, 'chemin', 'Ecrire'),
        position: entier(o, 'position', 'Ecrire'),
        longueur: entier(o, 'longueur', 'Ecrire'),
        // 🔴 BOTH flags are required. Without `premier`, the stream would open
        // with `keepExistingData` and a file rewritten shorter would keep its
        // tail of bytes — the EXACT defect of the old bridge. Without `last`, the
        // `close()` would never come and nothing would ever be committed.
        premier: booleen(o, 'premier', 'Ecrire'),
        dernier: booleen(o, 'dernier', 'Ecrire'),
    };
}

export function parseCreate(brut: unknown): CreateHeader {
    const o = objet(brut, 'Creer');
    return {
        chemin: chain(o, 'chemin', 'Creer'),
        repertoire: booleen(o, 'repertoire', 'Creer'),
    };
}

export function parseRenommer(brut: unknown): EnteteRenommer {
    const o = objet(brut, 'Renommer');
    return {
        de: chain(o, 'de', 'Renommer'),
        vers: chain(o, 'vers', 'Renommer'),
        repertoire: booleen(o, 'repertoire', 'Renommer'),
    };
}

export function parseDelete(brut: unknown): DeleteHeader {
    const o = objet(brut, 'Supprimer');
    return {
        chemin: chain(o, 'chemin', 'Supprimer'),
        repertoire: booleen(o, 'repertoire', 'Supprimer'),
    };
}

export function parseDues(brut: unknown): EnteteDues {
    const o = objet(brut, 'Dues');
    const list = o.dues;
    if (!Array.isArray(list)) {
        throw new Error('en-tête Dues : champ « dues » absent ou non tableau');
    }
    return {
        dues: list.map((d) => {
            const item = objet(d, 'Due');
            return {
                chemin: chain(item, 'chemin', 'Due'),
                octets: entier(item, 'octets', 'Due'),
            };
        }),
        retenues: booleen(o, 'retenues', 'Dues'),
    };
}

export function parseBonjour(brut: unknown): EnteteBonjour {
    const o = objet(brut, 'Bonjour');
    return {
        racine: chain(o, 'racine', 'Bonjour'),
        forcer: booleen(o, 'forcer', 'Bonjour'),
    };
}

export function parseEchec(brut: unknown): EnteteEchec {
    const o = objet(brut, 'Echec');
    const code = chain(o, 'code', 'Echec');
    // 🔴 The list is that of `CODES_ECHEC`, hence of the Rust enum: an
    // unknown code is refused rather than propagated as a free string.
    if (!(CODES_ECHEC as readonly string[]).includes(code)) {
        throw new Error(`en-tête Echec : code inconnu « ${code} »`);
    }
    return { code: code as CodeEchec };
}
