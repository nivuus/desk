// L'extraction du jeton porteur d'AGENT, PURE.
//
// 🔴 CE FICHIER EST LE JUMEAU DE `porteur.test.ts`, ET LES DEUX SE LISENT
// ENSEMBLE. `identite/jeton.ts` énumère les DEUX confusions et dit qu'elles
// sont graves toutes les deux : un jeton HUMAIN ouvrant un chemin d'agent, et
// un jeton d'AGENT ouvrant un chemin humain. Les deux jetons sont signés par
// le MÊME secret et portent la même charge `{sub, exp}` — sans le claim `sty`
// ils sont littéralement interchangeables.
//
// **Si l'un des deux sens se relâche, les deux identités redeviennent
// interchangeables.** `porteur.test.ts` tient « un jeton d'AGENT est refusé
// par le lecteur humain » ; ce fichier tient le sens INVERSE, « un jeton
// d'UTILISATEUR est refusé par le lecteur d'agent ». Ni l'un ni l'autre n'a de
// valeur seul : ce sont les deux moitiés d'une seule garde.

import { describe, expect, it } from 'vitest';
import { DUREE_JETON_ACCES_MS, signer } from '../identite/jeton';
import { lirePorteurAgent } from './porteur-agent';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const MS = 1_787_136_773_742;

/// Le sujet d'un jeton d'agent est le PRÉFIXE DE SESSION — c'est ce que
/// `agents/canal.ts` signe (`jetonNeuf(prefixe)`), et jamais l'identifiant de
/// VM. Ce nom-là est donc la moitié de l'assertion.
const PREFIXE = 'AAAAAAAAAAAAAAAAAAAAAA';

function entetes(
    valeur: string | string[] | undefined,
): Record<string, string | string[] | undefined> {
    // Node met les noms d'en-tête en MINUSCULES : `req.headers.authorization`
    // est la seule graphie qui existe côté serveur.
    return valeur === undefined ? {} : { authorization: valeur };
}

function jetonAgent(sujet: string = PREFIXE): string {
    return signer(sujet, SECRET, MS, undefined, 'agent');
}

