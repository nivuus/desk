// `GET /vm` et `POST /vm/:id/:operation` — la surface HTTP de l'inventaire.
//
// 🔴 LE CONTRAT EST CELUI DE `routes-auth.ts` : `Promise<boolean>`, `true` =
// servie, `false` = pas mon chemin. Le 404 générique de `http/serveur.ts` est
// alors seul à répondre, et il n'est pas dupliqué ici.
//
// ⚠️ « ALORS SEUL » N'EST PLUS VRAI SANS CONDITION DEPUIS LE 22 AOÛT 2026, et
// la phrase est laissée telle quelle parce qu'elle reste juste dans le montage
// nginx : quand `PLATEFORME_PAGE` est armée, un DIXIÈME routeur — le servant
// de page — est chaîné APRÈS tous les autres, et il résout n'importe quel
// chemin. Sur un `GET`/`HEAD`, c'est LUI qui répond `200 text/html` au `false`
// rendu ici ; hors `GET`/`HEAD` il se retire, et le 404 générique reprend la
// main. Voir `http/chaine.ts`, qui porte le compte et la règle.
//
// 🔴 `attribuer` N'EST PAS EXPOSÉE, et ce n'est pas un oubli. Il n'existe AUCUN
// rôle d'administration dans ce service : `identite/jeton.ts` ne connaît que
// `utilisateur` et `agent`, et `config.ts` n'a aucune variable
// d'administrateur. Une route d'attribution serait donc, au mieux, ouverte à
// tout utilisateur authentifié — une escalade de privilège offerte.
// L'attribution passe par `npm run admin:attribuer` (D8). La liste blanche
// `OPERATIONS_HTTP` est IMPORTÉE, jamais recopiée, et un test asserte
// nommément qu'`attribuer` n'y figure pas.
//
// 🔴 `vm-inconnue` COUVRE DEUX CAS — la VM n'existe pas, OU elle appartient à
// quelqu'un d'autre — et le corps est le MÊME, caractère pour caractère.
// Distinguer les deux ferait un ORACLE D'ÉNUMÉRATION : un utilisateur
// apprendrait quelles VMs existent en lisant le code de retour. Troisième
// application de la règle après `routes-auth.ts` et `agents/enrolement.ts`.
//
// 🔴 LA DIVERGENCE AVEC LE SOUS-BLOC G1 EST TRANCHÉE, PAR LE PROPRIÉTAIRE DU
// DÉPÔT, EN FAVEUR DE CE FICHIER — et ce module n'a donc pas changé d'une
// ligne. Sa décision D9 retenait `403 {refus:'vm-etrangere'}` sur une VM
// appartenant à autrui, c'est-à-dire un ORACLE D'ÉNUMÉRATION distinct du 404
// d'une VM inconnue ; `http/routes-applications.ts` s'est aligné sur le refus
// indistinguable ci-dessus, et le motif `vm-etrangere` n'existe plus nulle
// part dans le service.
//
// ⚠️ LA CONTREPARTIE VIT LÀ-BAS, PAS ICI, et c'est une asymétrie assumée :
// `routes-applications.ts` pose une ligne de journal qui nomme le cas réel,
// pour que l'exploitant garde le diagnostic que la réponse HTTP lui refuse.
// CE FICHIER N'EN A PAS, et pas par oubli — il ne SAIT pas distinguer les deux
// cas : sa recherche se fait dans `siennes`, où une VM d'autrui est absente
// exactement comme une VM inexistante. Il n'y a ici aucun verdict à
// journaliser, et en fabriquer un demanderait une seconde lecture de la base
// dont le seul usage serait la trace.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { compterOuvertesDe } from '../depot/session';
import { OPERATIONS_HTTP, type Operation } from '../orchestration/interface';
import { inventaireStatique } from '../orchestration/inventaire-statique';
import { BACKEND_STATIQUE, CODE_HTTP } from '../orchestration/refus';
import { vmsDe } from '../orchestration/selection';
import { adresseSource } from './adresse-source';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';
import { ligne } from '../obs/journal';
import { lirePorteur } from './porteur';
import { BUDGET_REQUETES, cleRequetes, type Budget, type Frein } from '../securite/frein';

