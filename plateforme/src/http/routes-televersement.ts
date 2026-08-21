// Les QUATRE routes du téléversement d'un installeur : `POST /televersement`
// (déclarer), `PUT /televersement/:id/tranche/:n` (déposer),
// `GET /televersement/:id` (relire pour reprendre) et
// `POST /televersement/:id/sceller` (arrêter le contenu).
//
// 🔴 LE CONTRAT EST CELUI DES QUATRE ROUTEURS EXISTANTS : `Promise<boolean>`,
// `true` = servie, `false` = pas mon chemin. Le 404 générique de
// `http/serveur.ts` est alors SEUL à répondre, et il n'est pas dupliqué ici.
//
// 🔴 CE MODULE NE DÉCIDE NI DU DÉCOUPAGE, NI DE L'ÉCRITURE, NI DU PORTEUR :
// `proto/ts/tranches.ts` (PUR, et importé AUSSI par le navigateur),
// `apps/magasin-tranches.ts`, `http/porteur.ts`. En recopier un ici en ferait
// une seconde source de vérité — « un scellement qui refuse sans qu'on sache
// lequel des deux bouts a tort ».
//
// 🔴 LE VOCABULAIRE DE REFUS EST LOCAL, ET NE REJOINT PAS
// `orchestration/refus.ts`, qui est celui de l'ORCHESTRATION DES VMs — ses six
// motifs parlent tous de VM, d'agent ou d'attribution. `routes-icone.ts` a
// tranché pareil.
//
// ✅ LE `PUT` EST ATTEIGNABLE DEPUIS UN NAVIGATEUR EN ORIGINE CROISÉE, ET IL NE
// L'ÉTAIT PAS QUAND CE FICHIER A ÉTÉ ÉCRIT. `http/cors.ts` n'annonçait alors que
// `Access-Control-Allow-Methods: 'GET, POST, OPTIONS'` : le déposant ÉTANT le
// navigateur, et un `PUT` portant `Authorization` étant NON SIMPLE, il demandait
// la préalable, n'y trouvait pas `PUT`, et ABANDONNAIT SANS ENVOYER LA VRAIE
// REQUÊTE. La valeur porte désormais `PUT`, et `cors.test.ts` l'assère
// nommément — rouge vue, `2 failed | 6 passed`.
//
// ⚠️ CE QUI RESTE ENTIÈREMENT VRAI, ET QU'IL NE FAUT PAS LIRE COMME FERMÉ :
// **aucun test de Node ne peut voir cette classe de défaut**, `fetch` Node
// n'appliquant pas la politique d'origine. C'est la classe que la corroboration
// navigateur de P4 a trouvée et qu'elle a déclarée SANS GARDE AUTOMATIQUE ; elle
// a mordu deux fois chez P4 et une troisième fois ici, sur la MÉTHODE. Le seul
// garde est une assertion sur la VALEUR, dans `cors.test.ts`. Sans effet en
// origine unique (profil `deploiement` de P5) ; mordait en développement, `vite`
// servant sur 5173 et le service sur 8080 — donc là où on le met au point.
//
// ⚠️ `routes-auth.ts::lireCorps` N'EST NI RELEVÉ NI RÉEMPLOYÉ, ET LES DEUX
// MOITIÉS COMPTENT. Il accumule dans une CHAÎNE UTF-8 : un corps BINAIRE y
// serait corrompu (tout octet invalide devient U+FFFD, et l'empreinte du
// fichier recomposé ne serait plus la sienne), et relever son plafond rendrait
// à un pair anonyme le mégaoctet que ce plafond lui retire. Ici la déclaration
// a son lecteur borné, et la tranche passe EN FLUX, jamais par la mémoire.

