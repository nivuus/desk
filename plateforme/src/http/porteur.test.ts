// L'extraction du jeton porteur, PURE.
//
// 🔴 LES DEUX SENS DE LA CONFUSION SONT NOMMÉS, UN SEUL EST ÉPROUVÉ ICI.
// `identite/jeton.ts` énumère les deux et dit qu'elles sont graves toutes les
// deux : un jeton HUMAIN ouvrant un rôle `agent`, et un jeton d'AGENT ouvrant
// un rôle humain. Le premier sens est fermé par `identite/garde.ts` et éprouvé
// par P3 ; c'est le SECOND que ce fichier tient, parce que c'est le seul que
// P4 ouvre — une route HTTP qui accepterait un jeton d'agent lui montrerait
// l'inventaire d'un humain.

import { describe, expect, it } from 'vitest';
import { DUREE_JETON_ACCES_MS, signer } from '../identite/jeton';
import { lirePorteur } from './porteur';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const MS = 1_787_136_773_742;

function entetes(valeur: string | string[] | undefined): Record<string, string | string[] | undefined> {
    // Node met les noms d'en-tête en MINUSCULES : `req.headers.authorization`
    // est la seule graphie qui existe côté serveur.
    return valeur === undefined ? {} : { authorization: valeur };
}

describe('lirePorteur', () => {
    it('en-tête ABSENT → jeton-absent, 401', () => {
        // 🔴 La rouge : rendre `ok:true` avec un sujet vide. Toute route
        // deviendrait publique, au nom d'un utilisateur qui n'existe pas.
        expect(lirePorteur(entetes(undefined), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-absent',
            code: 401,
        });
        // Une valeur vide n'est pas davantage un jeton.
        expect(lirePorteur(entetes(''), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-absent',
            code: 401,
        });
    });

    it('`Bearer <jeton d’utilisateur valide>` → le sujet', () => {
        const jeton = signer('u-ada', SECRET, MS);
        expect(lirePorteur(entetes(`Bearer ${jeton}`), SECRET, MS)).toEqual({
            ok: true,
            utilisateurId: 'u-ada',
        });
    });

    it('🔴 `Bearer <jeton d’AGENT>` → jeton-agent, 403', () => {
        // 🔴 La rouge : accepter le type `agent`. Les deux jetons sont signés
        // par le MÊME secret et portent la même charge `{sub, exp}` : sans le
        // claim `sty`, ils sont littéralement interchangeables
        // (`identite/jeton.ts`). Un agent verrait l'inventaire d'un humain.
        //
        // ⚠️ 403 ET NON 401 : le jeton est VALIDE, il n'est simplement pas
        // celui d'un humain. Un 401 inviterait à se reconnecter, ce qui ne
        // changerait rien.
        const jetonAgent = signer('PREFIXEdelaVM', SECRET, MS, undefined, 'agent');
        expect(lirePorteur(entetes(`Bearer ${jetonAgent}`), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-agent',
            code: 403,
        });
    });

    it('🔴 jeton EXPIRÉ → jeton-expire, et l’horloge VARIE', () => {
        // 🔴 La rouge : figer l'horloge. Le test deviendrait inerte — il n'y
        // aurait qu'un instant observable et le seuil ne serait jamais
        // franchi. La borne d'`identite/jeton.ts` est FRANCHE (`maintenant >=
        // exp`) précisément pour qu'on puisse l'assiéger des deux côtés.
        const jeton = signer('u-ada', SECRET, MS);
        const exp = MS + DUREE_JETON_ACCES_MS;
        // Une milliseconde AVANT : encore valide.
        expect(lirePorteur(entetes(`Bearer ${jeton}`), SECRET, exp - 1)).toEqual({
            ok: true,
            utilisateurId: 'u-ada',
        });
        // À la borne EXACTE : expiré.
        expect(lirePorteur(entetes(`Bearer ${jeton}`), SECRET, exp)).toEqual({
            ok: false,
            motif: 'jeton-expire',
            code: 401,
        });
    });

    it('🔴 un schéma autre que `Bearer` → jeton-invalide', () => {
        // 🔴 La rouge : accepter n'importe quel schéma. Et la casse est
        // comparée STRICTEMENT — voir le commentaire de `porteur.ts`, qui
        // déclare la divergence avec la RFC 7235 plutôt que de la subir. Le
        // contrôle doit être explicite dans un sens ou dans l'autre ; ici il
        // l'est dans le sens strict.
        const jeton = signer('u-ada', SECRET, MS);
        for (const brut of [
            `Basic ${jeton}`,
            `bearer ${jeton}`,
            `BEARER ${jeton}`,
            jeton,
            `Bearer`,
            `Bearer ${jeton} de-trop`,
        ]) {
            const v = lirePorteur(entetes(brut), SECRET, MS);
            expect(v.ok).toBe(false);
            if (v.ok) return;
            expect(v.motif).toBe('jeton-invalide');
            expect(v.code).toBe(401);
        }
        // Une signature fausse est invalide de la même façon — jamais 500.
        expect(lirePorteur(entetes(`Bearer ${jeton}x`), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-invalide',
            code: 401,
        });
    });

    it('🔴 un en-tête RÉPÉTÉ (string[]) → jeton-invalide', () => {
        // 🔴 La rouge : prendre `entetes.authorization[0]` en silence. Deux
        // en-têtes d'autorisation est une requête AMBIGUË, pas une requête à
        // interpréter — et choisir l'un des deux est exactement le genre de
        // décision qu'un attaquant exploite quand deux couches n'en choisissent
        // pas le même.
        const bon = signer('u-ada', SECRET, MS);
        const agent = signer('PREFIXEdelaVM', SECRET, MS, undefined, 'agent');
        expect(lirePorteur(entetes([`Bearer ${bon}`, `Bearer ${agent}`]), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-invalide',
            code: 401,
        });
        // Même un tableau d'UN SEUL élément : la forme est ambiguë, pas la
        // valeur. Node ne produit un tableau que s'il a vu plusieurs en-têtes.
        expect(lirePorteur(entetes([`Bearer ${bon}`]), SECRET, MS).ok).toBe(false);
    });
});
