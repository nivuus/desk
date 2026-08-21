// Lire `Authorization: Bearer <jeton>`, et refuser le jeton d'UTILISATEUR.
//
// 🔴 CE MODULE EST LE SYMÉTRIQUE EXACT DE `porteur.ts`, ET LES DEUX SE LISENT
// ENSEMBLE. `identite/jeton.ts` énumère les DEUX confusions et dit qu'elles
// sont graves toutes les deux — les deux jetons sont signés par le MÊME
// secret (`PLATEFORME_SECRET_JETON`) et portent la même charge `{sub, exp}` :
//   - un jeton HUMAIN qui ouvrirait un chemin d'agent ; c'est ce que ce
//     module ferme ;
//   - un jeton d'AGENT qui ouvrirait un chemin humain ; c'est ce que
//     `porteur.ts` ferme.
// Si l'un des deux se relâche, les deux identités redeviennent
// INTERCHANGEABLES : ce n'est pas deux gardes, c'est une seule, écrite en deux
// moitiés. **Ne jamais toucher l'une sans relire l'autre.**
//
// 🔴 CE MODULE EST PUR : ni base, ni socket, ni horloge lue — `maintenant` est
// un PARAMÈTRE, comme partout dans ce dépôt.
//
// 🔴 IL REND LE PRÉFIXE DE SESSION, JAMAIS L'IDENTIFIANT DE VM, et ce n'est
// pas un détail de nommage : `agents/canal.ts` signe `signer(prefixe, …,
// 'agent')`, et son en-tête écrit qu'« un canal qui signerait l'identifiant de
// VM au lieu du préfixe délivrerait des jetons parfaitement valides que RIEN
// n'accepterait ». Le sujet d'un jeton d'agent EST le préfixe. La VM se résout
// ensuite par `depot/agent.ts::lireParPrefixe` — **ce module ne touche pas la
// base**, il n'en a pas le droit et n'en a pas besoin.
//
// ⚠️ POURQUOI LE DÉCOUPAGE DE L'EN-TÊTE EST RECOPIÉ DE `porteur.ts` PLUTÔT QUE
// PARTAGÉ. Les deux modules ne diffèrent que par la ligne du type et par ce
// qu'ils rendent ; tout ce qui précède est identique. Le facteur commun
// vivrait naturellement dans un troisième module, et c'est ce qu'il faudra
// faire le jour où un TROISIÈME lecteur d'en-tête apparaîtra. Il n'est pas
// fait ici parce que le sous-bloc G3 n'ouvre pas `porteur.ts`, et qu'une
// extraction à moitié — le module neuf appelant l'ancien, l'ancien inchangé —
// coûterait une indirection sans rien fermer. **Déclaré plutôt que subi** :
// toute correction du découpage se fait DANS LES DEUX FICHIERS.

import { verifierJeton } from '../identite/jeton';

export type MotifPorteurAgent =
    | 'jeton-absent'
    | 'jeton-invalide'
    | 'jeton-expire'
    | 'jeton-utilisateur';

export type VerdictPorteurAgent =
    | { ok: true; prefixe: string }
    | { ok: false; motif: MotifPorteurAgent; code: 401 | 403 };

/// Le schéma, comparé caractère pour caractère.
const SCHEMA = 'Bearer';

/// Lit l'en-tête et rend le PRÉFIXE DE SESSION, ou un refus qui porte son code
/// HTTP.
///
/// ⚠️ DIVERGENCE DÉLIBÉRÉE AVEC LA RFC 7235 §2.1, reprise de `porteur.ts` mot
/// pour mot et pour la même raison : le standard rend le nom du schéma
/// INSENSIBLE À LA CASSE, et ce module le compare STRICTEMENT — `bearer` et
/// `BEARER` sont refusés. L'unique émetteur est notre propre agent
/// (`agent/src/apps/`, qui écrit `Bearer`), et une comparaison stricte est
/// décidable là où une comparaison souple ouvre une famille de graphies que
/// personne n'a énumérées. **Les deux moitiés de la garde doivent diverger sur
/// le TYPE et sur rien d'autre** : une casse tolérée d'un côté et refusée de
/// l'autre serait une différence que personne n'a décidée.
///
/// ⚠️ TOUS LES MOTIFS DE `verifierJeton` SAUF `expire` SE REPLIENT SUR
/// `jeton-invalide` : `forme`, `algorithme` et `signature` distinguent des
/// façons d'être faux dont le demandeur n'a rien à faire, et dont un
/// attaquant, lui, apprendrait où il en est. `expire` est conservé parce qu'il
/// est ACTIONNABLE — il dit à l'agent de redemander un jeton au canal `/agent`
/// plutôt que de se réenrôler.
export function lirePorteurAgent(
    entetes: Record<string, string | string[] | undefined>,
    secret: string,
    maintenant: number,
): VerdictPorteurAgent {
    const brut = entetes.authorization;

    // 🔴 UN EN-TÊTE RÉPÉTÉ EST REFUSÉ, jamais désambiguïsé — c'est une requête
    // ambiguë, pas une requête à interpréter, et en choisir un serait prendre
    // une décision qu'un attaquant exploite dès que deux couches n'en prennent
    // pas la même.
    //
    // ⚠️ CE CHEMIN EST INATTEIGNABLE DEPUIS UNE VRAIE REQUÊTE HTTP, et c'est
    // MESURÉ, pas supposé : sur ce Node (v24.9.0), deux en-têtes
    // `Authorization` sur la même requête rendent
    // `typeof req.headers.authorization === 'string'` et
    // `Array.isArray(...) === false` — le parseur garde le PREMIER et jette le
    // second, `authorization` figurant à sa liste de rejet. La garde reste
    // parce que ce module est PUR et que rien n'oblige son appelant à être la
    // couche HTTP de Node ; elle n'est simplement pas la ligne de défense
    // qu'on croirait.
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

    // 🔴 403 ET NON 401, exactement comme le `jeton-agent` de `porteur.ts` : le
    // jeton est VALIDE, il n'est simplement pas celui d'un agent. Un 401
    // inviterait à se reconnecter, ce qui ne changerait rien — et masquerait
    // que la requête a été faite avec la mauvaise identité.
    if (verdict.type !== 'agent') {
        return { ok: false, motif: 'jeton-utilisateur', code: 403 };
    }

    // Le sujet d'un jeton d'agent EST le préfixe de session — voir l'en-tête.
    return { ok: true, prefixe: verdict.sujet };
}
