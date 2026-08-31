// `POST /session` : l'utilisateur demande à ouvrir une session sur SA VM.
//
// 🔴 CE QUE CETTE ROUTE REND, ET CE QU'ELLE NE REND PAS. La spec §4 « P4 »
// écrit qu'elle rend « l'identifiant de session, le préfixe ET la
// configuration ICE ». Elle rend `{ vm, nom, prefixe, etat }`, et rien
// d'autre (divergence E4, décision D7). Deux raisons, dont la première est
// décisive :
//
// ① LA CONFIGURATION ICE EST PAR SESSION, et une route HTTP n'en connaîtrait
//    qu'une sur N. `signaling/ice.ts` compose son identifiant TURN à partir du
//    NOM DE SESSION et en signe le tout ; or une VM ouvre `<préfixe>:bureau`
//    PLUS une session par fenêtre (`<préfixe>:w-1`, `w-2`, …). La route ne
//    pourrait servir que la session de contrôle, et le relais continuerait de
//    servir toutes les autres. Un second chemin de délivrance qui couvre une
//    session sur N n'est pas une simplification : c'est un second endroit à
//    garder synchrone, dont on n'a pas le droit de se servir.
//
// ② LE NOM DE SESSION COMPOSÉ SERAIT UNE TROISIÈME COPIE DE `bureau`. La
//    constante vit déjà en Rust (`agent/src/superviseur/protocole.rs`,
//    `NOM_SESSION_DE_CONTROLE`) et, côté TypeScript, en littéral inline dans
//    `client/src/bureau/porteur-dom.ts` (`composer(deps.prefixe, 'bureau')` —
//    un `NOM_SESSION_DE_CONTROLE` nommé vivait dans `client/src/shell-page.ts`
//    avant que la tâche 9 ne l'inline, 31 août 2026), et la spec §2.6 nomme
//    déjà cette duplication comme un défaut connu.
//
// ⚠️ ELLE NE REND PAS NON PLUS `adresse` : c'est de la topologie interne dont
// le navigateur n'a aucun usage — il parle au signaling, jamais à la VM.
//
// ⚠️ AUCUN CODE CLIENT N'EST PRIVÉ DE QUOI QUE CE SOIT par cette décision, et
// c'est ce qui la rend gratuite : `client/src/prefixe.ts::composer` construit
// `<préfixe>:bureau`, et `client/src/webrtc.ts` reçoit son `ice-config` du
// relais comme aujourd'hui.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { inventaireStatique } from '../orchestration/inventaire-statique';
import { BACKEND_STATIQUE, CODE_HTTP } from '../orchestration/refus';
import { laVmDe } from '../orchestration/selection';
import { adresseSource } from './adresse-source';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';
import { ligne } from '../obs/journal';
import { lirePorteur } from './porteur';
import { BUDGET_REQUETES, cleRequetes, type Budget, type Frein } from '../securite/frein';

export interface DependancesSession {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    /// 🔴 L'HORLOGE VIENT D'ICI, jamais `Date.now()` lu dans ce module : c'est
    /// ce qui rend la transition du critère ④ observable dans une exécution de
    /// test, où il n'y aurait autrement qu'un seul instant.
    maintenant: () => number;
    /// 🔴 LE FREIN « TOUTE REQUÊTE », PARTAGÉ avec `routes-vm.ts` ET
    /// `signaling/relais.ts` — voir `securite/frein.ts::BUDGET_REQUETES`.
    /// Cette route n'a aucune notion d'échec : son abus est un VOLUME,
    /// jamais une suite de tentatives ratées.
    frein: Frein;
    /// Les proxys dont on croit l'en-tête `X-Forwarded-For` — même ensemble
    /// que `routes-auth.ts` et `routes-vm.ts`, jamais un second.
    proxyDeConfiance: ReadonlySet<string>;
}

const CHEMIN = '/session';

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

/// Enregistre la requête sur le budget « toute requête », et journalise SI ET
/// SEULEMENT SI le frein vient de mordre — même règle et même raison que
/// `routes-auth.ts::compterLEchec` et `routes-vm.ts::compterLaRequete` : la
/// requête suivante sera refusée tout en haut de `servirSession`, avant de
/// jamais rappeler cette fonction.
function compterLaRequete(
    frein: Frein,
    cles: readonly (readonly [string, Budget])[],
    adresse: string,
    instant: number,
): void {
    frein.echec(cles, instant);
    const apres = frein.consulter(cles, instant);
    if (!apres.freine) return;
    console.warn(
        ligne('frein-requetes', {
            route: CHEMIN,
            adresse,
            retry_apres_s: apres.retryApresS,
            entrees: frein.taille(),
            evictions: frein.evictions(),
        }),
    );
}