import { createHash } from 'node:crypto';
import type { IncomingMessage, ServerResponse } from 'node:http';
import type { MagasinTranches } from '../apps/magasin-tranches';
import { identifiantValide, rangValide } from '../apps/magasin-tranches';
import type { Pilote } from '../base/pilote';
import {
    compterEnCours,
    creer,
    lireParId,
    sceller,
    type LigneTeleversement,
} from '../depot/televersement';
import { plan, verdict } from '../../../proto/ts/tranches';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';
import { lirePorteur } from './porteur';
import { empreinteValide, METHODE, reconnaitre } from './televersement-regles';

export interface DependancesTeleversement {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    /// ⚠️ SEUL CE ROUTEUR LE LIT — même statut que `magasin` pour `servirIcone`.
    tranches: MagasinTranches;
    maintenant: () => number;
}

/// Le pas de découpage. ⚠️ **NON CALIBRÉE**, et AUCUN rapport avec
/// `OCTETS_LECTURE` du navigateur (4 Mio) — même valeur laisserait croire à une
/// dérivation. 🔴 ELLE EST ÉCRITE EN BASE À LA CRÉATION, ET C'EST CETTE
/// COPIE-LÀ QUI FAIT FOI ENSUITE : la changer re-découperait les téléversements
/// DÉJÀ déclarés, dont `verdict` dirait toutes les tranches `incoherentes` —
/// un état qui NE SE RÉPARE PAS en redéposant.
export const TAILLE_TRANCHE = 8 * 1024 * 1024;

/// ⚠️ **NON CALIBRÉE**, MAJORANTE À VUE : aucune taille d'installeur réelle n'a
/// été relevée pour la poser. Rejoint la liste tenue depuis `BPP_MIN`.
export const TELEVERSEMENT_MAX_OCTETS = 4 * 1024 * 1024 * 1024;

/// Combien de téléversements NON SCELLÉS un utilisateur peut avoir de front.
/// 🔴 C'EST UN QUOTA, PAS UN FREIN — la distinction qu'écrit déjà
/// `depot/televersement.ts::compterEnCours` : le frein garde les portes
/// PRÉ-AUTHENTIFIÉES et compte des TENTATIVES ; les quatre routes d'ici exigent
/// un jeton valide, et ce qui les protège est une borne sur le DISQUE.
/// ⚠️ **NON CALIBRÉE.**
export const TELEVERSEMENTS_EN_COURS_MAX = 3;

/// ⚠️ Même nombre que le plafond de `routes-auth.ts` par COÏNCIDENCE de
/// grandeur, jamais par dérivation : les deux se recalibreraient séparément.
const CORPS_DECLARATION_MAX_OCTETS = 4 * 1024;

/// Ce que les quatre traitements partagent. ⚠️ `cors` est CALCULÉ UNE FOIS, en
/// tête : par branche, un chemin oublié répondrait sans en-tête d'origine.
interface Contexte {
    rep: ServerResponse;
    deps: DependancesTeleversement;
    cors: Record<string, string> | undefined;
}

/// Répond, et rend `true` — LE CONTRAT DU ROUTEUR. 🔴 LE RETOUR N'EST PAS UNE
/// COMMODITÉ D'ÉCRITURE : il rend impossible d'oublier un `return` après avoir
/// écrit la réponse. Sans lui, une branche qui répondrait puis retomberait sur
/// la suite écrirait une SECONDE réponse — au mieux `ERR_HTTP_HEADERS_SENT`, au
/// pire le 404 générique concaténé à un refus déjà parti.
function repondre(ctx: Contexte, code: number, corps?: unknown): true {
    ctx.rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ INCONDITIONNELS, sur TOUTE réponse — refus compris. Étalés AVANT
        // `cors`, FACULTATIF, qui ne doit jamais pouvoir les écraser. Et le
        // CORS va AUSSI sur les 401 : une réponse illisible par le navigateur
        // s'affiche en panne réseau, pas en invitation à se reconnecter.
        ...ENTETES_SECURITE,
        ...(ctx.cors ?? {}),
    });
    ctx.rep.end(corps === undefined ? undefined : JSON.stringify(corps));
    return true;
}

/* ── LA PROPRIÉTÉ ─────────────────────────────────────────────────────── */

