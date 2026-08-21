// Les deux routes d'icône : `PUT /icone/:sha256` (l'AGENT dépose) et
// `GET /application/:id/icone?e=<empreinte>` (l'UTILISATEUR lit).
//
// 🔴 POURQUOI L'EMPREINTE ENTRE DANS L'URL DU `GET`. La spécification écrit
// « `GET /application/:id/icone` avec `Cache-Control` immuable clé sur
// l'empreinte » — **et les deux moitiés se contredisent telles quelles**. Sur
// une URL qui NE PORTE PAS l'empreinte, `immutable` est un MENSONGE : le jour
// où l'icône change, tous les caches servent l'ancienne, POUR UN AN. Le
// paramètre `?e=` rend l'URL véritablement adressée par contenu, et
// `Cache-Control: private, max-age=31536000, immutable` DIT VRAI.
//
// 🔴 ET LE `404` SUR UN `e` PÉRIMÉ N'EST PAS UNE COMMODITÉ : sans lui, une
// vieille URL servirait l'icône COURANTE sous un en-tête immuable, ce qui
// empoisonnerait le cache pour un an avec une image qui n'est pas celle que
// l'URL nomme.
//
// ⚠️ `private`, JAMAIS `public` : la réponse est authentifiée par le porteur,
// et un cache partagé n'a rien à faire d'une icône servie sous un jeton.
//
// 🔴 CONSÉQUENCE NOMMÉE ICI PLUTÔT QUE DÉCOUVERTE PLUS TARD, ET ELLE APPARTIENT
// AU SOUS-BLOC G5 : **un `<img src>` NE PORTE PAS D'EN-TÊTE `Authorization`.**
// Une page qui afficherait ces icônes devra les chercher par `fetch()` puis
// `URL.createObjectURL`, et **un manifeste PWA — dont le navigateur va chercher
// les icônes tout seul, sans en-tête — NE POURRA PAS pointer cette route en
// l'état**. G2 ne le tranche pas : le trancher demanderait de décider si une
// icône peut être servie sans jeton, ce qui est une décision de sécurité.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import type { Magasin } from '../apps/icones';
import { empreinteValide } from '../apps/icones';
import { lireParId } from '../depot/application';
import { lireParId as lireVm } from '../depot/vm';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';
import { lirePorteurAgent } from './porteur-agent';
import { lirePorteur } from './porteur';

export interface DependancesIcone {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    magasin: Magasin;
    maintenant: () => number;
}

/// ⚠️ **MAJORANTE À VUE, ET NON CALIBRÉE.** La plus grosse icône mesurée pèse
/// moins de 30 Kio en moyenne et le corpus entier 4,4 Mo pour 153 — **mais
/// AUCUNE taille individuelle n'a été relevée**, seulement une moyenne. Le
/// dire vaut mieux que de la présenter comme réglée.
///
/// ⚠️ ELLE N'A RIEN À VOIR AVEC LE PLAFOND DE CORPS DE `routes-auth.ts`
/// (4 Kio), QUI NE DOIT PAS ÊTRE RELEVÉ : une route qui accepte des images a
/// son propre plafond, et confondre les deux ouvrirait le corps des routes
/// d'authentification à un mégaoctet.
export const ICONE_MAX_OCTETS = 1_048_576;

const CHEMIN_PUT = '/icone/';

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ INCONDITIONNELS, et posés sur TOUTE réponse — y compris les
        // refus. Étalés AVANT `cors`, dont la politique est facultative et ne
        // doit jamais pouvoir les écraser.
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(corps === undefined ? undefined : JSON.stringify(corps));
}

/// Reconnaît `/icone/:sha256`, et RIEN d'autre.
///
/// 🔴 DÉCOUPÉ PAR SEGMENTS, JAMAIS PAR `startsWith`. G1 a MESURÉ qu'un
/// `startsWith('/application')` laissait ses dix-sept tests VERTS : la route
/// mangeait toute la famille et rendait SON PROPRE 404 typé, indiscernable du
/// 404 générique tant qu'on ne lisait que le statut. Le motif est ancré des
/// DEUX bouts.
function empreinteDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'icone', '<sha256>'] — exactement trois.
    if (segments.length !== 3) return undefined;
    if (segments[1] !== 'icone') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}

/// Reconnaît `/application/:id/icone`, et RIEN d'autre.
function iconeDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'application', '<id>', 'icone'] — exactement quatre.
    if (segments.length !== 4) return undefined;
    if (segments[1] !== 'application' || segments[3] !== 'icone') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}

