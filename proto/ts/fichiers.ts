// Trame binaire du pont fichiers. Doit rester strictement alignée sur
// proto/src/fichiers.rs — le vecteur épinglé de fichiers.test.ts, écrit en dur
// des deux côtés, est ce qui le vérifie.
//
// Format : version u8 | type u8 | correlation u32 | longueur_entete u32 |
// entete | charge, entiers en PETIT-BOUTISTE (`setUint32(…, true)`).
//
// La charge n'est JAMAIS encodée : c'est tout l'objet du format binaire. Le
// pont d'origine sérialisait les octets par `Array.from(buffer.slice(…))`,
// soit ~4 octets transmis par octet utile, sur chaque octet de chaque lecture.

export const FICHIERS_VERSION = 1;

/**
 * Taille maximale de la CHARGE d'une trame, en octets.
 *
 * ⚠️ NON CALIBRÉE — posée, pas mesurée. Et son nom dit « trame » là où la
 * valeur borne la charge : divergence relevée dans le plan, tranchée du côté
 * qui rend le module cohérent avec `pont::decoupe`. Voir la doc du jumeau Rust.
 */
export const TAILLE_TRAME_MAX = 64 * 1024;

/** Version, type, corrélation, longueur d'en-tête. */
export const TAILLE_ENTETE_FIXE = 1 + 1 + 4 + 4;

// Requêtes pont → navigateur (v1).
export const TYPE_LISTER = 1;
export const TYPE_ATTRIBUTS = 2;
export const TYPE_LIRE = 3;
// Réponses navigateur → pont (v1).
export const TYPE_ENTREES = 64;
export const TYPE_META = 65;
export const TYPE_DONNEES = 66;
export const TYPE_ECHEC = 127;

/** L'union des types de message v1. */
export type TypeMessage =
    | typeof TYPE_LISTER
    | typeof TYPE_ATTRIBUTS
    | typeof TYPE_LIRE
    | typeof TYPE_ENTREES
    | typeof TYPE_META
    | typeof TYPE_DONNEES
    | typeof TYPE_ECHEC;

/**
 * ⚠️ Table DÉRIVÉE de l'union `TypeMessage`, et c'est délibéré.
 *
 * `TYPES_AGENT` (`proto/ts/control.ts:106`) est le contre-exemple : une liste
 * écrite à la main, que RIEN ne confronte à l'union `AgentControl` qu'elle est
 * censée refléter. Un type ajouté à l'union et oublié dans la liste y passe
 * inaperçu, et le message est rejeté à l'exécution.
 *
 * Ici, `Record<TypeMessage, true>` force le compilateur à exiger une entrée par
 * membre de l'union — un oubli casse `tsc --noEmit`, pas seulement un test.
 * L'inverse est vrai aussi : une entrée qui ne correspond à aucun membre est
 * refusée comme propriété en trop.
 */
const TYPES_CONNUS: Readonly<Record<TypeMessage, true>> = {
    [TYPE_LISTER]: true,
    [TYPE_ATTRIBUTS]: true,
    [TYPE_LIRE]: true,
    [TYPE_ENTREES]: true,
    [TYPE_META]: true,
    [TYPE_DONNEES]: true,
    [TYPE_ECHEC]: true,
};

export const TOUS_LES_TYPES: readonly TypeMessage[] = Object.keys(TYPES_CONNUS).map(
    Number,
) as TypeMessage[];

/**
 * Les sept causes d'échec, dans leur forme EXACTE sur le fil.
 *
 * ⚠️ Doit correspondre caractère pour caractère au `#[serde(rename_all =
 * "kebab-case")]` de `CodeEchec` côté Rust. Les variantes à deux mots sont
 * celles qui se cassent en silence.
 */
export const CODES_ECHEC = [
    'introuvable',
    'chemin-introuvable',
    'acces-refuse',
    'protege-en-ecriture',
    'non-supporte',
    'trop-grand',
    'interne',
] as const;

export type CodeEchec = (typeof CODES_ECHEC)[number];

export interface Trame {
    version: number;
    type: number;
    correlation: number;
    /** L'en-tête JSON déjà analysé, ou `undefined` s'il est vide. */
    entete: unknown;
    /** Octets bruts, jamais encodés. */
    charge: Uint8Array;
}

/**
 * Encode une trame. `entete` est sérialisé en JSON ; `undefined` produit un
 * en-tête de longueur nulle, qui est licite.
 */
export function encoder(
    type: number,
    correlation: number,
    entete?: unknown,
    charge?: Uint8Array,
): ArrayBuffer {
    const enteteOctets =
        entete === undefined ? new Uint8Array(0) : new TextEncoder().encode(JSON.stringify(entete));
    const chargeOctets = charge ?? new Uint8Array(0);
    const sortie = new Uint8Array(TAILLE_ENTETE_FIXE + enteteOctets.length + chargeOctets.length);
    const vue = new DataView(sortie.buffer);
    sortie[0] = FICHIERS_VERSION;
    sortie[1] = type;
    vue.setUint32(2, correlation >>> 0, true);
    vue.setUint32(6, enteteOctets.length, true);
    sortie.set(enteteOctets, TAILLE_ENTETE_FIXE);
    sortie.set(chargeOctets, TAILLE_ENTETE_FIXE + enteteOctets.length);
    return sortie.buffer;
}

/** Décode une trame, ou lève en disant précisément pourquoi elle est refusée. */
export function decoder(octets: ArrayBuffer): Trame {
    const brut = new Uint8Array(octets);
    if (brut.length < TAILLE_ENTETE_FIXE) {
        throw new Error(
            `trame tronquée : ${brut.length} octets reçus, ${TAILLE_ENTETE_FIXE} attendus`,
        );
    }
    const version = brut[0];
    if (version !== FICHIERS_VERSION) {
        throw new Error(`version de fichiers non supportée : ${version}`);
    }
    const vue = new DataView(brut.buffer, brut.byteOffset, brut.byteLength);
    const correlation = vue.getUint32(2, true);
    const longueurEntete = vue.getUint32(6, true);
    const disponible = brut.length - TAILLE_ENTETE_FIXE;
    if (longueurEntete > disponible) {
        throw new Error(
            `en-tête de ${longueurEntete} octets annoncé, ${disponible} disponibles`,
        );
    }
    const debutCharge = TAILLE_ENTETE_FIXE + longueurEntete;
    const charge = brut.subarray(debutCharge);
    if (charge.length > TAILLE_TRAME_MAX) {
        throw new Error(`charge de ${charge.length} octets, maximum ${TAILLE_TRAME_MAX}`);
    }
    const entete =
        longueurEntete === 0
            ? undefined
            : JSON.parse(new TextDecoder().decode(brut.subarray(TAILLE_ENTETE_FIXE, debutCharge)));
    return { version, type: brut[1], correlation, entete, charge };
}