export interface DependancesVm {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    maintenant: () => number;
    /// 🔴 LE FREIN « TOUTE REQUÊTE », PARTAGÉ avec `routes-session.ts` ET
    /// `signaling/relais.ts` — voir `securite/frein.ts::BUDGET_REQUETES`. Ni
    /// `GET /vm` ni `POST /session` n'ont de notion d'échec : leur abus est
    /// un VOLUME, jamais une suite de tentatives ratées, et c'est ce budget
    /// qui le borne — jamais `BUDGET_COMPTE` ni `BUDGET_ADRESSE`, qui
    /// comptent des ÉCHECS d'authentification et n'ont donc rien à voir ici.
    frein: Frein;
    /// Les proxys dont on croit l'en-tête `X-Forwarded-For` — même ensemble
    /// que `routes-auth.ts`, jamais un second : voir `http/adresse-source.ts`.
    proxyDeConfiance: ReadonlySet<string>;
}

const CHEMIN_LISTE = '/vm';

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ INCONDITIONNELS, et posés sur TOUTE réponse — y compris les
        // réponses d'ERREUR (401, 405, 413, 429, 500, 503), qui portent
        // souvent plus d'information qu'une réponse normale. Ils sont étalés
        // AVANT `cors` pour que la politique d'origine, qui est facultative,
        // ne puisse jamais les écraser par mégarde.
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(JSON.stringify(corps));
}

/// Reconnaît `/vm/:id/:operation`, et RIEN d'autre.
///
/// 🔴 LE CHEMIN EST DÉCOUPÉ PAR SEGMENTS, JAMAIS PAR `startsWith` — la règle
/// que `http/serveur.ts` s'impose déjà pour le routage des montées : un
/// préfixe ouvrirait une famille entière de chemins que personne n'a décidés.
///
/// 🔴 ET L'OPÉRATION EST FILTRÉE ICI, AVANT TOUT LE RESTE. Un verbe absent de
/// la liste blanche ne produit PAS un refus : il produit `undefined`, la route
/// rend `false`, et le 404 générique s'applique. Un 501 sur un verbe inventé
/// affirmerait que l'opération existe et n'est pas supportée, ce qui est faux.
/// ⚠️ Le servant de page ne le supplante pas ICI, et pour une raison précise
/// plutôt que par chance : ces chemins n'arrivent que par `POST`, et le
/// servant se retire hors `GET`/`HEAD`. Voir `http/chaine.ts`.
function operationDe(chemin: string): { vmId: string; operation: Operation } | undefined {
    const segments = chemin.split('/');
    // ['', 'vm', '<id>', '<operation>'] — exactement quatre, ni plus ni moins.
    if (segments.length !== 4 || segments[1] !== 'vm') return undefined;
    const [, , vmId, brut] = segments;
    if (vmId === '') return undefined;
    const operation = (OPERATIONS_HTTP as readonly string[]).includes(brut)
        ? (brut as Operation)
        : undefined;
    return operation === undefined ? undefined : { vmId, operation };
}