/// Lit le corps, en s'arrêtant DÈS le dépassement.
///
/// 🔴 LE PLAFOND EST VÉRIFIÉ PENDANT LA LECTURE, PAS APRÈS : accumuler
/// d'abord et mesurer ensuite laisserait un pair remplir la mémoire du service
/// avant que le refus n'arrive.
async function lireCorps(req: IncomingMessage): Promise<Buffer | 'trop-gros'> {
    const morceaux: Buffer[] = [];
    let total = 0;
    for await (const morceau of req) {
        const b = morceau as Buffer;
        total += b.length;
        if (total > ICONE_MAX_OCTETS) return 'trop-gros';
        morceaux.push(b);
    }
    return Buffer.concat(morceaux);
}

export async function servirIcone(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesIcone,
): Promise<boolean> {
    const chemin = (req.url ?? '').split('?')[0];
    const empreintePut = empreinteDe(chemin);
    const idApplication = iconeDe(chemin);
    if (empreintePut === undefined && idApplication === undefined) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    // ⚠️ LES DEUX ROUTES EXIGENT `Authorization`, DONC LA REQUÊTE EST NON
    // SIMPLE : le navigateur envoie d'abord un `OPTIONS`, et **abandonne sans
    // jamais envoyer la vraie requête** si la réponse ne lui convient pas.
    // C'est le défaut exact que la corroboration navigateur du sous-bloc P4 a
    // trouvé, et qu'aucun test de Node ne pouvait voir.
    if (req.method === 'OPTIONS') {
        repondre(rep, 204, undefined, cors);
        return true;
    }

    if (empreintePut !== undefined) {
        if (req.method !== 'PUT') {
            repondre(rep, 405, { refus: 'methode' }, cors);
            return true;
        }
        return depot(req, rep, deps, cors, empreintePut);
    }

    if (req.method !== 'GET') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }
    return service(req, rep, deps, cors, idApplication!);
}

/// `PUT /icone/:sha256` — l'agent dépose.
async function depot(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesIcone,
    cors: Record<string, string> | undefined,
    empreinte: string,
): Promise<boolean> {
    // 🔴 UN JETON D'AGENT, PAS UN JETON D'HUMAIN, et le refus est le
    // SYMÉTRIQUE de celui de `porteur.ts` : `403` et non `401`, parce que le
    // jeton est VALIDE — il n'est simplement pas celui d'un agent. Un `401`
    // inviterait à se reconnecter pour rien.
    //
    // 🔴 CETTE LECTURE VIVAIT ICI EN LIGNE, RECOPIÉE DE `porteur.ts`, ET ELLE
    // EST PASSÉE DANS `porteur-agent.ts` (G3). La raison n'est pas
    // l'esthétique : `GET /televersement/:id/contenu` a besoin de la MÊME
    // lecture, et deux copies d'une garde de sécurité divergent en silence —
    // celle qu'on corrige et celle qu'on oublie. Le module rend en outre le
    // PRÉFIXE DE SESSION, dont cette route-ci n'a pas l'emploi et que l'autre
    // résoudra en VM par `depot/agent.ts::lireParPrefixe`.
    //
    // ⚠️ LES QUATRE MOTIFS ET LEURS CODES SONT INCHANGÉS, à la lettre :
    // `jeton-absent` 401, `jeton-invalide` 401, `jeton-expire` 401,
    // `jeton-utilisateur` 403, et dans cet ordre. La SEULE différence de
    // comportement est un en-tête `Authorization` RÉPÉTÉ, que la copie rangeait
    // avec `jeton-absent` et que le module refuse en `jeton-invalide` — une
    // requête ambiguë n'est pas une requête vide. **Elle est INATTEIGNABLE
    // depuis une vraie requête HTTP, et c'est MESURÉ** : sur Node v24.9.0, deux
    // en-têtes `Authorization` rendent `typeof req.headers.authorization ===
    // 'string'`, le parseur gardant le premier et jetant le second.
    const porteur = lirePorteurAgent(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }

    // 🔴 LA GARDE DE FORME PASSE AVANT TOUTE LECTURE DE CORPS. `:sha256` est
    // un COMPOSANT DE CHEMIN FOURNI PAR LE RÉSEAU, et `..` y est significatif.
    if (!empreinteValide(empreinte)) {
        repondre(rep, 400, { refus: 'empreinte-invalide' }, cors);
        return true;
    }

    const corps = await lireCorps(req);
    if (corps === 'trop-gros') {
        // ⚠️ TYPÉ ET JOURNALISÉ, JAMAIS SILENCIEUX : l'application entrera au
        // catalogue SANS icône, et il faut pouvoir le savoir.
        console.warn(`icone refusee, corps au-dela de ${ICONE_MAX_OCTETS} octets : ${empreinte}`);
        repondre(rep, 413, { refus: 'taille' }, cors);
        return true;
    }

    try {
        // 🔴 LE MAGASIN RECALCULE L'EMPREINTE. C'est la troisième des trois
        // vérifications — « aucun saut ne fait confiance au précédent ». Sans
        // elle, l'adressage par contenu n'en serait pas un.
        deps.magasin.ecrire(empreinte, corps);
    } catch (cause) {
        repondre(rep, 400, { refus: 'empreinte' }, cors);
        return true;
    }
    repondre(rep, 204, undefined, cors);
    return true;
}

