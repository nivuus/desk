// Le jeton d'accès : un JWT HS256 écrit avec `node:crypto` et RIEN d'autre.
//
// 🔴 AUCUNE DÉPENDANCE, et c'est une propriété MESURÉE, pas une intention.
// Le 19 août 2026, sur ce Node (v24.9.0) :
//     createHmac('sha256', secret).update(`${h}.${c}`).digest('base64url')
// rend une signature de 43 caractères, et la charge se relit par
// `Buffer.from(…, 'base64url')`. `jsonwebtoken` n'apporterait donc rien qu'un
// paquet de plus, et le contrôle `n'ajoute aucune dépendance de production
// hors l'allow-list` (`base/pilote.test.ts`) est le témoin de cette propriété.
//
// 🔴 CE MODULE EST PUR : l'horloge est un PARAMÈTRE, jamais `Date.now()` lu
// ici. C'est la règle du dépôt (`depot/session.ts`, `signaling/ice.ts`), et
// c'est ce qui rend l'expiration éprouvable sur trois instants distincts au
// lieu d'être inerte.
//
// ⚠️ DIVERGENCE DÉLIBÉRÉE AVEC LA RFC 7519 : `exp` est ici en
// MILLISECONDES, quand le standard le veut en secondes. La raison est qu'il
// n'y ait qu'UNE SEULE unité de temps dans tout ce paquet — la base écrit des
// `Date.now()` en millisecondes, le vivier de jetons aussi, et deux unités
// dans un même service se confondent tôt ou tard. Le jeton n'est lu que par
// ce service, jamais par un tiers ; le jour où il le serait, c'est cette
// décision qu'il faudrait rouvrir, et non la contourner. Sans ce paragraphe,
// le prochain lecteur croira à un bug.

// 🔴 LE CLAIM DE TYPE (P3) : un jeton d'agent et un jeton humain sont signes
// par le MEME secret. Sans un claim qui les distingue, ils sont
// interchangeables dans les deux sens -- voir `TypeSujet` ci-dessous, ou les
// deux confusions sont nommees. Le claim s'appelle `sty` et vit dans la
// CHARGE : voir `CLAIM_TYPE`.

import { createHmac, timingSafeEqual } from 'node:crypto';

export type MotifJeton = 'forme' | 'algorithme' | 'signature' | 'expire';

/// Ce qu'EST le sujet d'un jeton : un humain, ou un agent de VM.
///
/// 🔴 POURQUOI CE CLAIM EXISTE : les deux jetons sont signes par le MEME
/// secret (`PLATEFORME_SECRET_JETON`) et portent la meme charge `{sub, exp}`.
/// Sans lui, ils sont litteralement INTERCHANGEABLES, dans les deux sens et
/// les deux sont graves :
///   - un jeton HUMAIN vole ouvrirait un role `agent`, donc un `ice-config`
///     sur toute session dont il prefixerait le nom ;
///   - un jeton d'AGENT ouvrirait un role `client`, contournant l'appartenance
///     de session que P2 a posee (`signaling/propriete.ts`).
/// La garde exige donc `agent` pour le role `agent`, et non-`agent` pour le
/// role `client`.
export type TypeSujet = 'utilisateur' | 'agent';

export type VerdictJeton =
    | { ok: true; sujet: string; type: TypeSujet }
    | { ok: false; motif: MotifJeton };

/// 10 minutes. ⚠️ NON CALIBRÉE : aucune mesure ne l'a jugée, elle rejoint la
/// liste des constantes non calibrées du dépôt. Elle est courte parce que le
/// rafraîchissement rotatif (`depot/jeton.ts`) en porte la contrepartie.
export const DUREE_JETON_ACCES_MS = 600_000;

/// 32 caractères. ⚠️ NON CALIBRÉE de la même façon — c'est la longueur d'un
/// secret de 256 bits en hexadécimal réduit de moitié, pas un seuil mesuré.
export const LONGUEUR_SECRET_MIN = 32;

const ALGORITHME = 'HS256';

/// Le nom du claim de type DANS LA CHARGE.
///
/// ⚠️ IL S'APPELLE `sty` ET NON `typ`, ET CE N'EST PAS UN CAPRICE : `typ` est
/// deja employe par l'EN-TETE JWT (`{ alg: ALGORITHME, typ: 'JWT' }`,
/// ci-dessous), ou il designe le type du JETON et vaut toujours `JWT`. Deux
/// champs homonymes de sens differents dans le meme jeton egareraient tout
/// relecteur, et le premier qui confondrait les deux ecrirait un controle qui
/// ne peut pas echouer. `sty` = « sujet-type ».
///
/// 🔴 IL VIT DANS LA CHARGE, QUI EST SIGNEE — jamais dans l'en-tete, qui ne
/// l'est PAS. Deriver un comportement d'une donnee non signee est exactement
/// la confusion d'algorithme contre laquelle `alg` est compare plus bas.
const CLAIM_TYPE = 'sty';

/// L'absence du claim vaut `utilisateur`.
///
/// 🔴 C'est ce qui laisse EN VOL les jetons emis par P2, qui n'en portent
/// aucun : P3 ajoute la capacite de DIRE `agent`, il n'invalide pas ce qui
/// existe. Corollaire retenu ici : un jeton `utilisateur` ne l'ECRIT PAS non
/// plus, ce qui garde le format sur le fil identique a celui de P2 -- un seul
/// encodage, donc un seul chemin de lecture.
const TYPE_PAR_DEFAUT: TypeSujet = 'utilisateur';