/// Le refus, tel qu'il part sur le fil — LE MÊME dans les deux cas.
const REFUS_INCONNU = { refus: 'televersement-inconnu' } as const;

/// Lit la ligne, et n'en rend une QUE si elle appartient au demandeur.
///
/// 🔴 UN TÉLÉVERSEMENT D'AUTRUI EST INDISTINGUABLE D'UN TÉLÉVERSEMENT INCONNU,
/// ET C'EST UNE DÉCISION DÉJÀ PRISE, PAS UN ARBITRAGE ROUVERT ICI : les
/// distinguer serait un ORACLE D'ÉNUMÉRATION. Le propriétaire du dépôt a
/// tranché pour G1 (le `403 vm-etrangere` a cédé devant le `404 vm-inconnue`) ;
/// G3 APPLIQUE — quatrième fois, après `routes-auth.ts`, `agents/enrolement.ts`
/// et `routes-applications.ts`.
///
/// 🔴 LES DEUX BRANCHES RENDENT `undefined`, et le refus a UN SEUL site
/// d'émission : deux expressions, même rendant la même valeur, laisseraient la
/// porte ouverte à ce qu'une des deux change un jour. ⚠️ LA LIGNE DE JOURNAL
/// EST LA CONTREPARTIE, et elle N'ATTEINT JAMAIS LA RÉPONSE ; 🔴 posée ICI et
/// non aux trois points d'appel, où elle serait OUBLIABLE — et l'oublier ne
/// casserait rien de visible.
async function lireSienne(
    ctx: Contexte,
    id: string,
    utilisateurId: string,
): Promise<LigneTeleversement | undefined> {
    const ligne = await lireParId(ctx.deps.base, id);
    if (ligne === undefined) return journaliserLeRefus('inconnu', id, utilisateurId);
    if (ligne.utilisateur_id !== utilisateurId) {
        return journaliserLeRefus('etranger', id, utilisateurId);
    }
    return ligne;
}

/// ⚠️ `cas=` EST UN CHAMP, PAS UNE PHRASE : c'est lui que l'exploitant `grep`e.
/// Un test l'épingle, et épingle AUSSI que les deux cas diffèrent. ⚠️ Elle rend
/// `undefined` pour que tracer et refuser soient le MÊME geste.
function journaliserLeRefus(cas: 'inconnu' | 'etranger', id: string, u: string): undefined {
    console.warn(
        `refus d'accès au téléversement ${id} pour l'utilisateur ${u} : cas=${cas} — `
            + `la réponse HTTP, elle, est le même 404 « televersement-inconnu » dans les `
            + `deux cas (décision du propriétaire du dépôt : pas d'oracle d'énumération).`,
    );
    return undefined;
}

/* ── LE ROUTEUR ───────────────────────────────────────────────────────── */

