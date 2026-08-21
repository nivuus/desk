import { describe, expect, it } from 'vitest';
import vecteurs from '../fichiers-vectors.json';
import { FICHIERS_VERSION, type CodeEchec } from './fichiers';
import {
    encodeChemin,
    encodeCreer,
    encodeDonnees,
    encodeDues,
    encodeEchec,
    encodeEcrire,
    encodeEntrees,
    encodeLire,
    encodeMeta,
    encodeRenommer,
    encodeSupprimer,
    parseChemin,
    parseCreer,
    parseDonnees,
    parseDues,
    parseEchec,
    parseEcrire,
    parseEntrees,
    parseLire,
    parseMeta,
    parseRenommer,
    parseSupprimer,
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
            return encodeDonnees(c.position!, c.longueur!);
        case 'ecrire':
            return encodeEcrire(c.chemin!, c.position!, c.longueur!, c.premier!, c.dernier!);
        case 'creer':
            return encodeCreer(c.chemin!, c.repertoire!);
        case 'renommer':
            return encodeRenommer(c.de!, c.vers!, c.repertoire!);
        case 'supprimer':
            return encodeSupprimer(c.chemin!, c.repertoire!);
        case 'dues':
            return encodeDues(c.dues!);
        case 'echec':
            return encodeEchec(c.code as CodeEchec);
        default:
            throw new Error(`forme inconnue dans les vecteurs : ${c.forme}`);
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
            return parseDonnees(brut);
        case 'ecrire':
            return parseEcrire(brut);
        case 'creer':
            return parseCreer(brut);
        case 'renommer':
            return parseRenommer(brut);
        case 'supprimer':
            return parseSupprimer(brut);
        case 'dues':
            return parseDues(brut);
        case 'echec':
            return parseEchec(brut);
        default:
            throw new Error(`forme inconnue dans les vecteurs : ${c.forme}`);
    }
}

describe('vecteurs partagés des en-têtes du pont fichiers', () => {
    it('🔴 déclare la MÊME version que le protocole', () => {
        // 🔴 La rouge : l'omettre. C'est la lacune que `vectors.json` traîne
        // côté Rust — `input.rs` ne vérifie jamais `doc["version"]`. Ici les
        // DEUX côtés la vérifient.
        expect(FICHIERS_VERSION).toBe(vecteurs.version);
    });

    it('🔴 porte au moins un cas', () => {
        // 🔴 ANTI-TAUTOLOGIE : un fichier vide ferait passer toutes les boucles
        // ci-dessous sans rien éprouver.
        expect(cas.length).toBeGreaterThan(0);
    });

    it.each(cas)('encode « $name » exactement comme le vecteur', (c) => {
        expect(encoder(c)).toBe(c.json);
    });

    it.each(cas)('relit « $name » depuis le vecteur, sans le déformer', (c) => {
        // Aller-retour : ce qui est relu doit se ré-encoder à l'identique. Un
        // champ renommé casse ici, un champ perdu aussi.
        expect(JSON.stringify(analyser(c))).toBe(c.json);
    });
});

describe('les en-têtes incomplets sont rejetés', () => {
    // Doctrine de version de `control` appliquée aux en-têtes : rien n'est
    // silencieusement complété. Le jumeau Rust est
    // `un_entete_incomplet_est_rejete_plutot_que_complete`.
    it('refuse un Meta sans `modifie`', () => {
        expect(() => parseMeta({ nom: 'a', repertoire: false, taille: 1 })).toThrow(/modifie/);
    });

    it('🔴 refuse un Meta sans `nom` — le nom CANONIQUE', () => {
        // Sans lui, le substitut serait créé sous le nom que l'application a
        // TAPÉ, et non sous celui qui existe sur le poste local.
        expect(() => parseMeta({ repertoire: false, taille: 1, modifie: 0 })).toThrow(/nom/);
    });

    it('refuse un Donnees sans `longueur`', () => {
        expect(() => parseDonnees({ position: 0 })).toThrow(/longueur/);
    });

    it('refuse un Lire sans `longueur`', () => {
        expect(() => parseLire({ chemin: 'a', position: 0 })).toThrow(/longueur/);
    });

    it('refuse un champ du mauvais TYPE, pas seulement un champ absent', () => {
        // Un `taille` en chaîne passerait un contrôle de présence et
        // produirait une taille de fichier absurde côté ProjFS.
        expect(() => parseMeta({ nom: 'a', repertoire: false, taille: '1', modifie: 0 })).toThrow(
            /taille/,
        );
    });

    it('🔴 refuse un Renommer sans `vers` — le seul champ dont l’absence DÉTRUIT', () => {
        // Complété en silence par une chaîne vide, il ferait renommer vers la
        // racine — ou, si l'appelant sautait sa garde, écraserait la source par
        // elle-même. C'est le seul en-tête de ce protocole dont un champ
        // manquant a une conséquence destructrice.
        expect(() => parseRenommer({ de: 'a', repertoire: false })).toThrow(/vers/);
    });

    it('refuse un Supprimer sans `repertoire`', () => {
        expect(() => parseSupprimer({ chemin: 'a' })).toThrow(/repertoire/);
    });

    it('refuse un code d’échec inconnu', () => {
        expect(() => parseEchec({ code: 'inventé' })).toThrow(/code/);
    });

    it('🔴 refuse un Ecrire sans `premier`', () => {
        // Sans `premier`, le flux s'ouvrirait avec `keepExistingData` : un
        // fichier réécrit plus court garderait sa queue d'octets, ce qui est le
        // défaut EXACT de l'ancien pont (spec §12).
        expect(() =>
            parseEcrire({ chemin: 'a', position: 0, longueur: 1, dernier: true }),
        ).toThrow(/premier/);
    });

    it('🔴 refuse un Ecrire sans `dernier`', () => {
        // Sans `dernier`, le `close()` ne viendrait jamais : rien ne serait
        // jamais commis côté poste local, et l'entrée resterait due à jamais.
        expect(() =>
            parseEcrire({ chemin: 'a', position: 0, longueur: 1, premier: true }),
        ).toThrow(/dernier/);
    });

    it('refuse un Creer sans `repertoire`', () => {
        expect(() => parseCreer({ chemin: 'a' })).toThrow(/repertoire/);
    });

    it('refuse une due incomplète', () => {
        expect(() => parseDues({ dues: [{ chemin: 'a' }] })).toThrow(/octets/);
    });

    it('refuse une entrée de répertoire incomplète', () => {
        expect(() => parseEntrees({ entrees: [{ nom: 'a', repertoire: false }] })).toThrow(
            /taille/,
        );
    });
});
