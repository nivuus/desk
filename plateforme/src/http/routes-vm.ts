// `GET /vm` et `POST /vm/:id/:operation` — la surface HTTP de l'inventaire.
//
// 🔴 LE CONTRAT EST CELUI DE `routes-auth.ts` : `Promise<boolean>`, `true` =
// servie, `false` = pas mon chemin. Le 404 générique de `http/serveur.ts` est
// alors seul à répondre, et il n'est pas dupliqué ici.
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
// ⚠️ DIVERGENCE DÉCLARÉE AVEC LE SOUS-BLOC G1, non tranchée ici : sa décision
// D9 retient `403 {refus:'vm-etrangere'}` sur une VM appartenant à autrui,
// c'est-à-dire un ORACLE, distinct du 404 d'une VM inconnue. Les deux
// chantiers ne peuvent pas avoir raison en même temps. P4 retient le refus non
// énumérant ; unifier est une décision qui appartient au propriétaire du
// dépôt, pas à la seconde branche arrivée.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { compterOuvertesDe } from '../depot/session';
import { OPERATIONS_HTTP, type Operation } from '../orchestration/interface';
import { inventaireStatique } from '../orchestration/inventaire-statique';
import { BACKEND_STATIQUE, CODE_HTTP } from '../orchestration/refus';
import { vmsDe } from '../orchestration/selection';
import { entetesCors } from './cors';
import { lirePorteur } from './porteur';

export interface DependancesVm {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    maintenant: () => number;
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
        rep.writeHead(204, cors ?? {});
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