export async function servirTeleversement(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesTeleversement,
): Promise<boolean> {
    const cible = reconnaitre(new URL(req.url ?? '/', 'http://placeholder').pathname);
    if (cible === undefined) return false;

    const ctx: Contexte = { rep, deps, cors: entetesCors(req.headers.origin, deps.origineClient) };

    // 🔴 LA PRÉALABLE EST SERVIE, ET SANS ELLE RIEN N'EST ATTEIGNABLE depuis un
    // navigateur : les quatre routes exigent `Authorization: Bearer`, donc la
    // requête est NON SIMPLE, et un 404 sur l'`OPTIONS` ferait abandonner le
    // navigateur AVANT la vraie requête. ⚠️ Voir l'en-tête pour `PUT`.
    if (req.method === 'OPTIONS') return repondre(ctx, 204);

    // Le chemin EXISTE, c'est la méthode qui ne convient pas : un 404 ferait
    // chercher une route absente.
    if (req.method !== METHODE[cible.quoi]) return repondre(ctx, 405, { refus: 'methode' });

    // 🔴 L'AUTHENTIFICATION VIENT AVANT TOUTE LECTURE DE BASE, DE DISQUE OU DE
    // CORPS : refuser après offrirait du travail gratuit à un pair anonyme —
    // et, sur le `PUT`, la possibilité d'écrire des mégaoctets avant le refus.
    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) return repondre(ctx, porteur.code, { refus: porteur.motif });
    const utilisateur = porteur.utilisateurId;

    if (cible.quoi === 'creer') return declarer(req, ctx, utilisateur);

    // 🔴 LA GARDE DE FORME PASSE AVANT TOUTE LECTURE. `:id` vient du RÉSEAU et
    // devient un NOM DE RÉPERTOIRE, où `..` est significatif ; le magasin LÈVE
    // sur un identifiant mal formé, donc sans elle une URL tordue rendrait 500.
    if (!identifiantValide(cible.id)) return repondre(ctx, 400, { refus: 'identifiant-invalide' });

    const ligne = await lireSienne(ctx, cible.id, utilisateur);
    if (ligne === undefined) return repondre(ctx, 404, REFUS_INCONNU);

    if (cible.quoi === 'etat') return repondre(ctx, 200, etatDe(ctx, ligne));
    if (cible.quoi === 'tranche') return deposer(req, ctx, ligne, cible.rang);
    return arreter(ctx, ligne);
}

/* ── ① DÉCLARER ───────────────────────────────────────────────────────── */

/// 🔴 LE PLAFOND EST VÉRIFIÉ PENDANT LA LECTURE, PAS APRÈS : accumuler d'abord
/// laisserait un pair remplir la mémoire avant le refus. ⚠️ LES OCTETS SONT
/// COMPTÉS, PAS LES CARACTÈRES — `routes-auth.ts` mesure une chaîne DÉJÀ
/// DÉCODÉE, ce qui sous-compte tout ce qui n'est pas ASCII.
async function lireDeclaration(req: IncomingMessage): Promise<string | 'trop-gros'> {
    const morceaux: Buffer[] = [];
    let total = 0;
    for await (const morceau of req) {
        const b = morceau as Buffer;
        total += b.length;
        // 🔴 ON SORT DE LA BOUCLE, ET ON N'APPELLE PAS `req.destroy()`. Sortir
        // suffit à cesser de lire — l'itérateur asynchrone détruit la partie
        // LISIBLE en se refermant —, tandis que `destroy()` abat le SOCKET, et
        // MESURÉ : le client reçoit alors `UND_ERR_SOCKET` au lieu du 413 que
        // l'on vient de décider. C'est le comportement de `routes-icone.ts` ;
        // `routes-auth.ts`, lui, appelle `destroy()`, et son propre test admet
        // en toutes lettres que « la connexion peut être coupée avant la
        // réponse ». Un refus qu'on ne peut pas lire n'est pas un refus.
        if (total > CORPS_DECLARATION_MAX_OCTETS) return 'trop-gros';
        morceaux.push(b);
    }
    return Buffer.concat(morceaux).toString('utf8');
}

/// Le motif du refus, ou rien. ⚠️ TROIS MOTIFS DISTINCTS PLUTÔT QU'UN `forme`
/// UNIQUE, et aucun oracle n'y est ouvert : ils parlent du contenu que le
/// demandeur VIENT D'ENVOYER. 🔴 `isSafeInteger` ET NON `isInteger` : au-delà de
/// 2^53 l'arithmétique du plan cesse d'être exacte et les deux bouts
/// divergeraient EN SILENCE — `proto/ts/tranches.ts` déclare cette borne
/// « nommée, pas gardée », elle est gardée ICI, seul endroit venu du FIL.
function motifDeDeclaration(c: Record<string, unknown>): string | undefined {
    if (typeof c.nom !== 'string' || c.nom === '') return 'nom-invalide';
    if (!Number.isSafeInteger(c.taille) || (c.taille as number) < 0) return 'taille-invalide';
    if (typeof c.sha256 !== 'string' || !empreinteValide(c.sha256)) return 'empreinte-invalide';
    return undefined;
}

