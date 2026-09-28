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
 * Typage local des cas : le JSON mélange des champs propres à chaque `forme`,
 * tous optionnels ici puisqu'aucun cas ne les porte tous. Même choix que
 * `plateforme.test.ts` et `input.test.ts`, pour la même raison.
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

/** Produit la chaîne JSON d'un cas, quelle que soit sa forme. */
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

/** Relit la chaîne JSON d'un cas par le parseur de sa forme. */
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
        // 🔴 La rouge : l'omettre. C'est la lacune que `vectors.json` traîne
        // côté Rust — `input.rs` ne vérifie jamais `doc["version"]`. Ici les
        // DEUX côtés la vérifient.
        expect(FILES_VERSION).toBe(vecteurs.version);
    });

    it('🔴 carries at least one case', () => {
        // 🔴 ANTI-TAUTOLOGIE : un fichier vide ferait passer toutes les boucles
        // ci-dessous sans rien éprouver.
        expect(cas.length).toBeGreaterThan(0);
    });

    it.each(cas)('encodes « $name » exactly like the vector', (c) => {
        expect(encoder(c)).toBe(c.json);
    });

    it.each(cas)('re-reads « $name » from the vector, without distorting it', (c) => {
        // Aller-retour : ce qui est relu doit se ré-encoder à l'identique. Un
        // champ renommé casse ici, un champ perdu aussi.
        expect(JSON.stringify(analyser(c))).toBe(c.json);
    });
});

describe('incomplete headers are rejected', () => {
    // Doctrine de version de `control` appliquée aux en-têtes : rien n'est
    // silencieusement complété. Le jumeau Rust est
    // `an_incomplete_header_is_rejected_rather_than_completed`.
    it('refuses a Meta without `modifie`', () => {
        expect(() => parseMeta({ nom: 'a', repertoire: false, taille: 1 })).toThrow(/modifie/);
    });

    it('🔴 refuses a Meta without `nom` — the CANONICAL name', () => {
        // Sans lui, le substitut serait créé sous le nom que l'application a
        // TAPÉ, et non sous celui qui existe sur le poste local.
        expect(() => parseMeta({ repertoire: false, taille: 1, modifie: 0 })).toThrow(/nom/);
    });

    it('refuses a Donnees without `longueur`', () => {
        expect(() => parseData({ position: 0 })).toThrow(/longueur/);
    });

    it('refuses a Lire without `longueur`', () => {
        expect(() => parseLire({ chemin: 'a', position: 0 })).toThrow(/longueur/);
    });

    it('refuses a field of the wrong TYPE, not only an absent field', () => {
        // Un `size` en chaîne passerait un contrôle de présence et
        // produirait une taille de fichier absurde côté ProjFS.
        expect(() => parseMeta({ nom: 'a', repertoire: false, taille: '1', modifie: 0 })).toThrow(
            /taille/,
        );
    });

    it('🔴 refuses a Renommer without `vers` — the only field whose absence DESTROYS', () => {
        // Complété en silence par une chaîne vide, il ferait renommer vers la
        // racine — ou, si l'appelant sautait sa garde, écraserait la source par
        // elle-même. C'est le seul en-tête de ce protocole dont un champ
        // manquant a une conséquence destructrice.
        expect(() => parseRenommer({ de: 'a', repertoire: false })).toThrow(/vers/);
    });

    it('refuses a Supprimer without `repertoire`', () => {
        expect(() => parseDelete({ chemin: 'a' })).toThrow(/repertoire/);
    });

    it('refuses an unknown failure code', () => {
        expect(() => parseEchec({ code: 'invented' })).toThrow(/code/);
    });

    it('🔴 refuses an Ecrire without `premier`', () => {
        // Sans `premier`, le flux s'ouvrirait avec `keepExistingData` : un
        // fichier réécrit plus court garderait sa queue d'octets, ce qui est le
        // défaut EXACT de l'ancien pont (spec §12).
        expect(() =>
            parseWrite({ chemin: 'a', position: 0, longueur: 1, dernier: true }),
        ).toThrow(/premier/);
    });

    it('🔴 refuses an Ecrire without `dernier`', () => {
        // Sans `last`, le `close()` ne viendrait jamais : rien ne serait
        // jamais commis côté poste local, et l'entrée resterait due à jamais.
        expect(() =>
            parseWrite({ chemin: 'a', position: 0, longueur: 1, premier: true }),
        ).toThrow(/dernier/);
    });

    it('refuses a Creer without `repertoire`', () => {
        expect(() => parseCreate({ chemin: 'a' })).toThrow(/repertoire/);
    });

    it('refuses an incomplete due', () => {
        expect(() => parseDues({ dues: [{ chemin: 'a' }] })).toThrow(/octets/);
        // 🔴 **F5 — l'ABSENCE de défaut, épinglée.** Un `Dues` sans `retenues`
        // complété en silence vaudrait « le pont pousse », c'est-à-dire
        // l'inverse de ce que `Bonjour` existe pour empêcher.
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
