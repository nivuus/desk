// `GET /applications` et `POST /application/:id/lancer` — la surface HTTP du
// catalogue.
//
// 🔴 LE CONTRAT EST CELUI DE `routes-auth.ts` ET DE `routes-vm.ts` :
// `Promise<boolean>`, `true` = servie, `false` = pas mon chemin. Le 404
// générique de `http/serveur.ts` est alors seul à répondre, et il n'est pas
// dupliqué ici.
//
// ⚠️ « ALORS SEUL » N'EST PLUS VRAI SANS CONDITION DEPUIS LE 22 AOÛT 2026, et
// la phrase est laissée telle quelle parce qu'elle reste juste dans le montage
// nginx : quand `PLATEFORME_PAGE` est armée, un DIXIÈME routeur — le servant
// de page — est chaîné APRÈS tous les autres, et il résout n'importe quel
// chemin. Sur un `GET`/`HEAD`, c'est LUI qui répond `200 text/html` au `false`
// rendu ici ; hors `GET`/`HEAD` il se retire, et le 404 générique reprend la
// main. Voir `http/chaine.ts`, qui porte le compte et la règle.
//
// 🔴 L'AUTHENTIFICATION PASSE PAR `http/porteur.ts`, JAMAIS PAR UNE COPIE.
// Le plan de G1 (décision D9) prescrivait d'appeler `verifierJeton` puis
// d'exiger `verdict.type === 'utilisateur'` — c'est-à-dire de réécrire ici,
// mot pour mot, ce que P4 a livré entre-temps dans `lirePorteur`. Recopier une
// décision de sécurité est précisément ce que ce dépôt refuse : « une copie
// divergerait en silence » (`agents/canal.ts`), et celle-ci divergerait sur le
// jour où l'un des deux durcirait sa lecture de l'en-tête. Conséquence
// ASSUMÉE : les codes de refus sont ceux de `porteur.ts` — `401 jeton-absent`,
// `401 jeton-invalide`, `401 jeton-expire`, `403 jeton-agent` — et non le
// `401 {refus:'jeton'}` uniforme du plan. Le `403` sur un jeton d'agent est
// mieux argumenté que le `401` : le jeton est VALIDE, il n'est simplement pas
// celui d'un humain, et un 401 inviterait à se reconnecter pour rien.
//
// 🔴 DIVERGENCE DE SÉCURITÉ TRANCHÉE PAR LE PROPRIÉTAIRE DU DÉPÔT, ET
// C'EST CE MODULE QUI A CÉDÉ. Deux sous-blocs avaient livré deux réponses
// contradictoires à la même question — une demande portant sur une VM qui
// n'appartient pas au demandeur — et l'avaient chacun signalée sans la
// trancher : la décision D9 du plan de G1 retenait ici
// `403 {refus:'vm-etrangere'}`, DISTINCT du refus d'une VM inconnue, quand
// `http/routes-vm.ts` (sous-bloc P4) retenait un `404 vm-inconnue`
// INDISTINGUABLE, sur le modèle de `routes-auth.ts` et d'`agents/enrolement.ts`.
//
// LE PROPRIÉTAIRE DU DÉPÔT A RETENU LE REFUS INDISTINGUABLE, et la raison est
// que le `403` était un ORACLE D'ÉNUMÉRATION : il CONFIRMAIT l'existence d'une
// ressource à qui n'y a précisément aucun droit, si bien qu'un utilisateur
// apprenait par tâtonnement quelles VMs existent, sans jamais en voir une
// seule. Les deux routes de ce fichier rendent donc désormais le MÊME
// `404 {refus:'vm-inconnue'}`, produit par le même chemin, pour les deux cas.
//
// 🔴 LA CONTREPARTIE EST UNE LIGNE DE JOURNAL, ET ELLE N'EST PAS
// DÉCORATIVE : sans elle, uniformiser coûterait à l'exploitant tout le
// diagnostic — « la VM n'existe pas » et « elle est à quelqu'un d'autre » se
// liraient pareil des DEUX côtés, et plus rien ne distinguerait une erreur de
// saisie d'une tentative d'énumération. `acces` la pose, elle nomme le cas
// réel, et ⚠️ ELLE NE DOIT JAMAIS ATTEINDRE LA RÉPONSE HTTP : c'est tout
// l'objet. Un test asserte que le corps ne porte aucune trace du cas réel.
//
// ⚠️ LE PLAFOND DE CORPS DE `routes-auth.ts` NE S'APPLIQUE PAS ICI, et ne doit
// SURTOUT PAS être relevé : ces deux routes n'ont aucun corps significatif —
// l'une est un `GET`, l'autre ne porte que son chemin.