describe('lirePorteurAgent', () => {
    it('en-tête ABSENT → jeton-absent, 401', () => {
        // 🔴 La rouge : rendre `ok:true` avec un préfixe vide. La route de
        // téléversement deviendrait publique, au nom d'une VM qui n'existe
        // pas — et `depot/agent.ts::lireParPrefixe('')` ne rendrait rien, donc
        // le refus tomberait bien plus loin, sous un motif qui ne dit pas la
        // cause.
        expect(lirePorteurAgent(entetes(undefined), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-absent',
            code: 401,
        });
        // Une valeur vide n'est pas davantage un jeton.
        expect(lirePorteurAgent(entetes(''), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-absent',
            code: 401,
        });
    });

    it('`Bearer <jeton d’agent valide>` → le PRÉFIXE, jamais un id de VM', () => {
        // 🔴 CE QUI EST RENDU EST LE SUJET DU JETON, ET LE SUJET EST LE
        // PRÉFIXE. `agents/canal.ts` écrit `signer(prefixe, …, 'agent')` et
        // son en-tête dit qu'« un canal qui signerait l'identifiant de VM au
        // lieu du préfixe délivrerait des jetons parfaitement valides que RIEN
        // n'accepterait ». Un module qui rendrait ici un `vmId` serait une
        // panne muette de bout en bout : la VM se résout PLUS TARD, par
        // `depot/agent.ts::lireParPrefixe`.
        expect(lirePorteurAgent(entetes(`Bearer ${jetonAgent()}`), SECRET, MS)).toEqual({
            ok: true,
            prefixe: PREFIXE,
        });
    });

    it('🔴 `Bearer <jeton d’UTILISATEUR>` → jeton-utilisateur, 403', () => {
        // 🔴 LA ROUGE, ET ELLE VA PAR PAIRE AVEC CELLE DE `porteur.test.ts` :
        // accepter le type `utilisateur` ici. Un humain déposerait alors des
        // installeurs et téléchargerait ceux d'une VM dont il n'est pas
        // l'agent, avec un jeton que le service lui a lui-même délivré. La
        // symétrie est le point : `porteur.ts` refuse `agent`, ce module
        // refuse `utilisateur`, et relâcher L'UN DES DEUX suffit à rendre les
        // deux identités interchangeables (`identite/jeton.ts`).
        //
        // ⚠️ 403 ET NON 401 : le jeton est VALIDE, il n'est simplement pas
        // celui d'un agent. Un 401 inviterait à se reconnecter pour rien.
        const jetonHumain = signer('u-ada', SECRET, MS);
        expect(lirePorteurAgent(entetes(`Bearer ${jetonHumain}`), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-utilisateur',
            code: 403,
        });
    });

    it('🔴 un jeton SANS claim de type est un jeton d’utilisateur, donc refusé', () => {
        // 🔴 L'ABSENCE DU CLAIM VAUT `utilisateur` (`identite/jeton.ts`,
        // `TYPE_PAR_DEFAUT`) — c'est le format des jetons de P2, encore en
        // vol. Ce test tient que le lecteur d'agent lit bien le DÉFAUT et ne
        // se contente pas de `verdict.type !== 'utilisateur'` : un module qui
        // testerait l'absence du claim comme « pas humain » accepterait tout
        // jeton de P2.
        //
        // ⚠️ Il n'est pas redondant avec le précédent : `signer` N'ÉCRIT PAS
        // le claim pour un `utilisateur`, si bien que les deux jetons sont le
        // même octet pour octet — c'est justement ce qui rend l'assertion
        // solide et le commentaire nécessaire, sans quoi un lecteur croira à
        // une copie.
        const sansClaim = signer('u-ada', SECRET, MS, undefined, 'utilisateur');
        expect(sansClaim.split('.').length).toBe(3);
        expect(lirePorteurAgent(entetes(`Bearer ${sansClaim}`), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-utilisateur',
            code: 403,
        });
    });

    it('🔴 jeton EXPIRÉ → jeton-expire, et l’horloge VARIE', () => {
        // 🔴 La rouge : figer l'horloge. Le test deviendrait inerte — il n'y
        // aurait qu'un instant observable et le seuil ne serait jamais
        // franchi. La borne d'`identite/jeton.ts` est FRANCHE (`maintenant >=
        // exp`) précisément pour qu'on puisse l'assiéger des deux côtés.
        const jeton = jetonAgent();
        const exp = MS + DUREE_JETON_ACCES_MS;
        // Une milliseconde AVANT : encore valide.
        expect(lirePorteurAgent(entetes(`Bearer ${jeton}`), SECRET, exp - 1)).toEqual({
            ok: true,
            prefixe: PREFIXE,
        });
        // À la borne EXACTE : expiré.
        expect(lirePorteurAgent(entetes(`Bearer ${jeton}`), SECRET, exp)).toEqual({
            ok: false,
            motif: 'jeton-expire',
            code: 401,
        });
    });

    it('🔴 un schéma autre que `Bearer` → jeton-invalide', () => {
        // 🔴 La rouge : accepter n'importe quel schéma. La casse est comparée
        // STRICTEMENT, divergence avec la RFC 7235 déclarée dans
        // `porteur-agent.ts` — et elle DOIT être la même que celle de
        // `porteur.ts` : les deux moitiés de la garde divergent sur le TYPE et
        // sur rien d'autre.
        const jeton = jetonAgent();
        for (const brut of [
            `Basic ${jeton}`,
            `bearer ${jeton}`,
            `BEARER ${jeton}`,
            jeton,
            `Bearer`,
            `Bearer ${jeton} de-trop`,
        ]) {
            const v = lirePorteurAgent(entetes(brut), SECRET, MS);
            expect(v.ok).toBe(false);
            if (v.ok) return;
            expect(v.motif).toBe('jeton-invalide');
            expect(v.code).toBe(401);
        }
        // Une signature fausse est invalide de la même façon — jamais 500.
        expect(lirePorteurAgent(entetes(`Bearer ${jeton}x`), SECRET, MS)).toEqual({
            ok: false,
            motif: 'jeton-invalide',
            code: 401,
        });
    });

    it('🔴 un en-tête RÉPÉTÉ (string[]) → jeton-invalide', () => {
        // 🔴 La rouge : prendre `entetes.authorization[0]` en silence. Deux
        // en-têtes d'autorisation est une requête AMBIGUË, pas une requête à
        // interpréter.
        //
        // ⚠️ CE CHEMIN EST INATTEIGNABLE DEPUIS UNE VRAIE REQUÊTE HTTP, et
        // c'est MESURÉ : sur Node v24.9.0, deux en-têtes `Authorization`
        // rendent une CHAÎNE — le parseur garde le premier et jette le second.
        // Le test tient donc une propriété du module PUR, pas une défense de
        // la route ; le dire évite qu'un successeur le lise comme la preuve
        // que `PUT /icone/:sha256` est protégé de ce cas.
        expect(
            lirePorteurAgent(
                entetes([`Bearer ${jetonAgent()}`, `Bearer ${signer('u-ada', SECRET, MS)}`]),
                SECRET,
                MS,
            ),
        ).toEqual({ ok: false, motif: 'jeton-invalide', code: 401 });
        // Même un tableau d'UN SEUL élément : la forme est ambiguë, pas la
        // valeur. Node ne produit un tableau que s'il a vu plusieurs en-têtes.
        expect(lirePorteurAgent(entetes([`Bearer ${jetonAgent()}`]), SECRET, MS).ok).toBe(false);
    });
});
