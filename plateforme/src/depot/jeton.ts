// La chaîne de rafraîchissement : rotative à chaque emploi, hachée en base, et
// détectrice de rejeu.
//
// 🔴 POURQUOI SHA-256 ICI ALORS QUE LE MOT DE PASSE EMPLOIE `scrypt`.
// `scrypt` est lent PAR CONCEPTION, pour rendre coûteuse l'attaque par
// dictionnaire d'un secret à faible entropie. Un jeton de 256 bits tiré au
// hasard n'a pas de dictionnaire : le hachage ne sert ici qu'à ce qu'une fuite
// de la base ne rende pas les jetons utilisables, et SHA-256 y suffit.
// **Le coût est nommé** : si un jour un jeton de rafraîchissement devenait
// dérivé d'un secret humain, cette décision serait à rouvrir.
//
// 🔴 LA RÈGLE DE REJEU, et pourquoi elle exige DEUX colonnes que la spec §5
// n'avait pas. Sans `famille` ni `remplace_par`, rien ne relie un jeton tourné
// à son successeur : présenter un jeton déjà tourné ne pourrait révoquer que
// la ligne DÉJÀ révoquée, et le voleur qui a tourné le premier garderait son
// jeton neuf. La détection ne protégerait rien.
//
// ⚠️ `rejeu` et `revoque` se distinguent par `remplace_par`, et cette
// distinction n'est pas cosmétique : une ligne révoquée AVEC successeur a été
// tournée légitimement — la présenter de nouveau est un REJEU, donc une
// compromission, donc la famille tombe. Une ligne révoquée SANS successeur n'a
// jamais été tournée : c'est un jeton mort (déconnexion, révocation
// administrative), et il n'y a rien à révoquer de plus. Sans elle, le motif
// `revoque` serait une variante inatteignable — un contrôle qui ne peut pas
// échouer, sous une autre forme.

import { createHash, randomBytes, randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';

export type MotifRafraichissement = 'inconnu' | 'expire' | 'rejeu' | 'revoque';
export type IssueRotation =
    | { ok: true; clair: string; utilisateurId: string }
    | { ok: false; motif: MotifRafraichissement };

/// 30 jours. ⚠️ NON CALIBRÉE : aucune mesure ne l'a jugée. Elle rejoint la
/// liste des constantes non calibrées du dépôt.
export const DUREE_RAFRAICHISSEMENT_MS = 30 * 24 * 60 * 60 * 1000;

/// 32 octets, soit 256 bits d'entropie. C'est ce qui autorise SHA-256 plutôt
/// que `scrypt` — voir l'en-tête.
const OCTETS_CLAIR = 32;

interface LigneJeton {
    id: string;
    utilisateur_id: string;
    famille: string;
    remplace_par: string | null;
    expire_a: number | string;
    revoque_a: number | string | null;
}

function nouveauClair(): string {
    return randomBytes(OCTETS_CLAIR).toString('base64url');
}

/// L'empreinte stockée. Le clair n'entre JAMAIS en base.
function empreinteDe(clair: string): string {
    return createHash('sha256').update(clair).digest('base64url');
}

async function lireParEmpreinte(p: Pilote, empreinte: string): Promise<LigneJeton | undefined> {
    const lignes = await p.interroger<LigneJeton>(
        'SELECT id, utilisateur_id, famille, remplace_par, expire_a, revoque_a FROM jeton_rafraichissement WHERE empreinte = ?',
        [empreinte],
    );
    return lignes[0];
}

async function inserer(
    p: Pilote,
    id: string,
    utilisateurId: string,
    famille: string,
    clair: string,
    maintenant: number,
): Promise<void> {
    await p.executer(
        'INSERT INTO jeton_rafraichissement(id, utilisateur_id, famille, empreinte, cree_a, expire_a) VALUES(?, ?, ?, ?, ?, ?)',
        [id, utilisateurId, famille, empreinteDe(clair), maintenant, maintenant + DUREE_RAFRAICHISSEMENT_MS],
    );
}

/// Ouvre une famille NEUVE — c'est le geste de la connexion. Rend le jeton EN
/// CLAIR, la seule et unique fois où il existe hors du navigateur.
export async function emettre(
    p: Pilote,
    utilisateurId: string,
    maintenant: number,
): Promise<string> {
    const clair = nouveauClair();
    await inserer(p, randomUUID(), utilisateurId, randomUUID(), clair, maintenant);
    return clair;
}

/// Révoque toute une famille et rend le NOMBRE de lignes encore vivantes
/// qu'elle a fauchées. La clause `revoque_a IS NULL` rend l'appel idempotent :
/// une seconde révocation ne déplace pas l'instant de la première.
export async function revoquerFamille(
    p: Pilote,
    famille: string,
    maintenant: number,
): Promise<number> {
    const r = await p.executer(
        'UPDATE jeton_rafraichissement SET revoque_a = ? WHERE famille = ? AND revoque_a IS NULL',
        [maintenant, famille],
    );
    return r.lignes;
}

/// Tourne un jeton : révoque le présenté, en émet un neuf dans la MÊME
/// famille, le tout dans UNE SEULE transaction.
///
/// ⚠️ `genererClair` est une COUTURE DE TEST, et rien d'autre. Elle existe
/// pour qu'un test puisse faire échouer l'insertion neuve — sur l'index UNIQUE
/// de `empreinte` — et vérifier que la transaction annule bien la révocation
/// de l'ancien. Sans elle, le chemin de l'échec partiel serait du code jamais
/// couru, et ce dépôt a payé plusieurs fois pour des chemins de repli qui
/// n'avaient jamais tourné. La production ne la passe jamais.
export async function tourner(
    p: Pilote,
    clair: string,
    maintenant: number,
    genererClair: () => string = nouveauClair,
): Promise<IssueRotation> {
    return p.transaction(async (tx) => {
        const ligne = await lireParEmpreinte(tx, empreinteDe(clair));
        if (!ligne) return { ok: false, motif: 'inconnu' } as const;

        if (ligne.revoque_a !== null && ligne.revoque_a !== undefined) {
            if (ligne.remplace_par === null || ligne.remplace_par === undefined) {
                // Révoqué sans jamais avoir été tourné : jeton mort, pas rejeu.
                return { ok: false, motif: 'revoque' } as const;
            }
            // 🔴 REJEU : ce jeton a DÉJÀ servi à en obtenir un neuf. Ou bien
            // le porteur légitime rejoue, ou bien un voleur — indiscernable,
            // et le doute se tranche du côté sûr : toute la famille tombe, y
            // compris le jeton neuf que le voleur détient.
            await revoquerFamille(tx, ligne.famille, maintenant);
            return { ok: false, motif: 'rejeu' } as const;
        }

        if (Number(ligne.expire_a) <= maintenant) {
            return { ok: false, motif: 'expire' } as const;
        }

        const idNeuf = randomUUID();
        const clairNeuf = genererClair();
        await tx.executer(
            'UPDATE jeton_rafraichissement SET revoque_a = ?, remplace_par = ? WHERE id = ?',
            [maintenant, idNeuf, ligne.id],
        );
        await inserer(tx, idNeuf, ligne.utilisateur_id, ligne.famille, clairNeuf, maintenant);
        return { ok: true, clair: clairNeuf, utilisateurId: ligne.utilisateur_id } as const;
    });
}