import type { IncomingMessage, ServerResponse } from 'node:http';
import { randomUUID } from 'node:crypto';
import type { RegistreAgents } from '../agents/registre';
import type { Pilote } from '../base/pilote';
import { associationsDe, lireParId, lireParVm, sourceMaxDepuis } from '../depot/application';
import { lireParId as lireVm } from '../depot/vm';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';
import { lirePorteur } from './porteur';

export interface DependancesApplications {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    registre: RegistreAgents;
    maintenant: () => number;
}

const CHEMIN_LISTE = '/applications';

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

/// Reconnaît `/application/:id/lancer`, et RIEN d'autre.
///
/// 🔴 LE CHEMIN EST DÉCOUPÉ PAR SEGMENTS, JAMAIS PAR `startsWith` — la règle
/// que `http/serveur.ts` et `http/routes-vm.ts` s'imposent déjà : un préfixe
/// ouvrirait une famille entière de chemins que personne n'a décidés. Le motif
/// est donc ancré des DEUX bouts : `/application/x/lancer/y` n'est pas servi,
/// et `/application/x` non plus.
function lancementDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'application', '<id>', 'lancer'] — exactement quatre.
    if (segments.length !== 4) return undefined;
    if (segments[1] !== 'application' || segments[3] !== 'lancer') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}

/// Décide si cet utilisateur a le droit de voir cette VM.
///
/// ⚠️ LA BRANCHE « NON ATTRIBUÉE » JOURNALISE, ET CE N'EST PAS DÉCORATIF.
/// `vm.utilisateur_id` est NULL après `npm run admin:agent` — relu :
/// `enrolerLaVm` fait `INSERT INTO vm(id, nom, adresse)` et ne passe jamais
/// d'utilisateur. Tant qu'aucune VM n'est attribuée, TOUT UTILISATEUR
/// AUTHENTIFIÉ VOIT TOUTES LES VMS : ce n'est PAS une isolation, et la ligne
/// de journal est la seule chose qui rende cet état visible à l'opérateur.
/// Servir en silence le rendrait invisible, et il le resterait jusqu'à ce que
/// quelqu'un lise ce fichier.
///
/// Le comportement se DURCIT TOUT SEUL le jour où le sous-bloc P4 remplira la
/// colonne : la branche NULL cessera d'être atteinte, sans qu'une ligne change
/// ici.
///
/// 🔴 `'etrangere'` SURVIT COMME VERDICT INTERNE ALORS QUE LE MOTIF DE FIL
/// `'vm-etrangere'` A DISPARU, et les deux faits ne se contredisent pas.
/// Le motif de fil n'avait plus que des sites d'émission morts une fois
/// l'uniformisation faite — un motif inatteignable est une variante morte que
/// le prochain lecteur croirait vivante, et il aurait pu la rendre à nouveau
/// atteignable en croyant réparer. Il est donc retiré, et il ne subsiste nulle
/// part : ce n'était un membre d'aucune union (`orchestration/refus.ts`
/// n'a jamais connu que `vm-inconnue`), seulement deux littéraux et leurs deux
/// assertions de test, toutes quatre parties avec lui.
/// Le VERDICT, lui, est ce qui alimente la ligne de journal : l'aplatir en
/// booléen supprimerait la seule distinction que la décision conserve
/// délibérément. Il est vivant, éprouvé, et il ne franchit jamais le fil.
async function acces(
    deps: DependancesApplications,
    vmId: string,
    utilisateurId: string,
): Promise<'ok' | 'inconnue' | 'etrangere'> {
    const vm = await lireVm(deps.base, vmId);
    if (vm === undefined) return journaliserLeRefus('inconnue', vmId, utilisateurId);
    if (vm.utilisateur_id === null) {
        console.warn(
            `vm non attribuee, acces accorde sans isolation a la VM ${vmId} `
                + `pour l utilisateur ${utilisateurId} (attribution = sous-bloc P4)`,
        );
        return 'ok';
    }
    return vm.utilisateur_id === utilisateurId
        ? 'ok'
        : journaliserLeRefus('etrangere', vmId, utilisateurId);
}