/// Enregistre la requête sur le budget « toute requête », et journalise SI ET
/// SEULEMENT SI le frein vient de mordre — même règle et même raison que
/// `routes-auth.ts::compterLEchec` : la requête suivante sera refusée tout en
/// haut de `servirVm`, avant de jamais rappeler cette fonction.
///
/// ⚠️ **CETTE LIGNE JOURNALISE À LA TRANSITION, ET NON À CHAQUE REQUÊTE
/// ADMISE — c'est ce qui la distingue d'une trace par paquet.** Une ligne à
/// CHAQUE requête, même après que le frein a commencé à refuser, ferait
/// écrire le service à un rythme que l'attaquant contrôle sans plus rien lui
/// coûter — la règle du chantier TURN (`CLAUDE.md`) : « compter ou
/// échantillonner, jamais tracer par paquet ». Journaliser à la transition
/// ferme cela : une adresse martelée écrit UNE ligne, jamais une par requête.
function compterLaRequete(
    frein: Frein,
    cles: readonly (readonly [string, Budget])[],
    adresse: string,
    instant: number,
    // 🔴 MINEUR CORRIGÉ (round de correction 1) : ce paramètre manquait, et
    // la ligne journalisait INCONDITIONNELLEMENT `CHEMIN_LISTE` (`/vm`),
    // y compris pour `POST /vm/:id/:operation` — la trace nommait la
    // mauvaise route.
    chemin: string,
): void {
    frein.echec(cles, instant);
    const apres = frein.consulter(cles, instant);
    if (!apres.freine) return;
    console.warn(
        ligne('frein-requetes', {
            route: chemin,
            adresse,
            retry_apres_s: apres.retryApresS,
            entrees: frein.taille(),
            evictions: frein.evictions(),
        }),
    );
}