async function declarer(req: IncomingMessage, ctx: Contexte, utilisateur: string): Promise<boolean> {
    const brut = await lireDeclaration(req);
    if (brut === 'trop-gros') return repondre(ctx, 413, { refus: 'corps-trop-grand' });

    let corps: unknown;
    try {
        corps = JSON.parse(brut);
    } catch {
        return repondre(ctx, 400, { refus: 'forme' });
    }
    if (typeof corps !== 'object' || corps === null || Array.isArray(corps)) {
        return repondre(ctx, 400, { refus: 'forme' });
    }

    const champs = corps as Record<string, unknown>;
    const motif = motifDeDeclaration(champs);
    if (motif !== undefined) return repondre(ctx, 400, { refus: motif });

    const taille = champs.taille as number;
    if (taille > TELEVERSEMENT_MAX_OCTETS) {
        return repondre(ctx, 413, { refus: 'trop-grand', maximum: TELEVERSEMENT_MAX_OCTETS });
    }

    // 🔴 LE QUOTA EST COMPTÉ AVANT LA CRÉATION : vérifier ensuite créerait la
    // ligne puis la retirerait, et une panne entre les deux laisserait
    // précisément le téléversement de trop.
    if ((await compterEnCours(ctx.deps.base, utilisateur)) >= TELEVERSEMENTS_EN_COURS_MAX) {
        const refus = { refus: 'trop-de-televersements', maximum: TELEVERSEMENTS_EN_COURS_MAX };
        return repondre(ctx, 429, refus);
    }

    const entree = {
        utilisateurId: utilisateur,
        nom: champs.nom as string,
        taille,
        sha256: champs.sha256 as string,
        tailleTranche: TAILLE_TRANCHE,
    };
    const ligne = await creer(ctx.deps.base, entree, ctx.deps.maintenant());

    // 201 : la ressource EST créée, son identifiant est dans le corps. Le
    // navigateur ne regarde que `r.ok`, que 200 et 201 satisfont tous deux.
    return repondre(ctx, 201, etatDe(ctx, ligne));
}

/* ── ② RELIRE ─────────────────────────────────────────────────────────── */

/// L'état d'un téléversement, tel que le navigateur le lit pour reprendre.
///
/// 🔴 `tranches_presentes` EST DÉRIVÉ DU DISQUE, JAMAIS D'UNE COLONNE —
/// décision D7 (`depot/televersement.ts`). ⚠️ Dérivé MÊME à la création, où il
/// vaut nécessairement `[]` : un `[]` en dur ferait de la réponse de création
/// une SECONDE expression de la même chose.
///
/// ⚠️ LA FORME EST `{n, octets}[]`, ET NON `number[]` : le navigateur accepte
/// les deux (`normaliserPresentes`), mais seule la première lui permet de VOIR
/// une tranche à la mauvaise taille AVANT de redéposer — avec des rangs nus il
/// déduirait les tailles du plan, donc supposerait justes celles qu'il devrait
/// vérifier, et l'incohérence n'apparaîtrait qu'au scellement.
///
/// ⚠️ `scelle_a` est une DATE ou `null` — jamais `0`, qui se lirait comme une
/// époque de 1970 (même raisonnement qu'`application.disparue_a`).
function etatDe(ctx: Contexte, ligne: LigneTeleversement): unknown {
    return {
        id: ligne.id,
        nom: ligne.nom,
        taille: ligne.taille,
        sha256: ligne.sha256,
        taille_tranche: ligne.taille_tranche,
        scelle_a: ligne.scelle_a,
        tranches_presentes: ctx.deps.tranches.lister(ligne.id),
    };
}

/* ── ③ DÉPOSER ───────────────────────────────────────────────────────── */