/// La contrepartie du refus indistinguable : la seule chose, dans tout le
/// service, qui dise LEQUEL des deux cas s'est produit.
///
/// 🔴 ELLE EST POSÉE DANS `acces`, ET PAS AUX POINTS D'APPEL. Les deux routes
/// de ce fichier refusent par le même chemin, et un troisième appelant
/// arrivera un jour ; poser la ligne chez l'appelant la rendrait
/// OUBLIABLE — et l'oublier ne casserait rien de visible, ce qui est
/// exactement le mode de défaillance que ce dépôt appelle une panne muette.
/// Ici, le refus et sa trace naissent ensemble ou pas du tout.
///
/// ⚠️ `cas=` EST UN CHAMP, PAS UNE PHRASE : c'est lui que l'exploitant `grep`e,
/// et une phrase se réécrit sans que rien ne casse. Un test l'épingle, et il
/// épingle AUSSI que les deux cas ne rendent pas le même — sans quoi une ligne
/// unique disant « refus » satisferait un contrôle qui ne chercherait que la
/// présence de la trace.
///
/// ⚠️ LE RETOUR EST LE VERDICT LUI-MÊME, pour que la ligne ne puisse pas
/// dériver de la décision qu'elle décrit : il n'existe aucun chemin où l'on
/// journalise un cas et où l'on rende l'autre.
function journaliserLeRefus(
    cas: 'inconnue' | 'etrangere',
    vmId: string,
    utilisateurId: string,
): 'inconnue' | 'etrangere' {
    console.warn(
        `refus d'accès à la VM ${vmId} pour l'utilisateur ${utilisateurId} : `
            + `cas=${cas} — la réponse HTTP, elle, est le même 404 « vm-inconnue » `
            + `dans les deux cas (décision du propriétaire du dépôt : pas d'oracle `
            + `d'énumération).`,
    );
    return cas;
}

