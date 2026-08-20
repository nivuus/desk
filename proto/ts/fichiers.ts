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

// Requêtes pont → navigateur — elles ATTENDENT une réponse.
export const TYPE_LISTER = 1;
export const TYPE_ATTRIBUTS = 2;
export const TYPE_LIRE = 3;
export const TYPE_ECRIRE = 4; // F2 — en-tête `Ecrire`, la charge porte les octets
export const TYPE_CREER = 5; // F2 — en-tête `Creer`, charge vide

// Annonces pont → navigateur — elles n'attendent RIEN.
//
// 🔴 TROISIÈME FAMILLE. Une ANNONCE ne reçoit aucune réponse : aucune entrée de
// table ne lui correspond côté pont, et n'y pas répondre ne laisse donc rien en
// vol. LA LISTE DES ANNONCES EST CLOSE — c'est ce qui empêche cette famille de
// devenir le bras fourre-tout silencieux payé quatre fois sur
// `capteur/pont_media.rs`.
export const TYPE_DUES = 6; // F2 — en-tête `Dues`, charge vide

// Réponses navigateur → pont.
export const TYPE_ENTREES = 64;
export const TYPE_META = 65;
export const TYPE_DONNEES = 66;
export const TYPE_FAIT = 67; // F2 — en-tête VIDE `{}`, charge vide
export const TYPE_ECHEC = 127;

// ⚠️ 7 et 8 sont RÉSERVÉS à F3 (`TYPE_RENOMMER`, `TYPE_SUPPRIMER`).

/** L'union des types de message. */
export type TypeMessage =
    | typeof TYPE_LISTER
    | typeof TYPE_ATTRIBUTS
    | typeof TYPE_LIRE
    | typeof TYPE_ECRIRE
    | typeof TYPE_CREER
    | typeof TYPE_DUES
    | typeof TYPE_ENTREES
    | typeof TYPE_META
    | typeof TYPE_DONNEES
    | typeof TYPE_FAIT
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
    [TYPE_ECRIRE]: true,
    [TYPE_CREER]: true,
    [TYPE_DUES]: true,
    [TYPE_ENTREES]: true,
    [TYPE_META]: true,
    [TYPE_DONNEES]: true,
    [TYPE_FAIT]: true,
    [TYPE_ECHEC]: true,
};

export const TOUS_LES_TYPES: readonly TypeMessage[] = Object.keys(TYPES_CONNUS).map(
    Number,
) as TypeMessage[];

/**
 * Les DIX causes d'échec, dans leur forme EXACTE sur le fil.
 *
 * ⚠️ Doit correspondre caractère pour caractère au `#[serde(rename_all =
 * "kebab-case")]` de `CodeEchec` côté Rust. Les variantes à deux mots sont
 * celles qui se cassent en silence — et les TROIS neuves de F2 en sont.
 *
 * 🔴 `disque-plein` N'ATTEINT AUCUNE APPLICATION WINDOWS : il naît d'une
 * poussée d'écriture, donc APRÈS que l'application a refermé son handle. Il
 * sert au journal et au compteur d'écritures dues, jamais à un `HRESULT`.
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
 * Encode une trame dont l'en-tête est DÉJÀ sérialisé.
 *
 * ⚠️ C'est le jumeau EXACT de `proto::fichiers::encoder`, dont la signature
 * Rust prend `entete: &str`. La variante qui suit, `encoder`, prend un objet et
 * le sérialise : commode pour les tests, mais elle laisse `JSON.stringify`
 * décider de l'ordre des clés. Le produit passe donc par ici, avec les
 * fonctions de `fichiers-entetes.ts` dont l'ordre est épinglé par
 * `fichiers-vectors.json` — autrement, ces fonctions seraient épinglées sans
 * appelant, et le vecteur ne garantirait rien de ce qui part réellement.
 */
export function encoderTexte(
    type: number,
    correlation: number,
    enteteJson: string,
    charge?: Uint8Array,
): ArrayBuffer {
    const enteteOctets = new TextEncoder().encode(enteteJson);
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

/**
 * Encode une trame en sérialisant `entete` en JSON. `undefined` produit un
 * en-tête de longueur nulle, qui est licite.
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
