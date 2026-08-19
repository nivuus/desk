// Lire `Authorization: Bearer <jeton>`, et refuser le jeton d'agent.
//
// 🔴 CE MODULE EST PUR : ni base, ni socket, ni horloge lue — `maintenant` est
// un PARAMÈTRE, comme partout dans ce dépôt.
//
// 🔴 POURQUOI IL EXISTE, ET POURQUOI `identite/garde.ts` NE POUVAIT PAS
// SERVIR. La garde a pour signature `verifier(poignee: { role; session;
// jeton? })` : elle est taillée pour une POIGNÉE DE MAIN WebSocket, elle porte
// le registre d'appartenance de session, et elle est branchée exclusivement sur
// le relais. Rien, nulle part dans ce service, ne lisait l'en-tête
// `Authorization` avant P4. Le précédent d'un consommateur direct de
// `verifierJeton` hors de la garde est `agents/canal.ts`.
//
// 🔴 LE REFUS DU TYPE `agent` N'EST PAS DÉCORATIF. Les deux jetons sont signés
// par le MÊME secret et portent la même charge : `identite/jeton.ts` énumère
// les deux confusions et dit qu'elles sont graves toutes les deux. Le sens
// « un jeton humain ouvre un rôle agent » est fermé par la garde et éprouvé
// par P3 ; c'est l'autre sens que ce module ferme, et c'est le seul que P4
// ouvre.

import { verifierJeton } from '../identite/jeton';

export type MotifPorteur = 'jeton-absent' | 'jeton-invalide' | 'jeton-expire' | 'jeton-agent';

export type VerdictPorteur =
    | { ok: true; utilisateurId: string }
    | { ok: false; motif: MotifPorteur; code: 401 | 403 };

/// Le schéma, comparé caractère pour caractère.
const SCHEMA = 'Bearer';

/// Lit l'en-tête et rend le sujet, ou un refus qui porte son code HTTP.
///
/// ⚠️ DIVERGENCE DÉLIBÉRÉE AVEC LA RFC 7235 §2.1, déclarée plutôt que subie :
/// le standard rend le nom du schéma INSENSIBLE À LA CASSE, et ce module le
/// compare STRICTEMENT — `bearer` et `BEARER` sont refusés. La raison est que
/// l'unique émetteur est notre propre client (`client/src/connexion.ts`), qui
/// écrit `Bearer`, et qu'une comparaison stricte est décidable là où une
/// comparaison souple ouvre une famille de graphies que personne n'a
/// énumérées. Le jour où un client tiers parlerait à cette API, c'est cette
/// décision qu'il faudrait rouvrir, et non la contourner. Le contrôle est
/// EXPLICITE dans un sens ; ne pas le rendre implicite dans l'autre.
///
/// ⚠️ TOUS LES MOTIFS DE `verifierJeton` SAUF `expire` SE REPLIENT SUR
/// `jeton-invalide`, et c'est délibéré : `forme`, `algorithme` et `signature`
/// distinguent des façons d'être faux dont le demandeur n'a rien à faire, et
/// dont un attaquant, lui, apprendrait où il en est. `expire` est conservé
/// parce qu'il est ACTIONNABLE — il dit de rafraîchir plutôt que de se
/// reconnecter.
export function lirePorteur(
    entetes: Record<string, string | string[] | undefined>,
    secret: string,
    maintenant: number,
): VerdictPorteur {
    const brut = entetes.authorization;

    // 🔴 UN EN-TÊTE RÉPÉTÉ EST REFUSÉ, jamais désambiguïsé. Node rend un
    // tableau quand il a vu plusieurs en-têtes du même nom ; en choisir un
    // serait prendre une décision qu'un attaquant exploite dès que deux
    // couches n'en prennent pas la même. C'est une requête ambiguë, pas une
    // requête à interpréter.
    if (Array.isArray(brut)) return { ok: false, motif: 'jeton-invalide', code: 401 };
    if (brut === undefined || brut === '') {
        return { ok: false, motif: 'jeton-absent', code: 401 };
    }

    // Découpage sur les espaces, et EXACTEMENT deux morceaux : un troisième
    // morceau est une valeur qu'on ne sait pas lire, pas un jeton à tronquer.
    const morceaux = brut.split(' ');
    if (morceaux.length !== 2 || morceaux[0] !== SCHEMA || morceaux[1] === '') {
        return { ok: false, motif: 'jeton-invalide', code: 401 };
    }

    const verdict = verifierJeton(morceaux[1], secret, maintenant);
    if (!verdict.ok) {
        return verdict.motif === 'expire'
            ? { ok: false, motif: 'jeton-expire', code: 401 }
            : { ok: false, motif: 'jeton-invalide', code: 401 };
    }

    // 🔴 403 ET NON 401 : le jeton est VALIDE, il n'est simplement pas celui
    // d'un humain. Un 401 inviterait à se reconnecter, ce qui ne changerait
    // rien — et masquerait que la requête a été faite avec la mauvaise
    // identité.
    if (verdict.type !== 'utilisateur') {
        return { ok: false, motif: 'jeton-agent', code: 403 };
    }

    return { ok: true, utilisateurId: verdict.sujet };
}