async function deposer(
    req: IncomingMessage,
    ctx: Contexte,
    ligne: LigneTeleversement,
    rangBrut: string,
): Promise<boolean> {
    // 🔴 LE RANG EST COMPARÉ À UNE SUITE DE CHIFFRES AVANT D'ÊTRE CONVERTI :
    // `Number` seul accepte `+1`, ` 1`, `0x10`, `1e3` et `Infinity`, qui
    // donneraient un `n` que `String(n)` ne réécrirait pas à l'identique — deux
    // URL distinctes désigneraient alors la même tranche.
    const n = Number(rangBrut);
    if (!/^\d+$/.test(rangBrut) || !rangValide(n)) {
        return repondre(ctx, 400, { refus: 'rang-invalide' });
    }

    // 🔴 ÉCRIRE DANS UN TÉLÉVERSEMENT SCELLÉ EST REFUSÉ, ET LE BRIEF NE LE
    // DEMANDAIT PAS : l'omettre ANNULERAIT LE SCELLEMENT SANS LE DIRE — un
    // dépôt postérieur remplacerait les octets vérifiés, et l'agent
    // installerait un contenu que personne n'a vu, sous une ligne qui affirme
    // le contraire.
    if (ligne.scelle_a !== null) return repondre(ctx, 409, { refus: 'deja-scelle' });

    // 🔴 UN RANG HORS DU PLAN EST REFUSÉ AU DÉPÔT, PAS AU SCELLEMENT : il ne
    // deviendra JAMAIS cohérent (`verdict` le dirait `incoherentes`, le verdict
    // qui ne se répare pas). Le refuser tout de suite évite d'écrire des octets
    // dont le seul avenir est de faire échouer le téléversement entier.
    const attendu = plan(ligne.taille, ligne.taille_tranche);
    if (n >= attendu.length) {
        return repondre(ctx, 409, { refus: 'rang-hors-plan', tranches: attendu.length });
    }

    // 🔴 LA BORNE EST DURE, RELUE EN BASE, ET C'EST `taille_tranche` — jamais la
    // taille ATTENDUE de cette tranche-ci : la dernière est plus courte que le
    // pas, et borner à sa taille exacte ferait de ce plafond un juge du
    // DÉCOUPAGE, rôle de `proto/ts/tranches.ts::verdict` et de lui seul. Ici on
    // borne le DISQUE ; une tranche trop courte passe et sera `incoherentes`.
    //
    // 🔴 LE CORPS N'EST JAMAIS TENU EN MÉMOIRE : `req` est un
    // `AsyncIterable<Uint8Array>` passé TEL QUEL au magasin. Un `Buffer.concat`
    // ferait du service une bombe mémoire pilotée par ses clients.
    const issue = await ctx.deps.tranches.ecrire(ligne.id, n, req, ligne.taille_tranche);
    if (!issue.ok) {
        // ⚠️ LE FICHIER PARTIEL EST DÉJÀ SUPPRIMÉ PAR LE MAGASIN — relu dans
        // `magasin-tranches.ts` : `rmSync(provisoire)` au `catch`, et le
        // `renameSync` n'a jamais eu lieu. Rien à SUPPOSER : un test relit
        // l'état après le refus.
        return repondre(ctx, 413, { refus: 'tranche-trop-grande', maximum: issue.plafond });
    }

    // ⚠️ LE DÉPÔT EST IDEMPOTENT, ET C'EST LE `renameSync` DU MAGASIN QUI LE
    // REND TEL : renommer sur un fichier existant le remplace — une tranche
    // déposée à moitié puis re-déposée en entier doit gagner.
    return repondre(ctx, 200, { n, octets: issue.octets });
}

/* ── ④ SCELLER ───────────────────────────────────────────────────────── */

