// Le hachage des mots de passe, dans un format qui porte son propre algorithme.
//
// 🔴 POURQUOI LE FORMAT PORTE SES PARAMÈTRES : `scrypt$N$r$p$sel$empreinte`.
// Le jour où ces paramètres seront calibrés — ou remplacés par Argon2id —, un
// re-hachage à la connexion suivante suffira, SANS migration de données
// (spec §3.5). Une empreinte nue ne permettrait pas de savoir avec quoi elle a
// été produite, et toute la base serait à jeter d'un coup.
//
// Mesuré le 19 août 2026 sur ce Node (v24.9.0) :
//     N=16384 r=8 p=1 : OK 29 ms (relevé du plan), 31 ms (relevé de
//         l'implémentation, même machine — l'écart est la variance de mesure)
//     N=32768 r=8 p=1 : REFUSE -> Invalid scrypt params:
//         error:030000AC:digital envelope routines::memory limit exceeded
// La limite est celle de `maxmem` (32 MiB par défaut), franchie dès que
// 128·N·r la dépasse. Le message ne parle PAS de N. Durcir le paramètre exige
// donc de lever `maxmem` explicitement — geste qui n'a PAS été fait ici, faute
// de mesure qui le justifie. Ces paramètres NE SONT PAS CALIBRÉS : les 29 ms
// sont une mesure, pas un objectif atteint. Ils rejoignent la liste déjà
// longue du dépôt (BPP_MIN, FACTEUR_FOCUS, PART_DORMANTE_BPS, HYSTERESIS,
// TAILLE_MAX_SORTIE, DUREE_SECONDES).

import { randomBytes, scrypt, timingSafeEqual } from 'node:crypto';

export interface ParametresScrypt {
    N: number;
    r: number;
    p: number;
}

export const PARAMETRES_COURANTS: ParametresScrypt = { N: 16384, r: 8, p: 1 };

/// 16 octets de sel, 32 octets d'empreinte : les tailles usuelles, et celles
/// que les tests assertent — un sel plus court affaiblirait la protection
/// contre les tables précalculées sans rien économiser d'utile.
const OCTETS_SEL = 16;
const OCTETS_EMPREINTE = 32;

const ALGO = 'scrypt';

function deriver(
    motDePasse: string,
    sel: Buffer,
    params: ParametresScrypt,
): Promise<Buffer> {
    return new Promise((resolve, rejeter) => {
        scrypt(motDePasse, sel, OCTETS_EMPREINTE, params, (erreur, cle) => {
            if (erreur) rejeter(erreur);
            else resolve(cle);
        });
    });
}

/// Rend `scrypt$N$r$p$sel$empreinte`, sel et empreinte en base64url.
export async function hacher(
    motDePasse: string,
    params: ParametresScrypt = PARAMETRES_COURANTS,
): Promise<string> {
    const sel = randomBytes(OCTETS_SEL);
    const empreinte = await deriver(motDePasse, sel, params);
    return [
        ALGO,
        params.N,
        params.r,
        params.p,
        sel.toString('base64url'),
        empreinte.toString('base64url'),
    ].join('$');
}

/// Décompose un encodage. LÈVE sur une forme illisible : un appelant qui
/// recevrait un objet à demi rempli produirait un refus dont la cause serait
/// invisible.
export function analyser(encode: string): {
    algo: string;
    params: ParametresScrypt;
    sel: Buffer;
    empreinte: Buffer;
} {
    const morceaux = encode.split('$');
    if (morceaux.length !== 6) {
        throw new Error(
            `empreinte de mot de passe malformée : ${morceaux.length} segments au lieu de 6`,
        );
    }
    const [algo, n, r, p, sel, empreinte] = morceaux;
    return {
        algo,
        params: { N: Number(n), r: Number(r), p: Number(p) },
        sel: Buffer.from(sel, 'base64url'),
        empreinte: Buffer.from(empreinte, 'base64url'),
    };
}

/// `false` sur mot de passe faux OU empreinte malformée.
///
/// 🔴 LÈVE sur un algorithme inconnu, et c'est délibéré : un `false`
/// silencieux y serait indiscernable d'un mauvais mot de passe, et personne ne
/// saurait diagnostiquer une base écrite par une version future du service.
export async function verifier(motDePasse: string, encode: string): Promise<boolean> {
    let analyse: ReturnType<typeof analyser>;
    try {
        analyse = analyser(encode);
    } catch {
        // Une forme illisible est un refus, pas une exception : l'appelant
        // HTTP doit répondre 401 et non 500.
        return false;
    }

    if (analyse.algo !== ALGO) {
        throw new Error(
            `algorithme de hachage inconnu : ${analyse.algo} — ce service ne sait vérifier que ${ALGO}`,
        );
    }

    const { N, r, p } = analyse.params;
    if (!Number.isInteger(N) || !Number.isInteger(r) || !Number.isInteger(p)) {
        return false;
    }

    let calculee: Buffer;
    try {
        calculee = await deriver(motDePasse, analyse.sel, analyse.params);
    } catch {
        // Des paramètres que la bibliothèque refuse (voir le relevé de tête)
        // rendent un refus, jamais une exception qui remonterait en 500.
        return false;
    }

    // 🔴 LES LONGUEURS D'ABORD. Mesuré : `timingSafeEqual` LÈVE
    // `Input buffers must have the same byte length` sur des longueurs qui
    // diffèrent. Une empreinte tronquée en base ferait donc lever la
    // vérification au lieu de rendre `false`.
    if (calculee.length !== analyse.empreinte.length) return false;
    return timingSafeEqual(calculee, analyse.empreinte);
}

/// Vrai si l'empreinte a été produite avec des paramètres plus faibles que les
/// courants — auquel cas la connexion suivante doit la remplacer.
export function doitEtreRehache(encode: string): boolean {
    let analyse: ReturnType<typeof analyser>;
    try {
        analyse = analyser(encode);
    } catch {
        // Illisible : à réécrire, certainement.
        return true;
    }
    if (analyse.algo !== ALGO) return true;
    return (
        analyse.params.N < PARAMETRES_COURANTS.N ||
        analyse.params.r < PARAMETRES_COURANTS.r ||
        analyse.params.p < PARAMETRES_COURANTS.p
    );
}