/// `GET /application/:id/icone?e=<empreinte>` — l'utilisateur lit.
async function service(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesIcone,
    cors: Record<string, string> | undefined,
    idApplication: string,
): Promise<boolean> {
    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }

    const url = new URL(req.url ?? '', 'http://interne');
    const attendue = url.searchParams.get('e');
    if (attendue === null || attendue === '') {
        repondre(rep, 400, { refus: 'empreinte-absente' }, cors);
        return true;
    }

    const application = await lireParId(deps.base, idApplication);
    if (application === undefined) {
        repondre(rep, 404, { refus: 'application-inconnue' }, cors);
        return true;
    }

    // 🔴 L'AUTORISATION PASSE PAR LA MÊME RÈGLE QUE LES DEUX ROUTES DE G1, et
    // le refus est INDISTINGUABLE — `404 vm-inconnue`, jamais un `403` qui
    // dirait « celle-là existe, mais pas pour vous ». C'est l'oracle
    // d'énumération que le propriétaire du dépôt a retiré, et le
    // réintroduire ici serait le rouvrir par une porte de derrière.
    const vm = await lireVm(deps.base, application.vm_id);
    const autorise =
        vm !== undefined
        && (vm.utilisateur_id === null || vm.utilisateur_id === porteur.utilisateurId);
    if (!autorise) {
        repondre(rep, 404, { refus: 'vm-inconnue' }, cors);
        return true;
    }

    // 🔴 UN `e` QUI NE CORRESPOND PAS À L'EMPREINTE COURANTE REND `404`, et
    // c'est ce qui rend `immutable` honnête : sans ce refus, une vieille URL
    // servirait l'icône COURANTE sous un en-tête immuable, empoisonnant le
    // cache pour un an avec une image qui n'est pas celle que l'URL nomme.
    if (application.icone === null || application.icone !== attendue) {
        repondre(rep, 404, { refus: 'icone-inconnue' }, cors);
        return true;
    }

    const octets = deps.magasin.lire(attendue);
    if (octets === undefined) {
        // La base connaît l'empreinte, le disque ne l'a pas encore : c'est
        // l'état NORMAL entre l'annonce et le téléversement, et c'est aussi
        // celui d'un magasin perdu. Les deux se réparent seuls.
        repondre(rep, 404, { refus: 'icone-inconnue' }, cors);
        return true;
    }

    // 🔴 L'ORDRE EST INVERSE DE CELUI DE `repondre`, ET C'EST UNE EXCEPTION
    // DÉCLARÉE, LA SEULE DU SERVICE.
    //
    // `ENTETES_SECURITE` porte `Cache-Control: no-store`, posé pour les
    // réponses JSON — dont `/auth/*`, qui rend des jetons. Cette réponse-ci
    // n'est pas du JSON : c'est une image ADRESSÉE PAR CONTENU, dont l'URL
    // porte l'empreinte, et la mettre en cache est tout l'objet de la route.
    // Les deux en-têtes sont donc étalés D'ABORD et `cache-control` est
    // écrasé ENSUITE, délibérément.
    //
    // ⚠️ CE QUI N'EST PAS ÉCRASÉ EST `X-Content-Type-Options: nosniff`, et
    // c'est le seul des deux qui soit une garde de sécurité : sans lui, un
    // navigateur pourrait deviner un type autre que `image/png` sur des
    // octets qu'un pair a déposés. Un test l'assère nommément sur la réponse
    // 200, pour que cette exception ne puisse pas s'élargir en silence.
    rep.writeHead(200, {
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
        'content-type': 'image/png',
        'content-length': String(octets.length),
        // ⚠️ `private` ET NON `public` : la réponse est authentifiée par le
        // porteur. `immutable` DIT VRAI parce que l'URL porte l'empreinte.
        //
        // 🔴 LA CASSE DE CETTE CLÉ N'EST PAS LIBRE : elle doit être CELLE
        // D'`ENTETES_SECURITE`, à la lettre. Un objet JavaScript distingue
        // `Cache-Control` de `cache-control`, et `writeHead` émet ALORS LES
        // DEUX — le client lit `no-store, private, max-age=…`, c'est-à-dire
        // une réponse qui se dit à la fois non stockable et immuable. Trouvé
        // par l'exécution, pas par la relecture.
        'Cache-Control': 'private, max-age=31536000, immutable',
    });
    rep.end(octets);
    return true;
}
