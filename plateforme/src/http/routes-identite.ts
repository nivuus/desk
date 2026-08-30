// `GET /auth/moi` : l'identité posée par Pomerium, échangée contre le jeton
// interne — le MÊME jeton que celui du mot de passe, à l'octet près.
//
// 🔴 POURQUOI CE FICHIER EXISTE PLUTÔT QU'UNE ROUTE DE PLUS DANS
// `routes-auth.ts` : ce dernier pesait 397 lignes au 21 août 2026, et la règle
// des 500 lignes veut qu'une addition substantielle s'accompagne d'une
// extraction. Ce sont par ailleurs deux DÉLIVRANCES différentes du même jeton,
// et elles ne partagent aucune règle : l'une hache un mot de passe, l'autre
// lit un en-tête.
//
// 🔴 LE JETON INTERNE N'EST PAS REMPLACÉ, ET C'EST LE CŒUR DE TOUT LE
// CHANTIER. Il authentifie la poignée de main du relais, que Pomerium ne peut
// pas garder — l'agent Windows n'a ni navigateur, ni cookie, ni session
// Google. Le retirer couperait l'agent.
//
// ⚠️ AUCUN JETON DE RAFRAÎCHISSEMENT N'EST DÉLIVRÉ, et ce n'est pas un oubli :
// le cookie Pomerium vit 8640 h, et tenir une chaîne rotative anti-rejeu dont
// plus personne n'a besoin serait du code vivant que rien n'exerce.
//
// 🔴 MAIS « À L'EXPIRATION, LE CLIENT RAPPELLE CETTE ROUTE » ÉTAIT FAUX, ET LA
// PHRASE EST CORRIGÉE PLUTÔT QUE SUPPRIMÉE (revue transverse du chantier,
// 21 août 2026). **Aucun code du client ne rappelle cette route à
// l'expiration** : `client/src/jeton.ts::rafraichirSiNecessaire` n'a aucun
// appelant de production, et `connexion.ts::tenterPomerium` ne court qu'au
// CHARGEMENT de la page de connexion. Ce qui rappelle réellement `/auth/moi`
// est donc un rechargement de page — un geste de l'utilisateur, que le cookie
// de 8640 h rend silencieux pour lui, mais qui reste un geste. Le raisonnement
// sur le rafraîchissement ne change pas ; la description du produit, si.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { creerUtilisateur, lireParEmail } from '../depot/utilisateur';
import { signer } from '../identite/jeton';
import { pairDeConfiance } from './adresse-source';
import { entetesCors } from './cors';
import { repondreIntrouvable } from './introuvable';
import { ENTETES_SECURITE } from './entetes';

export const CHEMIN_MOI = '/auth/moi';

/// L'en-tête que Pomerium pose quand la route déclare
/// `pass_identity_headers: true`. **En minuscules** : Node normalise les noms
/// d'en-tête entrants, et une comparaison sur la casse d'origine ne
/// correspondrait jamais.
export const ENTETE_IDENTITE = 'x-pomerium-claim-email';

/// Ce qu'on écrit dans `empreinte_mdp`, qui est `NOT NULL` (`0001-socle.sql`).
///
/// ⚠️ JAMAIS UNE CHAÎNE VIDE : elle pourrait un jour croiser un vérificateur
/// permissif. Ce marqueur ne peut correspondre à aucun format que
/// `identite/mot-de-passe.ts` sait lire (`scrypt$N$r$p$sel$empreinte`).
export const MARQUEUR_SANS_MOT_DE_PASSE = 'pomerium$aucun-mot-de-passe';

export type VerdictIdentite =
    | { ok: true; email: string }
    | { ok: false; motif: 'identite-absente' };

export interface DependancesIdentite {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    maintenant: () => number;
    auth: 'pomerium' | 'motdepasse';
    /// L'ensemble des adresses dont on croit l'en-tête `X-Pomerium-Claim-Email`.
    /// Voir la garde ci-dessous, et `http/adresse-source.ts::pairDeConfiance`.
    proxyDeConfiance: ReadonlySet<string>;
}

