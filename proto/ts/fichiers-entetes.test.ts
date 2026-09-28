import { describe, expect, it } from 'vitest';
import vecteurs from '../fichiers-vectors.json';
import { FILES_VERSION, type CodeEchec } from './fichiers';
import {
    encodeChemin,
    encodeCreate,
    encodeData,
    encodeBonjour,
    encodeDues,
    encodeEchec,
    encodeWrite,
    encodeEntrees,
    encodeLire,
    encodeMeta,
    encodeRenommer,
    encodeDelete,
    parseChemin,
    parseCreate,
    parseData,
    parseBonjour,
    parseDues,
    parseEchec,
    parseWrite,
    parseEntrees,
    parseLire,
    parseMeta,
    parseRenommer,
    parseDelete,
    type Due,
    type EntreeJson,
} from './fichiers-entetes';

/**
 * Local typing of the cases: the JSON mixes fields specific to each `forme`,
 * all optional here since no case carries them all. Same choice as
 * `plateforme.test.ts` and `input.test.ts`, for the same reason.
 */
interface CasVecteur {
    name: string;
    forme: string;
    json: string;
    chemin?: string;
    position?: number;
    longueur?: number;
    entrees?: EntreeJson[];
    repertoire?: boolean;
    taille?: number;
    modifie?: number;
    code?: string;
    nom?: string;
    premier?: boolean;
    dernier?: boolean;
    dues?: Due[];
    retenues?: boolean;
    racine?: string;
    forcer?: boolean;
    de?: string;
    vers?: string;
}

const cas: CasVecteur[] = vecteurs.cases as CasVecteur[];

/** Produces the JSON string of a case, whatever its shape. */
function encoder(c: CasVecteur): string {
    switch (c.forme) {
        case 'chemin':
            return encodeChemin(c.chemin!);
        case 'lire':
            return encodeLire(c.chemin!, c.position!, c.longueur!);
        case 'entrees':
            return encodeEntrees(c.entrees!);
        case 'meta':
            return encodeMeta(c.nom!, c.repertoire!, c.taille!, c.modifie!);
        case 'donnees':
            return encodeData(c.position!, c.longueur!);
        case 'ecrire':
            return encodeWrite(c.chemin!, c.position!, c.longueur!, c.premier!, c.dernier!);
        case 'creer':
            return encodeCreate(c.chemin!, c.repertoire!);
        case 'renommer':
            return encodeRenommer(c.de!, c.vers!, c.repertoire!);
        case 'supprimer':
            return encodeDelete(c.chemin!, c.repertoire!);
        case 'dues':
            return encodeDues(c.dues!, c.retenues!);
        case 'bonjour':
            return encodeBonjour(c.racine!, c.forcer!);
        case 'echec':
            return encodeEchec(c.code as CodeEchec);
        default:
            throw new Error(`unknown shape in the vectors: ${c.forme}`);
    }
}

/** Reads the JSON string of a case back through the parser of its shape. */
function analyser(c: CasVecteur): unknown {
    const brut: unknown = JSON.parse(c.json);
    switch (c.forme) {
        case 'chemin':
            return parseChemin(brut);
        case 'lire':
            return parseLire(brut);
        case 'entrees':
            return parseEntrees(brut);
        case 'meta':
            return parseMeta(brut);
        case 'donnees':
            return parseData(brut);
        case 'ecrire':
            return parseWrite(brut);
        case 'creer':
            return parseCreate(brut);
        case 'renommer':
            return parseRenommer(brut);
        case 'supprimer':
            return parseDelete(brut);
        case 'dues':
            return parseDues(brut);
        case 'bonjour':
            return parseBonjour(brut);
        case 'echec':
            return parseEchec(brut);
        default:
            throw new Error(`unknown shape in the vectors: ${c.forme}`);
    }
}