const TYPES_CONNUS: readonly TypeSujet[] = ['utilisateur', 'agent'];

function encoder(valeur: unknown): string {
    return Buffer.from(JSON.stringify(valeur), 'utf8').toString('base64url');
}

function signature(tete: string, secret: string): string {
    return createHmac('sha256', secret).update(tete).digest('base64url');
}

/// Signe un jeton pour `sujet`. LÈVE sur un secret trop court : un service qui
/// démarrerait avec un secret devinable ne serait pas authentifié du tout.
export function signer(
    sujet: string,
    secret: string,
    maintenant: number,
    dureeMs: number = DUREE_JETON_ACCES_MS,
    type: TypeSujet = TYPE_PAR_DEFAUT,
): string {
    if (secret.length < LONGUEUR_SECRET_MIN) {
        throw new Error(
            `secret de signature trop court : ${secret.length} caractères, ${LONGUEUR_SECRET_MIN} au moins sont exigés`,
        );
    }
    const tete = `${encoder({ alg: ALGORITHME, typ: 'JWT' })}.${encoder({
        sub: sujet,
        // En MILLISECONDES — voir la divergence déclarée en tête de fichier.
        exp: maintenant + dureeMs,
        // Le claim n'est ECRIT que s'il dit autre chose que le defaut : voir
        // `TYPE_PAR_DEFAUT`. Un `utilisateur` explicite et un jeton de P2 sont
        // ainsi le MEME jeton, octet pour octet.
        ...(type === TYPE_PAR_DEFAUT ? {} : { [CLAIM_TYPE]: type }),
    })}`;
    return `${tete}.${signature(tete, secret)}`;
}

function estObjet(valeur: unknown): valeur is Record<string, unknown> {
    return typeof valeur === 'object' && valeur !== null && !Array.isArray(valeur);
}

/// Vérifie un jeton. Rend TOUJOURS un verdict, jamais une exception : le
/// `jeton` vient du réseau, et un `JSON.parse` qui lèverait ferait répondre
/// 500 là où il faut répondre 401.
export function verifierJeton(jeton: unknown, secret: string, maintenant: number): VerdictJeton {
    if (typeof jeton !== 'string') return { ok: false, motif: 'forme' };

    const morceaux = jeton.split('.');
    if (morceaux.length !== 3) return { ok: false, motif: 'forme' };
    const [enteteB64, chargeB64, signatureRecue] = morceaux;

    let entete: unknown;
    let charge: unknown;
    try {
        entete = JSON.parse(Buffer.from(enteteB64, 'base64url').toString('utf8'));
        charge = JSON.parse(Buffer.from(chargeB64, 'base64url').toString('utf8'));
    } catch {
        return { ok: false, motif: 'forme' };
    }
    if (!estObjet(entete) || !estObjet(charge)) return { ok: false, motif: 'forme' };

    // 🔴 L'`alg` est COMPARÉ, jamais employé pour CHOISIR un algorithme.
    // L'en-tête n'est pas signé : dériver un comportement d'une donnée non
    // signée est la confusion d'algorithme, et c'est ainsi qu'un `alg:'none'`
    // sans signature se fait accepter. Le contrôle vient AVANT celui de la
    // signature, pour que le motif rendu nomme la vraie cause.
    if (entete.alg !== ALGORITHME) return { ok: false, motif: 'algorithme' };

    const attendue = Buffer.from(signature(`${enteteB64}.${chargeB64}`, secret), 'utf8');
    const recue = Buffer.from(signatureRecue, 'utf8');
    // Les LONGUEURS d'abord : mesuré, `timingSafeEqual` LÈVE
    // `Input buffers must have the same byte length` quand elles diffèrent.
    if (attendue.length !== recue.length) return { ok: false, motif: 'signature' };
    if (!timingSafeEqual(attendue, recue)) return { ok: false, motif: 'signature' };

    const { sub, exp } = charge;
    if (typeof sub !== 'string' || sub.length === 0) return { ok: false, motif: 'forme' };
    if (typeof exp !== 'number' || !Number.isFinite(exp)) return { ok: false, motif: 'forme' };

    // 🔴 UN TYPE INCONNU EST REFUSE, jamais ramene au defaut. Le ramener a
    // `utilisateur` ferait qu'un jeton d'un TROISIEME type, emis un jour par
    // une version future du service, serait accepte comme humain par une
    // version ancienne -- une degradation silencieuse de privilege verifiee
    // dans le mauvais sens. L'ABSENCE, elle, vaut bien le defaut : c'est le
    // format de P2, et c'est un etat connu, pas un etat inconnu.
    const brut = charge[CLAIM_TYPE];
    let type: TypeSujet;
    if (brut === undefined) {
        type = TYPE_PAR_DEFAUT;
    } else if (typeof brut === 'string' && (TYPES_CONNUS as readonly string[]).includes(brut)) {
        type = brut as TypeSujet;
    } else {
        return { ok: false, motif: 'forme' };
    }

    // Borne FRANCHE, écrite pour que le test puisse l'assiéger des deux côtés.
    if (maintenant >= exp) return { ok: false, motif: 'expire' };

    return { ok: true, sujet: sub, type };
}