/// 🔴 PURE : ni base, ni socket, ni horloge. C'est ce qui la rend éprouvable
/// sans monter de serveur, et c'est la convention de tout ce répertoire.
export function lireIdentitePomerium(
    entetes: Record<string, string | string[] | undefined>,
): VerdictIdentite {
    const brut = entetes[ENTETE_IDENTITE];
    // ⚠️ CE COMMENTAIRE DISAIT « un en-tête RÉPÉTÉ est refusé », ET C'EST FAUX
    // POUR CE CHEMIN PRÉCIS (mesuré, tâche 6, revue « round de correction 1 »,
    // 22 août 2026) : Node ne rend PAS un tableau pour deux occurrences de
    // `x-pomerium-claim-email` — ce nom n'est pas dans la petite liste
    // d'en-têtes que Node expose en tableau (`set-cookie` en est ; celui-ci
    // n'en est pas). Node les JOINT en UNE SEULE chaîne séparée par `, ` avant
    // même que ce code ne s'exécute. Le garde `Array.isArray` ci-dessous est
    // donc MORT pour ce chemin : mesuré, deux en-têtes distincts depuis un
    // pair de confiance rendent aujourd'hui `200` et créent un compte au
    // courriel joint (`"a@b.c, evil@x.y"`). **C'est un défaut PRÉEXISTANT,
    // reporté à la revue finale — non corrigé ici, seul ce commentaire l'est.**
    // Ce que ce garde referme réellement : le cas, différent, où un APPELANT
    // interne construit lui-même `entetes` avec un tableau (les tests de ce
    // fichier le font), et le précédent littéral de `porteur.ts` qui refuse
    // ainsi de désambiguïser une valeur ambiguë quand elle SE PRÉSENTE sous
    // cette forme.
    if (Array.isArray(brut) || brut === undefined) {
        return { ok: false, motif: 'identite-absente' };
    }
    const email = brut.trim();
    if (email === '') return { ok: false, motif: 'identite-absente' };
    return { ok: true, email };
}

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(JSON.stringify(corps));
}