describe('shared vectors of the file bridge headers', () => {
    it('🔴 declares the SAME version as the protocol', () => {
        // 🔴 The red: omitting it. It is the gap `vectors.json` drags along
        // on the Rust side — `input.rs` never checks `doc["version"]`. Here
        // BOTH sides check it.
        expect(FILES_VERSION).toBe(vecteurs.version);
    });

    it('🔴 carries at least one case', () => {
        // 🔴 ANTI-TAUTOLOGY: an empty file would make all the loops
        // below pass without testing anything.
        expect(cas.length).toBeGreaterThan(0);
    });

    it.each(cas)('encodes « $name » exactly like the vector', (c) => {
        expect(encoder(c)).toBe(c.json);
    });

    it.each(cas)('re-reads « $name » from the vector, without distorting it', (c) => {
        // Round trip: what is read back must re-encode identically. A
        // renamed field breaks here, a lost field too.
        expect(JSON.stringify(analyser(c))).toBe(c.json);
    });
});

describe('incomplete headers are rejected', () => {
    // `control`'s version doctrine applied to headers: nothing is
    // silently completed. The Rust twin is
    // `an_incomplete_header_is_rejected_rather_than_completed`.
    it('refuses a Meta without `modifie`', () => {
        expect(() => parseMeta({ nom: 'a', repertoire: false, taille: 1 })).toThrow(/modifie/);
    });

    it('🔴 refuses a Meta without `nom` — the CANONICAL name', () => {
        // Without it, the placeholder would be created under the name the application
        // TYPED, and not under the one that exists on the local machine.
        expect(() => parseMeta({ repertoire: false, taille: 1, modifie: 0 })).toThrow(/nom/);
    });

    it('refuses a Donnees without `longueur`', () => {
        expect(() => parseData({ position: 0 })).toThrow(/longueur/);
    });

    it('refuses a Lire without `longueur`', () => {
        expect(() => parseLire({ chemin: 'a', position: 0 })).toThrow(/longueur/);
    });

    it('refuses a field of the wrong TYPE, not only an absent field', () => {
        // A `size` as a string would pass a presence check and
        // would produce an absurd file size on the ProjFS side.
        expect(() => parseMeta({ nom: 'a', repertoire: false, taille: '1', modifie: 0 })).toThrow(
            /taille/,
        );
    });

    it('🔴 refuses a Renommer without `vers` — the only field whose absence DESTROYS', () => {
        // Silently completed with an empty string, it would rename to the
        // root — or, if the caller skipped its guard, would overwrite the source with
        // itself. It is the only header of this protocol whose missing
        // field has a destructive consequence.
        expect(() => parseRenommer({ de: 'a', repertoire: false })).toThrow(/vers/);
    });

    it('refuses a Supprimer without `repertoire`', () => {
        expect(() => parseDelete({ chemin: 'a' })).toThrow(/repertoire/);
    });

    it('refuses an unknown failure code', () => {
        expect(() => parseEchec({ code: 'invented' })).toThrow(/code/);
    });

    it('🔴 refuses an Ecrire without `premier`', () => {
        // Without `premier`, the stream would open with `keepExistingData`: a
        // file rewritten shorter would keep its tail of bytes, which is the
        // EXACT defect of the old bridge (spec §12).
        expect(() =>
            parseWrite({ chemin: 'a', position: 0, longueur: 1, dernier: true }),
        ).toThrow(/premier/);
    });

    it('🔴 refuses an Ecrire without `dernier`', () => {
        // Without `last`, the `close()` would never come: nothing would
        // ever be committed on the local machine side, and the entry would stay due forever.
        expect(() =>
            parseWrite({ chemin: 'a', position: 0, longueur: 1, premier: true }),
        ).toThrow(/dernier/);
    });

    it('refuses a Creer without `repertoire`', () => {
        expect(() => parseCreate({ chemin: 'a' })).toThrow(/repertoire/);
    });

    it('refuses an incomplete due', () => {
        expect(() => parseDues({ dues: [{ chemin: 'a' }] })).toThrow(/octets/);
        // 🔴 **F5 — the ABSENCE of a default, pinned.** A `Dues` without `retenues`
        // silently completed would mean "the bridge pushes", that is,
        // the opposite of what `Bonjour` exists to prevent.
        expect(() => parseDues({ dues: [] })).toThrow(/retenues/);
        expect(() => parseBonjour({ racine: 'Documents' })).toThrow(/forcer/);
        expect(() => parseBonjour({ forcer: false })).toThrow(/racine/);
    });

    it('refuses an incomplete directory entry', () => {
        expect(() => parseEntrees({ entrees: [{ nom: 'a', repertoire: false }] })).toThrow(
            /taille/,
        );
    });
});
