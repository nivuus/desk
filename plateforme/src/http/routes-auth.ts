// Les deux routes d'authentification : `POST /auth/connexion` et
// `POST /auth/rafraichir`.
//
// 🔴 LE MESSAGE DE REFUS EST IDENTIQUE pour « courriel inconnu » et « mot de
// passe faux » — `{refus:'identifiants'}`. Un message qui les distinguerait
// serait un ORACLE d'énumération de comptes : l'attaquant apprendrait quelles
// adresses existent en lisant la réponse. C'est la même règle que le critère ②
// de P3, posée ici parce que le premier cas où elle mord est celui-ci.
//
// ⚠️ ET LE COÛT DU CHEMIN EST ÉGALISÉ AUSSI : sur un courriel inconnu, la
// route hache quand même un mot de passe leurre, pour que la durée de réponse
// ne trahisse pas l'existence du compte. **CETTE ÉGALISATION N'EST PAS
// MESURÉE**, et ne le sera pas : un test de temporisation serait instable, et
// la spec §8 range déjà les attaques temporelles parmi ce que ⑤ n'éprouve pas.
// Ce qui EST testé est le message identique, qui est décidable. Écrire
// « égalisé » sans cette réserve serait une affirmation au-delà du relevé.
//
// 🔴 AUCUN MOT DE PASSE N'APPARAÎT DANS UNE TRACE NI DANS UNE RÉPONSE
// (critère ④). Aucune ligne de ce fichier ne journalise un corps de requête,
// et c'est délibéré : journaliser `JSON.stringify(corps)` pour diagnostiquer
// écrirait le mot de passe en clair dans `agent.log`. Le test du critère ④
// balaie LE CHAMP (`motdepasse`, `mot_de_passe`, `empreinte_mdp`) et non la
// valeur, précisément pour attraper ce geste-là.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { emettre, tourner } from '../depot/jeton';
import { lireParEmail, remplacerEmpreinte } from '../depot/utilisateur';
import { DUREE_JETON_ACCES_MS, signer } from '../identite/jeton';
import { doitEtreRehache, hacher, verifier } from '../identite/mot-de-passe';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';
import { adresseSource } from './adresse-source';
import { ligne } from '../obs/journal';
import {
    BUDGET_ADRESSE,
    BUDGET_COMPTE,
    cleAdresse,
    cleCompte,
    type Budget,
    type Frein,
} from '../securite/frein';

export interface DependancesAuth {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    maintenant: () => number;
    /// Le frein, PARTAGÉ avec le canal `/agent` — une seule table, jamais
    /// deux (voir `securite/frein.ts`).
    frein: Frein;
    /// Les proxys dont on croit l'en-tête `X-Forwarded-For`. VIDE par défaut :
    /// on ne croit personne (`config.ts`).
    proxyDeConfiance: ReadonlySet<string>;
}

/// Les clés à consulter pour une requête, et celle qu'un succès efface.
interface ContexteFrein {
    cles: readonly (readonly [string, Budget])[];
    /// L'adresse RETENUE par `adresseSource` — celle que la trace nomme.
    adresse: string;
    /// ⚠️ RENSEIGNÉE POUR `/auth/connexion` SEULEMENT. `/auth/rafraichir` n'a
    /// pas de courriel à présenter — seulement un jeton opaque —, et prendre
    /// ce jeton pour clé reviendrait à INDEXER UNE TABLE SUR UN SECRET.
    cleDuCompte?: string;
}

/// 4 KiB. Un corps d'authentification honnête pèse quelques centaines
/// d'octets ; sans borne, un pair ANONYME — la route est ouverte, c'est son
/// objet — ferait grossir la mémoire du service à volonté.
/// ⚠️ NON CALIBRÉE : c'est une borne généreuse, pas une mesure.
const CORPS_MAX_OCTETS = 4 * 1024;

const CHEMINS = new Set(['/auth/connexion', '/auth/rafraichir']);

/// Le mot de passe leurre haché sur un courriel inconnu. Calculé UNE fois et
/// mémorisé : le hacher à chaque requête coûterait le même temps, mais le
/// calculer ici garde le coût du chemin « compte inexistant » comparable à
/// celui du chemin « compte existant ».
const LEURRE = 'un-mot-de-passe-leurre-qui-n-est-a-personne';

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