/// Rend `true` si la requête a été servie.
///
/// 🔴 EN MODE `motdepasse`, ELLE REND LE `404` ELLE-MÊME — et c'est ce qui
/// porte le mode jusqu'au client : la page est bâtie statiquement par Vite et
/// ne peut lire aucune variable du serveur, alors elle DEMANDE. Un `403`
/// dirait « la route existe, tu n'y as pas droit », ce qui inviterait à
/// réessayer ; `404` dit la vérité.
///
/// 🔴 ELLE RENDAIT `false` JUSQU'AU 22 AOÛT 2026, POUR LAISSER RÉPONDRE LE 404
/// GÉNÉRIQUE DU SERVEUR — ET CE MÉCANISME EST MORT SANS BRUIT dans le lot
/// « page derrière Pomerium ». Le servant de fichiers, chaîné EN DERNIER,
/// replie tout chemin sans extension sur la page (`hub.html` depuis le
/// 30 août 2026, `index.html` avant — voir `page/resolution.ts::PAGE`) :
/// `GET /auth/moi` en mode `motdepasse` avec `PLATEFORME_PAGE` armée rendait
/// `200 text/html` (mesuré). Le client ne cassait que par accident — son
/// `.catch(() => undefined)` faisait tomber le formulaire au bon endroit.
///
/// ⚠️ CE N'EST PAS UN SECOND 404 : c'est LE MÊME, `http/introuvable.ts`, celui
/// que `serveur.ts` rend aussi. Un texte écrit à la main ici dériverait de
/// celui du serveur sans que rien ne le dise.
export async function servirIdentite(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesIdentite,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    // Comparaison EXACTE, jamais un `startsWith`.
    if (chemin !== CHEMIN_MOI) return false;
    // 🔴 L'INVARIANT DES DEUX GARDES DE MODE, ÉCRIT ICI ET DANS `routes-auth.ts`
    // PARCE QU'IL N'APPARTIENT NI À L'UN NI À L'AUTRE : **les deux gardes ont
    // des POLARITÉS OPPOSÉES** — celui-ci se retire si le mode n'est PAS
    // `pomerium`, celui de `routes-auth.ts` s'il n'est PAS `motdepasse` —, et
    // c'est ce qui les fait PARTITIONNER les modes : à DEUX modes, tout mode
    // ouvre exactement une des deux portes.
    //
    // 🔴 À TROIS MODES, LES DEUX RÉPONDENT `404` ENSEMBLE et le service n'a
    // plus AUCUNE route d'authentification, **en silence** : deux `404` justes
    // chacun pris seul, et rien qui dise qu'aucune porte n'est ouverte.
    // **Ajouter une valeur à `AUTHS` (`config.ts`) OBLIGE à revenir ici** et à
    // décider laquelle des deux portes le mode neuf ouvre — TypeScript ne le
    // demandera pas, ces gardes comparant des chaînes plutôt qu'un `switch`
    // exhaustif.
    if (deps.auth !== 'pomerium') {
        repondreIntrouvable(rep);
        return true;
    }

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    if (req.method === 'OPTIONS') {
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }
    if (req.method !== 'GET') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    // 🔴 LA GARDE QUI FERME LE CONTOURNEMENT. Sans elle, `/auth/moi` rend un
    // jeton interne valide pour N'IMPORTE QUEL courriel posé dans un en-tête
    // qu'AUCUNE SIGNATURE NE VÉRIFIE : quiconque atteint le port — donc la VM
    // Windows, que le § 7.1 de la spec `auth-pomerium` place nommément dans ce
    // périmètre — s'authentifie sous l'identité de son choix.
    //
    // ⚠️ ELLE EST PLACÉE AVANT LA LECTURE DE L'EN-TÊTE, PAS APRÈS. Après, elle
    // serait correcte aussi — mais le service aurait déjà lu une identité qu'il
    // refuse, et un successeur pourrait déplacer la lecture sans voir que la
    // garde en dépendait.
    //
    // ⚠️ CE QU'ELLE NE PROMET PAS : que seul Pomerium porte cette adresse. Cela
    // reste à la charge de l'exploitant, comme la garde d'écoute de
    // `PLATEFORME_HOTE` le dit déjà d'elle-même.
    if (!pairDeConfiance(req.socket.remoteAddress, deps.proxyDeConfiance)) {
        repondre(rep, 401, { refus: 'pair-non-de-confiance' }, cors);
        return true;
    }

    const identite = lireIdentitePomerium(req.headers);
    if (!identite.ok) {
        repondre(rep, 401, { refus: identite.motif }, cors);
        return true;
    }

    const maintenant = deps.maintenant();
    const id = await identifiantDe(deps.base, identite.email, maintenant);
    repondre(rep, 200, { acces: signer(id, deps.secretJeton, maintenant) }, cors);
    return true;
}

/// L'identifiant du compte, créé s'il n'existe pas.
///
/// ⚠️ LA SECONDE LECTURE N'EST PAS DÉFENSIVE, ELLE FERME UNE COURSE RÉELLE :
/// deux requêtes simultanées d'un même utilisateur inconnu passeraient toutes
/// deux la première lecture, et la seconde insertion violerait l'index UNIQUE
/// du courriel (`0001-socle.sql`) — un `500` sur la toute première ouverture
/// de page. On relit alors, et on ne relève l'erreur que si le compte est
/// toujours introuvable, auquel cas elle dit autre chose qu'une course.
async function identifiantDe(base: Pilote, email: string, maintenant: number): Promise<string> {
    const existant = await lireParEmail(base, email);
    if (existant !== undefined) return existant.id;
    try {
        return await creerUtilisateur(base, email, MARQUEUR_SANS_MOT_DE_PASSE, maintenant);
    } catch (cause) {
        const rattrape = await lireParEmail(base, email);
        if (rattrape !== undefined) return rattrape.id;
        throw cause;
    }
}