export async function servirApplications(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesApplications,
): Promise<boolean> {
    const requete = new URL(req.url ?? '/', 'http://placeholder');
    const chemin = requete.pathname;
    const idApplication = lancementDe(chemin);
    const estListe = chemin === CHEMIN_LISTE;
    if (!estListe && idApplication === undefined) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    // 🔴 LA REQUÊTE PRÉALABLE EST SERVIE, ET SANS ELLE RIEN N'EST ATTEIGNABLE
    // depuis un navigateur : les deux routes exigent `Authorization: Bearer`,
    // ce qui rend la requête NON SIMPLE. Un 404 sur l'`OPTIONS` ferait
    // abandonner le navigateur avant même d'envoyer la vraie requête. Même
    // choix que `routes-vm.ts` et `routes-auth.ts`.
    if (req.method === 'OPTIONS') {
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }

    // Le chemin EXISTE, c'est la méthode qui ne convient pas : un 404 ferait
    // chercher une route absente.
    if (estListe && req.method !== 'GET') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }
    if (idApplication !== undefined && req.method !== 'POST') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    // 🔴 L'AUTHENTIFICATION VIENT AVANT TOUTE LECTURE DE BASE. Une route qui
    // lirait le catalogue puis refuserait le jeton ne fuiterait rien par sa
    // réponse, mais elle offrirait un travail gratuit à un pair anonyme.
    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        // ⚠️ LES EN-TÊTES CORS SONT POSÉS SUR LE REFUS AUSSI : une 401 que le
        // navigateur ne peut pas lire s'affiche comme une panne réseau, pas
        // comme une invitation à se reconnecter.
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }

    if (estListe) {
        const vmId = requete.searchParams.get('vm');
        if (vmId === null || vmId === '') {
            // 400 et non 404 : le chemin est bon, c'est la requête qui est
            // incomplète — et le dire évite de chercher une route absente.
            repondre(rep, 400, { refus: 'vm-absente' }, cors);
            return true;
        }
        const verdict = await acces(deps, vmId, porteur.utilisateurId);
        if (verdict !== 'ok') {
            // 🔴 LE VERDICT N'EST PAS RELU ICI, et c'est le fond de la
            // décision : « inconnue » et « étrangère » passent par la MÊME
            // expression, si bien que les deux corps ne PEUVENT pas différer.
            // Un ternaire, même rendant deux fois la même valeur, laisserait
            // la porte ouverte à ce qu'un jour l'une des deux branches
            // change. Même geste que `routes-vm.ts`.
            repondre(rep, 404, { refus: 'vm-inconnue' }, cors);
            return true;
        }
        const lignes = await lireParVm(deps.base, vmId);
        // 🔴 UNE SEULE REQUÊTE POUR TOUTES LES ASSOCIATIONS. Les demander
        //    application par application ferait 156 allers-retours par
        //    affichage du hub, sur le corpus de la VM de développement.
        const associations = await associationsDe(
            deps.base,
            lignes.map((l) => l.id),
        );
        // ⚠️ NI `cible`, NI `arguments`, NI `repertoire`, NI `chemin` : ce sont
        // des chemins du DISQUE DE LA VM, dont le navigateur n'a aucun usage
        // et qui décrivent l'intérieur d'une machine. Le lancement se fait par
        // l'identifiant, jamais par un chemin que le client fournirait — c'est
        // ce qui empêche une page de demander l'exécution d'un programme
        // arbitraire. Même discipline que `routes-vm.ts`, qui tait `adresse`.
        repondre(
            rep,
            200,
            {
                applications: lignes.map((l) => ({
                    id: l.id,
                    nom: l.nom,
                    // ⚠️ CES DEUX-LÀ TRAVERSENT, ET LE RAISONNEMENT CI-DESSUS
                    // NE S'Y OPPOSE PAS : une empreinte n'est le chemin de
                    // rien, et `source_max` est une propriété de l'IMAGE. Ce
                    // que le paragraphe précédent tait, ce sont des chemins du
                    // disque de la VM ; ceux-ci n'en sont pas.
                    //
                    // 🔴 `source_max` PASSE PAR `sourceMaxDepuis`, ÉCRITE UNE
                    // SEULE FOIS AU DÉPÔT. Reconstruire ici ferait vivre la
                    // règle `null -> non-mesuree` à deux endroits, et les deux
                    // divergeraient le jour où l'une déciderait que `null`
                    // vaut `0` — c'est-à-dire ferait dire à une provenance
                    // INCONNUE qu'elle vaut quelque chose.
                    icone: l.icone,
                    source_max: sourceMaxDepuis(l.source_max_px),
                    // ⚠️ CES DEUX-LÀ TRAVERSENT AUSSI, ET LE RAISONNEMENT
                    // CI-DESSUS NE S'Y OPPOSE PAS : une couleur n'est le
                    // chemin de rien, et une extension de fichier est une
                    // convention publique — ni l'une ni l'autre ne décrit
                    // l'intérieur d'une machine.
                    //
                    // 🔴 `associations` EST TOUJOURS UN TABLEAU, JAMAIS ABSENT.
                    // Une application sans association est le cas le plus
                    // fréquent, et le navigateur ne doit pas avoir à
                    // distinguer « aucune » de « pas renseigné ».
                    accent: l.accent,
                    associations: associations.get(l.id) ?? [],
                })),
            },
            cors,
        );
        return true;
    }

    const application = await lireParId(deps.base, idApplication!);
    if (application === undefined) {
        repondre(rep, 404, { refus: 'application-inconnue' }, cors);
        return true;
    }
    const verdict = await acces(deps, application.vm_id, porteur.utilisateurId);
    if (verdict !== 'ok') {
        // 🔴 SANS CETTE GARDE, UN IDENTIFIANT D'APPLICATION SUFFIRAIT À LANCER
        // UN PROGRAMME SUR LA MACHINE DE QUELQU'UN D'AUTRE — et l'agent, lui,
        // n'a aucun moyen de savoir qui a demandé : il exécute ce qu'on lui
        // dit d'exécuter.
        // Le refus est celui de la liste, mot pour mot : un seul motif, un
        // seul code, pour les deux routes comme pour les deux cas.
        repondre(rep, 404, { refus: 'vm-inconnue' }, cors);
        return true;
    }

    // 🔴 LA DEMANDE EST TIRÉE ICI, et elle est ce qui apparie l'ordre à sa
    // réponse. Employer la clé à sa place mélangerait deux lancements
    // concurrents de la même application — deux onglets du hub suffisent.
    const issue = await deps.registre.lancer(application.vm_id, application.cle, randomUUID());

    if (issue === 'agent-injoignable') {
        // 503 : le service va bien, c'est la VM qui ne répond pas. Rendre 200
        // ferait afficher au hub un succès pour un lancement qui n'a pas eu
        // lieu — la panne la plus difficile à diagnostiquer qui soit, parce
        // que rien nulle part ne la contredit.
        repondre(rep, 503, { refus: 'agent-injoignable' }, cors);
        return true;
    }
    if (issue === 'delai') {
        // 504 : l'ordre est PARTI, et personne n'a répondu. ⚠️ Il n'est pas
        // annulé pour autant — l'agent peut très bien avoir lancé
        // l'application et avoir répondu trop tard. Le code dit « je ne sais
        // pas », jamais « ça n'a pas eu lieu ».
        repondre(rep, 504, { refus: 'delai' }, cors);
        return true;
    }

    // 🔴 L'ISSUE EST RENDUE TELLE QUELLE, jamais aplatie en booléen :
    // `raccourci` contre `cible` est ce qui dit si c'est bien le `.lnk` qu'on a
    // lancé ou une cible reconstruite, et un booléen ferait perdre au critère
    // de recette toute discrimination.
    //
    // ⚠️ `echec` REND 200, ET C'EST DÉLIBÉRÉ : l'ordre a abouti, la plateforme
    // a fait son travail, et l'agent a répondu. Une 5xx dirait que le SERVICE
    // a échoué, ce qui est faux — et rendrait `echec` indistinguable
    // d'`agent-injoignable`, alors que ce sont deux situations opposées.
    repondre(rep, 200, { issue }, cors);
    return true;
}
