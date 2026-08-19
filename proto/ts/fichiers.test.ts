import { describe, expect, it } from 'vitest';
import {
    CODES_ECHEC,
    FICHIERS_VERSION,
    TAILLE_ENTETE_FIXE,
    TAILLE_TRAME_MAX,
    TOUS_LES_TYPES,
    TYPE_DONNEES,
    TYPE_ENTREES,
    TYPE_LISTER,
    TYPE_META,
    decoder,
    encoder,
} from './fichiers';

/**
 * Le vecteur épinglé, **écrit en dur ici ET dans `proto/src/fichiers/tests.rs`**.
 *
 * ⚠️ C'est la seule façon de voir ROUGE une divergence d'endianness entre les
 * deux implémentations : un aller-retour TS→TS reste vert quel que soit le
 * boutisme, du moment qu'il est le même des deux côtés du même fichier. Les
 * octets ci-dessous sont la source de vérité du format, pas une conséquence
 * du code — ils doivent rester identiques, octet pour octet, à
 * `VECTEUR_EPINGLE` côté Rust.
 *
 * version 1 | type 66 (`TYPE_DONNEES`) | corrélation 0x0A0B0C0D petit-boutiste |
 * longueur d'en-tête 2 petit-boutiste | en-tête `{}` | charge `00 FF 7F 80`.
 */
const VECTEUR_EPINGLE = new Uint8Array([
    1, 66, 0x0d, 0x0c, 0x0b, 0x0a, 2, 0, 0, 0, 0x7b, 0x7d, 0x00, 0xff, 0x7f, 0x80,
]);

function octets(t: ArrayBuffer): Uint8Array {
    return new Uint8Array(t);
}

describe('trame binaire du pont fichiers', () => {
    it('refuse une trame sans version', () => {
        // Zéro octet ne porte pas sa version : rejet, jamais complétion.
        expect(() => decoder(new ArrayBuffer(0))).toThrow(/tronqu/i);
        // …et un octet de moins que l'en-tête fixe l'est aussi.
        expect(() => decoder(new ArrayBuffer(TAILLE_ENTETE_FIXE - 1))).toThrow(/tronqu/i);
    });

    it('refuse une trame de version 2', () => {
        const trame = octets(encoder(TYPE_LISTER, 7, {}));
        trame[0] = FICHIERS_VERSION + 1;
        expect(() => decoder(trame.buffer as ArrayBuffer)).toThrow(/version/i);
    });

    it("refuse un en-tête dont la longueur déborde la trame", () => {
        const trame = octets(encoder(TYPE_ENTREES, 1, {}, new Uint8Array([1, 2, 3])));
        new DataView(trame.buffer).setUint32(6, 0xffffffff, true);
        expect(() => decoder(trame.buffer as ArrayBuffer)).toThrow(/en-tête/i);

        // Le débordement d'UN SEUL octet est refusé aussi : c'est là que vit
        // l'erreur d'inégalité stricte.
        const dun = octets(encoder(TYPE_ENTREES, 1, {}));
        new DataView(dun.buffer).setUint32(6, 3, true);
        expect(() => decoder(dun.buffer as ArrayBuffer)).toThrow(/en-tête/i);
    });

    it('conserve les octets bruts sur un aller-retour', () => {
        // 0x00 et 0xFF sont les deux octets qu'un encodage textuel abîme en
        // premier ; on passe les 256.
        const charge = new Uint8Array(256);
        for (let i = 0; i < 256; i += 1) charge[i] = i;
        const trame = decoder(encoder(TYPE_DONNEES, 0xdeadbeef, { position: 0 }, charge));
        expect(trame.version).toBe(FICHIERS_VERSION);
        expect(trame.type).toBe(TYPE_DONNEES);
        expect(trame.correlation).toBe(0xdeadbeef);
        expect(trame.entete).toEqual({ position: 0 });
        expect(Array.from(trame.charge)).toEqual(Array.from(charge));
    });

    it('accepte une charge vide et un en-tête vide', () => {
        const brut = octets(encoder(TYPE_META, 0));
        expect(brut.length).toBe(TAILLE_ENTETE_FIXE);
        const trame = decoder(brut.buffer as ArrayBuffer);
        expect(trame.entete).toBeUndefined();
        expect(trame.charge.length).toBe(0);
        expect(trame.correlation).toBe(0);
    });

    it('accepte une charge de TAILLE_TRAME_MAX et refuse un octet de plus', () => {
        // Au seuil EXACT. C'est l'inégalité stricte qui est éprouvée, pas la
        // borne en général.
        const pleine = new Uint8Array(TAILLE_TRAME_MAX).fill(0xab);
        expect(decoder(encoder(TYPE_DONNEES, 1, {}, pleine)).charge.length).toBe(TAILLE_TRAME_MAX);

        const trop = new Uint8Array(TAILLE_TRAME_MAX + 1).fill(0xab);
        expect(() => decoder(encoder(TYPE_DONNEES, 1, {}, trop))).toThrow(/charge/i);
    });

    it('décode ce que Rust a encodé — le vecteur épinglé', () => {
        // ⚠️ LE test de ce fichier. Sans lui, une divergence d'endianness entre
        // Rust et TypeScript resterait verte des deux côtés.
        const trame = decoder(VECTEUR_EPINGLE.buffer as ArrayBuffer);
        expect(trame.version).toBe(1);
        expect(trame.type).toBe(TYPE_DONNEES);
        expect(trame.correlation).toBe(0x0a0b0c0d);
        expect(trame.entete).toEqual({});
        expect(Array.from(trame.charge)).toEqual([0x00, 0xff, 0x7f, 0x80]);
        // …et l'encodeur TypeScript le REPRODUIT à l'octet près.
        expect(
            Array.from(
                octets(encoder(TYPE_DONNEES, 0x0a0b0c0d, {}, new Uint8Array([0x00, 0xff, 0x7f, 0x80]))),
            ),
        ).toEqual(Array.from(VECTEUR_EPINGLE));
    });

    it('épingle la forme des codes d\'échec sur le fil', () => {
        // ⚠️ Les variantes à DEUX MOTS sont celles qui se cassent en silence :
        // ce dépôt a laissé passer `battement-recu` verte sur cinquante tests
        // parce que rien n'épinglait ses octets. Ces sept chaînes doivent être
        // identiques, caractère pour caractère, au `#[serde(rename_all =
        // "kebab-case")]` de `CodeEchec` côté Rust.
        expect(CODES_ECHEC).toEqual([
            'introuvable',
            'chemin-introuvable',
            'acces-refuse',
            'protege-en-ecriture',
            'non-supporte',
            'trop-grand',
            'interne',
        ]);
    });

    it('ne fait chevaucher aucun type de message', () => {
        // `TOUS_LES_TYPES` est DÉRIVÉE de l'union, pas écrite à la main : c'est
        // le remède structurel au défaut de `TYPES_AGENT` (`control.ts:106`),
        // liste manuelle que rien ne confronte à son union. Ajouter un type
        // sans l'inscrire dans la table casse `tsc --noEmit`, pas seulement ce
        // test.
        expect(new Set(TOUS_LES_TYPES).size).toBe(TOUS_LES_TYPES.length);
        expect(TOUS_LES_TYPES).toContain(TYPE_LISTER);
        expect(TOUS_LES_TYPES).toContain(TYPE_DONNEES);
    });
});