export async function servirVm(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesVm,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    const action = operationDe(chemin);
    const estListe = chemin === CHEMIN_LISTE;
    if (!estListe && action === undefined) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    // 🔴 LA REQUÊTE PRÉALABLE EST SERVIE, ET SANS ELLE RIEN N'EST ATTEIGNABLE.
    // Les deux routes exigent `Authorization: Bearer`, ce qui rend la requête
    // NON SIMPLE : le navigateur émet d'abord un `OPTIONS`, et un 404 lui
    // ferait abandonner sans jamais envoyer la vraie requête. ⚠️ Le plan de P4
    // ne le prescrivait pas ; c'est un défaut relevé, pas recopié — jumeau de
    // celui de `Access-Control-Allow-Headers` (voir `cors.ts`).
    //
    // 204 même sans en-tête CORS : la requête préalable est servie, mais sans
    // autorisation le navigateur refusera la vraie requête — un refus BRUYANT,
    // que l'opérateur voit. Même choix que `routes-auth.ts`.
    if (req.method === 'OPTIONS') {
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }

    if (estListe && req.method !== 'GET') {
        // Le chemin EXISTE, c'est la méthode qui ne convient pas : un 404
        // ferait chercher une route absente.
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }
    if (action !== undefined && req.method !== 'POST') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    // 🔴 LE FREIN « TOUTE REQUÊTE » EST CONSULTÉ ICI — AVANT `lirePorteur`,
    // donc avant la moindre vérification HMAC et avant tout accès à la base.
    // Même position que le frein d'ÉCHECS de `routes-auth.ts`, et la même
    // raison : compter APRÈS le travail qu'on cherche à borner ne le borne
    // pas. `OPTIONS` ne consomme rien — la requête préalable ne coûte que
    // 204 octets et ne doit pas priver le navigateur de sa vraie requête.
    const adresseRequete = adresseSource(
        req.socket.remoteAddress,
        Array.isArray(req.headers['x-forwarded-for'])
            ? req.headers['x-forwarded-for'].join(',')
            : req.headers['x-forwarded-for'],
        deps.proxyDeConfiance,
    );
    // 🔴 **LA GRAVITÉ D'UNE `PLATEFORME_PROXY_DE_CONFIANCE` MAL POSÉE A
    // CHANGÉ AVEC CE LOT (round de correction 1, critique ③), ET C'EST ICI
    // QU'IL FAUT LE DIRE — c'est la première des trois consultations de
    // `BUDGET_REQUETES` (`routes-session.ts`, `signaling/relais.ts` la
    // renvoient à ce paragraphe).**
    //
    // Le piège lui-même n'est pas neuf : `adresseSource` (`http/
    // adresse-source.ts`) ne croit `X-Forwarded-For` QUE d'un pair dont
    // `remoteAddress` figure dans `proxyDeConfiance`. Un ensemble trop
    // large — ou une valeur qui n'est plus celle du proxy — fait que
    // `adresseRequete` devient la MÊME chaîne pour tout le monde : celle
    // que le premier arrivant a bien voulu écrire dans l'en-tête, ou celle
    // du proxy lui-même. Le frein par adresse dégénère alors en frein
    // GLOBAL, et le premier attaquant bloque tout le monde.
    //
    // 🔴 CE QUI EST NEUF : AVANT CE LOT, cette dégénérescence ne plafonnait
    // que `ECHECS_MAX_ADRESSE` = 50 échecs D'AUTHENTIFICATION par quart
    // d'heure à cette adresse fusionnée — gênant, borné aux routes
    // `/auth/*` et `/agent`. **DEPUIS CE LOT, LA MÊME DÉGÉNÉRESCENCE
    // PLAFONNE LE SERVICE ENTIER À `REQUETES_MAX_ADRESSE` ÉVÉNEMENTS PAR
    // FENÊTRE, HTTP ET WebSocket CONFONDUS** : `GET /vm`, `POST /session`
    // ET le relais `/signal` partagent ce même compteur (`cleRequetes`), à
    // cette même adresse fusionnée. Un pair anonyme ouvre ou consomme ce
    // budget commun, et plus personne — authentifié ou non — ne peut plus
    // ouvrir de VM, de session, ni de connexion `/signal` derrière ce proxy.
    //
    // ⚠️ **AGGRAVANT, ET IL FAUT LE DIRE AUSSI** : le profil livré
    // (`docker-compose.plateforme.yml`) pose `PLATEFORME_AUTH: motdepasse`,
    // mode où `PLATEFORME_PROXY_DE_CONFIANCE` est FACULTATIVE — rien
    // n'oblige à la poser correctement, ni même à la poser du tout ; elle
    // vit dans un fichier `.env` NON VERSIONNÉ, donc invisible à toute revue
    // de dépôt ; et sa valeur correcte est l'IP de CONTENEUR de nginx, qui
    // CHANGE quand le réseau docker est recréé — une valeur juste hier peut
    // être fausse aujourd'hui sans qu'aucun déploiement n'ait touché au
    // code.
    //
    // 🔴 **LE SEUL TÉMOIN** : la ligne `frein-requetes` que `compterLaRequete`
    // émet plus bas (et ses jumelles de `routes-session.ts` et
    // `relais.ts`), qui NOMME l'adresse retenue — voir son champ `adresse`.
    // Une même adresse sur toutes les lignes, tous chemins confondus, EST le
    // signal. Voir aussi `PLATEFORME_PROXY_DE_CONFIANCE` dans `CLAUDE.md`,
    // qui documente le second rôle de cette variable (l'autorisation
    // d'`X-Pomerium-Claim-Email`) — cette note-ci ne porte que sur le
    // premier, le crédit d'`X-Forwarded-For`.
    const clesRequetes: readonly (readonly [string, Budget])[] = [
        [cleRequetes(adresseRequete), BUDGET_REQUETES],
    ];
    const verdictRequetes = deps.frein.consulter(clesRequetes, deps.maintenant());
    if (verdictRequetes.freine) {
        rep.setHeader('Retry-After', String(verdictRequetes.retryApresS));
        repondre(rep, 429, { refus: 'trop-de-requetes' }, cors);
        return true;
    }
    compterLaRequete(deps.frein, clesRequetes, adresseRequete, deps.maintenant(), chemin);

    // 🔴 L'AUTHENTIFICATION VIENT AVANT TOUTE LECTURE DE BASE. Une route qui
    // lirait l'inventaire puis refuserait le jeton ne fuiterait rien par sa
    // réponse, mais elle offrirait un travail gratuit à un pair anonyme.
    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        // ⚠️ LES EN-TÊTES CORS SONT POSÉS SUR LE REFUS AUSSI : une 401 que le
        // navigateur ne peut pas lire s'affiche comme une panne réseau, pas
        // comme une invitation à se reconnecter.
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }

    const orchestrateur = inventaireStatique(deps.base, deps.maintenant);
    // Le filtrage est fait par le module PUR, jamais par une clause SQL écrite
    // ici : un défaut de filtre qui vivrait dans cette couche fuiterait
    // l'inventaire entier, et il n'y aurait aucun endroit où le rougir sans
    // monter un serveur.
    const siennes = vmsDe(await orchestrateur.lister(), porteur.utilisateurId);

    if (estListe) {
        // ⚠️ LE COMPTE EST CELUI DE L'UTILISATEUR, PAS CELUI DE LA VM, et il
        // n'est exact par VM que parce que l'index partiel `vm_un_utilisateur`
        // garantit AU PLUS UNE VM par utilisateur. Le jour où cet invariant
        // tomberait, ce champ deviendrait le total de l'utilisateur reporté sur
        // chaque ligne — donc faux. C'est écrit ici plutôt que découvert plus
        // tard ; `depot/session.ts` ne sait rien des VMs, sa table ne portant
        // `vm_id` que depuis P3 et pour la trace.
        const ouvertes = await compterOuvertesDe(deps.base, porteur.utilisateurId);
        const vms = [];
        for (const v of siennes) {
            vms.push({
                id: v.id,
                nom: v.nom,
                // L'état est DEMANDÉ à l'orchestrateur, seul détenteur de
                // l'horloge et du seuil : le recalculer ici dupliquerait la
                // règle, et les deux copies divergeraient le jour où l'une
                // changerait.
                etat: await orchestrateur.etat(v.id),
                prefixe: v.prefixe,
                // ⚠️ NI `adresse`, NI `utilisateurId` : la première est de la
                // topologie interne dont le navigateur n'a aucun usage (D7), la
                // seconde est celle du demandeur, qu'il connaît déjà.
                sessions_ouvertes: ouvertes,
            });
        }
        repondre(rep, 200, { vms }, cors);
        return true;
    }

    const { vmId, operation } = action!;
    // 🔴 « INCONNUE » ET « À QUELQU'UN D'AUTRE » SONT LE MÊME REFUS : la
    // recherche se fait dans `siennes`, donc une VM d'autrui est absente
    // exactement comme une VM inexistante, et le corps est produit par le même
    // chemin — il ne PEUT donc pas différer.
    // ⚠️ `BACKEND_STATIQUE` vient de `refus.ts`, jamais d'un littéral recopié :
    // c'est la MÊME constante que celle que l'orchestrateur met dans ses
    // refus, si bien que les deux corps ne peuvent pas diverger.
    if (!siennes.some((v) => v.id === vmId)) {
        repondre(
            rep,
            CODE_HTTP['vm-inconnue'],
            { motif: 'vm-inconnue', operation, backend: BACKEND_STATIQUE },
            cors,
        );
        return true;
    }

    // Les trois verbes refusent tous, et c'est le critère ①. Le `Resultat`
    // n'est pas reconstruit ici : il vient de l'orchestrateur, dont il porte le
    // nom de backend.
    const issue =
        operation === 'demarrer'
            ? await orchestrateur.demarrer(vmId)
            : operation === 'arreter'
              ? await orchestrateur.arreter(vmId)
              : await orchestrateur.instantane(vmId, '');
    if (issue.ok) {
        // Inatteignable avec le backend v1 — les trois verbes refusent. Écrit
        // quand même : le jour où un backend d'hyperviseur réussira, cette
        // branche existe et rend 200 plutôt qu'un `undefined` silencieux.
        repondre(rep, 200, { ok: true }, cors);
        return true;
    }
    repondre(
        rep,
        CODE_HTTP[issue.motif],
        { motif: issue.motif, operation: issue.operation, backend: issue.backend },
        cors,
    );
    return true;
}