export async function servirSession(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesSession,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    // Comparaison EXACTE, jamais un `startsWith`.
    if (chemin !== CHEMIN) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    // La requête préalable : voir le commentaire jumeau de `routes-vm.ts`.
    // Sans elle, la route est inatteignable depuis un navigateur, l'en-tête
    // `Authorization` rendant la requête non simple.
    if (req.method === 'OPTIONS') {
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }
    if (req.method !== 'POST') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    // 🔴 LE FREIN « TOUTE REQUÊTE » EST CONSULTÉ ICI — AVANT `lirePorteur` et
    // avant tout accès à la base. Même position et même raison que
    // `routes-vm.ts` : compter après le travail qu'on borne ne le borne pas.
    const adresseRequete = adresseSource(
        req.socket.remoteAddress,
        Array.isArray(req.headers['x-forwarded-for'])
            ? req.headers['x-forwarded-for'].join(',')
            : req.headers['x-forwarded-for'],
        deps.proxyDeConfiance,
    );
    // 🔴 UNE `PLATEFORME_PROXY_DE_CONFIANCE` MAL POSÉE FAIT DÉGÉNÉRER CE
    // FREIN EN FREIN GLOBAL, ET SA GRAVITÉ A CHANGÉ AVEC CE LOT — voir le
    // paragraphe complet chez `routes-vm.ts` (même position, même clé
    // `BUDGET_REQUETES`, même témoin : la ligne `frein-requetes` qui nomme
    // l'adresse retenue), jamais recopié pour ne pas diverger.
    const clesRequetes: readonly (readonly [string, Budget])[] = [
        [cleRequetes(adresseRequete), BUDGET_REQUETES],
    ];
    const verdictRequetes = deps.frein.consulter(clesRequetes, deps.maintenant());
    if (verdictRequetes.freine) {
        rep.setHeader('Retry-After', String(verdictRequetes.retryApresS));
        repondre(rep, 429, { refus: 'trop-de-requetes' }, cors);
        return true;
    }
    compterLaRequete(deps.frein, clesRequetes, adresseRequete, deps.maintenant());

    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }

    // 🔴 LE CORPS DE LA REQUÊTE N'EST PAS LU, ET IL N'Y A RIEN À Y METTRE :
    // l'index partiel `vm_un_utilisateur` garantit zéro ou une VM par
    // utilisateur, donc il n'y a aucune VM à désigner. C'est ce qui dispense
    // cette route de la borne de 4 Kio de `routes-auth.ts` — ⚠️ ET LE JOUR OÙ
    // UN CORPS DEVIENDRA NÉCESSAIRE, LA BORNE LE DEVIENDRA AUSSI. Sans borne,
    // un pair authentifié ferait grossir la mémoire du service à volonté ; la
    // seule raison pour laquelle elle manque ici est qu'il n'y a rien à lire.
    const orchestrateur = inventaireStatique(deps.base, deps.maintenant);
    // `laVmDe` et non `find` : elle LÈVE si l'inventaire portait deux VMs pour
    // le même utilisateur, ce que l'index partiel rend impossible en base — et
    // si la base le portait quand même, c'est un défaut, pas une préférence à
    // exprimer par un choix silencieux.
    const sienne = laVmDe(await orchestrateur.lister(), porteur.utilisateurId);

    if (sienne === undefined) {
        // ③ Un listing vide n'est PAS un refus, et c'est pourquoi ce chemin
        // rend 409 et non 200 : un 200 ferait écrire une chaîne vide dans le
        // coffre du navigateur, `lirePrefixe` retomberait sur `''`, et la page
        // rejoindrait SILENCIEUSEMENT l'espace de noms partagé — la panne muette
        // que la spec §10 nomme.
        repondre(rep, CODE_HTTP['aucune-vm'], { motif: 'aucune-vm' }, cors);
        return true;
    }

    const etat = await orchestrateur.etat(sienne.id);
    // Le corps commun aux deux issues, écrit UNE SEULE FOIS pour qu'elles ne
    // puissent pas diverger sur le préfixe.
    const commun = { vm: sienne.id, nom: sienne.nom, prefixe: sienne.prefixe, etat };

    if (etat !== 'prete') {
        // ④ 503 : la VM est bien à cet utilisateur, elle ne répond pas. C'est un
        // état du monde, pas une erreur de la requête.
        //
        // 🔴 `redemarrage` EST L'AVEU, PAS LA FONCTION. Le cadrage promet
        // « VM injoignable -> le hub l'indique, PROPOSE REDÉMARRAGE » ; avec le
        // backend v1 le hub INDIQUE et dit qu'il ne sait pas redémarrer. Le
        // champ porte le MÊME motif et le MÊME backend que le refus typé de
        // `orchestrateur.demarrer`, dont il est la projection HTTP — et il est
        // rendu ici plutôt que laissé au navigateur à deviner, parce qu'une
        // absence de champ se lit comme un oubli.
        //
        // ⚠️ LE PRÉFIXE EST RENDU QUAND MÊME : il est connu et juste, et le
        // navigateur en a besoin pour ne pas rejoindre l'espace partagé en
        // attendant que la VM revienne.
        repondre(
            rep,
            CODE_HTTP['agent-injoignable'],
            {
                ...commun,
                motif: 'agent-injoignable',
                redemarrage: {
                    possible: false,
                    motif: 'non-supporte',
                    backend: BACKEND_STATIQUE,
                },
            },
            cors,
        );
        return true;
    }

    repondre(rep, 200, commun, cors);
    return true;
}