/// Lit le corps, ou rend `undefined` si la borne est franchie — auquel cas la
/// requête est ABANDONNÉE sans lire la suite, plutôt que d'accumuler.
function lireCorps(req: IncomingMessage): Promise<string | undefined> {
    return new Promise((resolve, rejeter) => {
        let recu = '';
        req.on('data', (morceau: Buffer) => {
            recu += morceau.toString('utf8');
            if (recu.length > CORPS_MAX_OCTETS) {
                // On cesse de lire IMMÉDIATEMENT : continuer à accumuler pour
                // répondre poliment serait exactement le déni de service que
                // la borne existe pour empêcher.
                req.destroy();
                resolve(undefined);
            }
        });
        req.on('end', () => resolve(recu));
        req.on('error', rejeter);
    });
}

function estObjet(v: unknown): v is Record<string, unknown> {
    return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/// Rend `true` si la requête a été servie, `false` si elle ne concerne pas
/// l'authentification — le serveur répond alors 404, comme aujourd'hui.
export async function servirAuth(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesAuth,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    if (!CHEMINS.has(chemin)) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    if (req.method === 'OPTIONS') {
        // 204 même sans en-tête CORS : la requête préalable est servie, mais
        // sans autorisation le navigateur refusera la vraie requête — un refus
        // BRUYANT, que l'opérateur voit (voir `config.ts`).
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }

    if (req.method !== 'POST') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    const brut = await lireCorps(req);
    if (brut === undefined) {
        repondre(rep, 413, { refus: 'corps-trop-grand' }, cors);
        return true;
    }

    let corps: unknown;
    try {
        corps = JSON.parse(brut);
    } catch {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return true;
    }
    if (!estObjet(corps)) {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return true;
    }

    // 🔴 LE FREIN EST CONSULTÉ ICI, ET C'EST LA POSITION QUI COMPTE : AVANT
    // `lireParEmail`, donc AVANT le moindre accès à la base, ET AVANT
    // `verifier`/`hacher`, donc AVANT LA DÉRIVATION `scrypt`. `scrypt` est à
    // mémoire dure et coûte délibérément cher (68 ms mesurés le 20 août 2026
    // sur cette machine) : un attaquant qui le déclenche à volonté épuise le
    // service sans jamais deviner un secret. UN FREIN POSTÉ APRÈS LA
    // VÉRIFICATION NE PROTÈGE RIEN — il compte des échecs qu'il a déjà payés.
    //
    // Le corps est lu d'abord, parce que la clé de compte en dépend ; il est
    // borné à `CORPS_MAX_OCTETS` et ne coûte donc rien de comparable.
    const contexte = clesDe(chemin, corps, req, deps);
    const verdict = deps.frein.consulter(contexte.cles, deps.maintenant());
    if (verdict.freine) {
        // ⚠️ LE 429 PORTE LES EN-TÊTES CORS COMME TOUTES LES AUTRES RÉPONSES.
        // Sans eux, le NAVIGATEUR ne peut pas lire le refus : l'utilisateur
        // voit un échec opaque au lieu de « réessayez dans n minutes ».
        rep.setHeader('Retry-After', String(verdict.retryApresS));
        repondre(rep, 429, { refus: 'trop-de-tentatives' }, cors);
        return true;
    }

    if (chemin === '/auth/connexion') {
        await connexion(corps, rep, deps, cors, contexte);
    } else {
        await rafraichir(corps, rep, deps, cors, contexte);
    }
    return true;
}

/// Construit les clés de frein d'une requête.
///
/// ⚠️ `/auth/connexion` PORTE DEUX CLÉS, `/auth/rafraichir` UNE SEULE (D1) :
///   - par COMPTE, seul frein qui ferme la force brute CIBLÉE — un attaquant
///     disposant de mille adresses source n'en est pas ralenti autrement ;
///   - par ADRESSE, seul frein qui ferme le BALAYAGE de comptes — mille
///     courriels essayés une fois chacun ne consomment aucun budget de compte.
function clesDe(
    chemin: string,
    corps: Record<string, unknown>,
    req: IncomingMessage,
    deps: DependancesAuth,
): ContexteFrein {
    const adresse = adresseSource(
        req.socket.remoteAddress,
        // Node rend `string[]` si l'en-tête est répété. Le concaténer avec des
        // virgules le ramène à la forme d'un en-tête unique, que
        // `adresseSource` sait lire — et dont il prend le DERNIER élément,
        // c'est-à-dire celui que le proxy le plus proche a écrit.
        Array.isArray(req.headers['x-forwarded-for'])
            ? req.headers['x-forwarded-for'].join(',')
            : req.headers['x-forwarded-for'],
        deps.proxyDeConfiance,
    );
    const parAdresse: readonly [string, Budget] = [cleAdresse(adresse), BUDGET_ADRESSE];

    // Le courriel n'est une clé que s'il est une chaîne : un corps mal formé
    // sera refusé en 400 plus bas, et n'a pas à consommer de budget de compte.
    const email = corps.email;
    if (chemin !== '/auth/connexion' || typeof email !== 'string') {
        return { cles: [parAdresse], adresse };
    }
    const cle = cleCompte(email);
    return { cles: [[cle, BUDGET_COMPTE], parAdresse], adresse, cleDuCompte: cle };
}

/// Enregistre l'échec, et journalise SI ET SEULEMENT SI le frein vient de
/// mordre.
///
/// 🔴 POURQUOI PAS UNE LIGNE PAR REFUS. Une trace émise à chaque 429 rendrait
/// le service AMPLIFICATEUR sur le chemin même qu'on ferme : un attaquant à
/// dix mille requêtes par seconde ferait écrire dix mille lignes par seconde,
/// pour des requêtes qui, elles, ne coûtent plus rien. `CLAUDE.md` porte la
/// règle depuis le chantier TURN — « compter ou échantillonner, jamais tracer
/// par paquet », après qu'une trace par paquet a écrit 18 619 lignes en
/// quelques secondes et détruit la mesure qu'elle servait.
///
/// La transition est détectée en reconsultant APRÈS l'échec : la requête
/// suivante étant refusée tout en haut de `servirAuth`, elle n'atteindra
/// jamais cette fonction. Il y a donc EXACTEMENT UNE ligne par clé et par
/// fenêtre, borne que `ENTREES_MAX` referme.
///
/// ⚠️ LA LIGNE PORTE LE COURRIEL VISÉ, et c'est un arbitrage : savoir QUEL
/// compte est attaqué est précisément ce dont un exploitant a besoin. Aucun
/// mot de passe n'y figure — critère ④ —, et la clé est déjà normalisée.
function compterLEchec(deps: DependancesAuth, contexte: ContexteFrein, chemin: string): void {
    const instant = deps.maintenant();
    deps.frein.echec(contexte.cles, instant);
    const apres = deps.frein.consulter(contexte.cles, instant);
    if (!apres.freine) return;
    // ⚠️ L'ADRESSE EST NOMMÉE, ET C'EST LE SEUL REMÈDE au mode de défaillance
    // de `http/adresse-source.ts` : un exploitant qui a posé un proxy sans
    // déclarer sa confiance verra ici l'adresse de son proxy sur toutes les
    // lignes, et comprendra que son frein par adresse est devenu GLOBAL.
    console.warn(
        ligne('frein', {
            route: chemin,
            adresse: contexte.adresse,
            cles: contexte.cles.map(([cle]) => cle).join(' '),
            retry_apres_s: apres.retryApresS,
            // ⚠️ `entrees` ET `evictions` SONT LÀ POUR QUE LA SATURATION DU
            // FREIN CESSE D'ÊTRE INVISIBLE. Sous saturation, une éviction rend
            // son budget à un compte visé (voir `ENTREES_MAX`) : un exploitant
            // qui voit `evictions` monter sait que le frein est débordé, et
            // que ses budgets ne valent plus ce qu'ils annoncent.
            entrees: deps.frein.taille(),
            evictions: deps.frein.evictions(),
        }),
    );
}

async function connexion(
    corps: Record<string, unknown>,
    rep: ServerResponse,
    deps: DependancesAuth,
    cors: Record<string, string> | undefined,
    contexte: ContexteFrein,
): Promise<void> {
    const { email, motdepasse } = corps;
    if (typeof email !== 'string' || typeof motdepasse !== 'string') {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return;
    }

    const utilisateur = await lireParEmail(deps.base, email);
    if (!utilisateur) {
        // Voir l'en-tête : le coût du chemin est égalisé, l'égalisation n'est
        // PAS mesurée, et le message est le même que pour un mot de passe faux.
        await hacher(LEURRE);
        // ⚠️ UN COURRIEL INCONNU COMPTE COMME UN ÉCHEC, exactement comme un
        // mot de passe faux. Ne compter que les comptes existants rouvrirait
        // l'ORACLE que cette route ferme sur trois paragraphes : le balayage
        // d'un million d'adresses ne consommerait alors aucun budget.
        compterLEchec(deps, contexte, '/auth/connexion');
        repondre(rep, 401, { refus: 'identifiants' }, cors);
        return;
    }

    let bon: boolean;
    try {
        bon = await verifier(motdepasse, utilisateur.empreinte_mdp);
    } catch (cause) {
        // `verifier` LÈVE sur un algorithme inconnu — une base écrite par une
        // version future. C'est un défaut de données, pas une entrée fautive :
        // il se journalise SANS le corps de la requête, et la réponse reste
        // celle des identifiants, pour ne pas devenir un oracle.
        console.error(`empreinte illisible pour un compte existant : ${String(cause)}`);
        compterLEchec(deps, contexte, '/auth/connexion');
        repondre(rep, 401, { refus: 'identifiants' }, cors);
        return;
    }
    if (!bon) {
        compterLEchec(deps, contexte, '/auth/connexion');
        repondre(rep, 401, { refus: 'identifiants' }, cors);
        return;
    }

    // Le re-hachage à la connexion suivante : c'est ce qui rendra inutile
    // toute migration de données le jour où les paramètres changeront.
    if (doitEtreRehache(utilisateur.empreinte_mdp)) {
        await remplacerEmpreinte(deps.base, utilisateur.id, await hacher(motdepasse));
    }

    // 🔴 LE SUCCÈS N'EFFACE QUE LA CLÉ DE COMPTE, JAMAIS CELLE DE L'ADRESSE.
    // L'effacer aussi BLANCHIRAIT un attaquant qui possède un compte valide :
    // il lui suffirait de s'y connecter entre deux rafales pour rendre son
    // budget d'adresse à zéro, et le frein par adresse ne fermerait plus rien.
    if (contexte.cleDuCompte !== undefined) deps.frein.succes(contexte.cleDuCompte);

    // Une connexion ouvre une famille NEUVE — c'est le seul geste qui le
    // fasse.
    await delivrer(rep, deps, utilisateur.id, await emettre(deps.base, utilisateur.id, deps.maintenant()), cors);
}

async function rafraichir(
    corps: Record<string, unknown>,
    rep: ServerResponse,
    deps: DependancesAuth,
    cors: Record<string, string> | undefined,
    contexte: ContexteFrein,
): Promise<void> {
    const { rafraichissement } = corps;
    if (typeof rafraichissement !== 'string') {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return;
    }

    const issue = await tourner(deps.base, rafraichissement, deps.maintenant());
    if (!issue.ok) {
        // ⚠️ SEULE LA CLÉ D'ADRESSE EST CONSOMMÉE ICI — `contexte.cles` n'en
        // porte qu'une pour cette route (voir `clesDe`). Un jeton de
        // rafraîchissement volé ne peut donc pas servir à verrouiller le
        // compte de sa victime.
        compterLEchec(deps, contexte, '/auth/rafraichir');
        // Le motif est rendu au demandeur : il porte sur SON propre jeton, et
        // lui dire s'il doit se reconnecter ou s'il vient d'être compromis
        // n'apprend rien sur les comptes des autres.
        repondre(rep, 401, { refus: issue.motif }, cors);
        return;
    }

    // 🔴 LE JETON RENDU EST CELUI DE LA ROTATION, jamais un jeton neuf émis
    // par-dessus. Appeler `emettre` ici ouvrirait une famille NEUVE à chaque
    // rafraîchissement : la détection de rejeu révoquerait alors une famille
    // à laquelle le jeton volé n'appartient plus, et ne protégerait RIEN. Ce
    // défaut a réellement été écrit, et c'est le test du rejeu de bout en bout
    // qui l'a attrapé — les tests du dépôt seuls ne le pouvaient pas.
    await delivrer(rep, deps, issue.utilisateurId, issue.clair, cors);
}

/// La paire délivrée par les deux routes, écrite une seule fois pour qu'elles
/// ne puissent pas diverger.
async function delivrer(
    rep: ServerResponse,
    deps: DependancesAuth,
    utilisateurId: string,
    rafraichissement: string,
    cors: Record<string, string> | undefined,
): Promise<void> {
    const maintenant = deps.maintenant();
    const acces = signer(utilisateurId, deps.secretJeton, maintenant);
    repondre(
        rep,
        200,
        // `expire_a` en MILLISECONDES, comme tout horodatage de ce service —
        // voir la divergence déclarée dans `identite/jeton.ts`.
        { acces, rafraichissement, expire_a: maintenant + DUREE_JETON_ACCES_MS },
        cors,
    );
}