async function arreter(ctx: Contexte, ligne: LigneTeleversement): Promise<boolean> {
    // ⚠️ SCELLER DEUX FOIS EST UN SUCCÈS, PAS UN CONFLIT, et sans recalculer :
    // l'empreinte a DÉJÀ été vérifiée et les dépôts sont refusés depuis. Un 409
    // ferait échouer le cas le plus banal — une réponse perdue, un client qui
    // réessaie.
    if (ligne.scelle_a !== null) return repondre(ctx, 200, scellement(ligne, ligne.scelle_a));

    // ⚠️ `plan` et `verdict` LÈVENT sur un contrat absurde, et c'est voulu : le
    // contrat vient de NOTRE base, pas du fil, et `declarer` l'a validé avant de
    // l'écrire. Une ligne au pas nul serait un défaut de PROGRAMME — frontière
    // posée par `proto/ts/tranches.ts` —, et le 500 est la réponse juste : le
    // déguiser en refus ferait recompléter des tranches qui n'existent pas.
    const attendu = plan(ligne.taille, ligne.taille_tranche);
    const v = verdict(ligne.taille, ligne.taille_tranche, ctx.deps.tranches.lister(ligne.id));

    // 🔴 DEUX VERDICTS, DEUX REFUS DISTINCTS, ET LES CONFONDRE SERAIT UNE BOUCLE
    // SANS FIN : un trou se comble en le redemandant, une tranche à la mauvaise
    // taille jamais — le déposant renverrait la même chose, indéfiniment. Tout
    // l'argument est dans `proto/ts/tranches.ts` ; ici on se contente de ne pas
    // aplatir ce qu'il a distingué.
    if (v.etat === 'incoherentes') {
        return repondre(ctx, 409, { refus: 'tranches-incoherentes', n: v.n });
    }
    if (v.etat === 'manquantes') {
        return repondre(ctx, 409, { refus: 'tranches-manquantes', n: v.n });
    }

    // 🔴 LA PLATEFORME RECALCULE, ELLE NE FAIT PAS CONFIANCE. Le `sha256` de la
    // ligne est ce que le DÉPOSANT a ANNONCÉ ; sceller sans le vérifier ferait
    // de la colonne une affirmation que rien n'a confrontée aux octets, et
    // l'agent installerait un fichier en croyant l'avoir vérifié. C'est le seul
    // contrôle de la chaîne qui ne puisse pas être satisfait par accident.
    //
    // 🔴 LES RANGS VIENNENT DU PLAN, PAS DU LISTAGE, et l'ordre EST le contrat :
    // une tranche disparue entre le verdict et la lecture devient une ERREUR de
    // flux, là où un listage frais se terminerait PROPREMENT en plus court — et
    // l'empreinte serait fausse sans qu'on sache pourquoi. ⚠️ EN FLUX : la
    // mémoire ne dépend pas de la taille du fichier.
    const condensat = createHash('sha256');
    const flux = ctx.deps.tranches.concatener(ligne.id, attendu.map((t) => t.n));
    for await (const morceau of flux) condensat.update(morceau as Uint8Array);
    const relu = condensat.digest('hex');

    if (relu !== ligne.sha256) {
        // ⚠️ NI L'EMPREINTE ANNONCÉE NI LA RELUE NE TRAVERSENT : la relue
        // décrirait le contenu réellement stocké à un pair qui pourrait n'en
        // avoir déposé qu'une partie. Le journal, lui, porte les deux.
        console.warn(
            `scellement refusé pour le téléversement ${ligne.id} : empreinte annoncée `
                + `${ligne.sha256}, empreinte relue ${relu} — les octets stockés ne sont `
                + `pas ceux que le déposant a annoncés.`,
        );
        return repondre(ctx, 409, { refus: 'empreinte' });
    }

    const instant = ctx.deps.maintenant();
    await sceller(ctx.deps.base, ligne.id, instant);
    return repondre(ctx, 200, scellement(ligne, instant));
}

/// ⚠️ LA MÊME EXPRESSION pour le scellement qui vient d'avoir lieu et pour celui
/// qui avait déjà eu lieu : sinon un client qui réessaie lirait deux formes du
/// même fait.
function scellement(ligne: LigneTeleversement, scelleA: number): unknown {
    return { id: ligne.id, taille: ligne.taille, sha256: ligne.sha256, scelle_a: scelleA };
}
