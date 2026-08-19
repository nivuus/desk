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

export interface DependancesAuth {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    maintenant: () => number;
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
        rep.writeHead(204, cors ?? {});
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

    if (chemin === '/auth/connexion') {
        await connexion(corps, rep, deps, cors);
    } else {
        await rafraichir(corps, rep, deps, cors);
    }
    return true;
}

async function connexion(
    corps: Record<string, unknown>,
    rep: ServerResponse,
    deps: DependancesAuth,
    cors: Record<string, string> | undefined,
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
        repondre(rep, 401, { refus: 'identifiants' }, cors);
        return;
    }
    if (!bon) {
        repondre(rep, 401, { refus: 'identifiants' }, cors);
        return;
    }

    // Le re-hachage à la connexion suivante : c'est ce qui rendra inutile
    // toute migration de données le jour où les paramètres changeront.
    if (doitEtreRehache(utilisateur.empreinte_mdp)) {
        await remplacerEmpreinte(deps.base, utilisateur.id, await hacher(motdepasse));
    }

    // Une connexion ouvre une famille NEUVE — c'est le seul geste qui le
    // fasse.
    await delivrer(rep, deps, utilisateur.id, await emettre(deps.base, utilisateur.id, deps.maintenant()), cors);
}

async function rafraichir(
    corps: Record<string, unknown>,
    rep: ServerResponse,
    deps: DependancesAuth,
    cors: Record<string, string> | undefined,
): Promise<void> {
    const { rafraichissement } = corps;
    if (typeof rafraichissement !== 'string') {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return;
    }

    const issue = await tourner(deps.base, rafraichissement, deps.maintenant());
    if (!issue.ok) {
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
