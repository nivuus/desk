// Les trois routes de l'INSTALLATION : l'ordre, l'état, et les octets servis à
// l'AGENT.
//
//   `POST /installation`               jeton PORTEUR (humain)  -> { id }
//   `GET  /installation/:id`           jeton PORTEUR (humain)  -> l'état
//   `GET  /televersement/:id/contenu`  jeton d'AGENT, LUI SEUL -> les octets
//
// 🔴 CONTRAT DE `routes-auth.ts`, `routes-vm.ts`, `routes-applications.ts` et
// `routes-icone.ts` : `Promise<boolean>`, `true` = servie, `false` = pas mon
// chemin. Le 404 générique de `http/serveur.ts` répond alors seul.
//
// ⚠️ « ALORS SEUL » N'EST PLUS VRAI SANS CONDITION DEPUIS LE 22 AOÛT 2026, et
// la phrase est laissée telle quelle parce qu'elle reste juste dans le montage
// nginx : quand `PLATEFORME_PAGE` est armée, un DIXIÈME routeur — le servant
// de page — est chaîné APRÈS tous les autres, et il résout n'importe quel
// chemin. Sur un `GET`/`HEAD`, c'est LUI qui répond `200 text/html` au `false`
// rendu ici ; hors `GET`/`HEAD` il se retire, et le 404 générique reprend la
// main. Voir `http/chaine.ts`, qui porte le compte et la règle.
//
// 🔴 LA GARDE LA PLUS IMPORTANTE DU SOUS-BLOC VIT DANS CE FICHIER :
// `GET …/contenu` COMPARE LA VM DU JETON À CELLE DE L'INSTALLATION. Sans elle,
// n'importe quelle VM enrôlée téléchargerait l'installeur de n'importe quelle
// autre — le contenu qu'un utilisateur a déposé pour SA machine et pour elle
// seule. **L'autorisation n'est pas « un agent valide », c'est « CET agent-LÀ ».**
// Le détail est à `autorisePourVm` et à `contenu`.
//
// 🔴 LES DEUX MOITIÉS DE L'IDENTITÉ SONT EMPLOYÉES, JAMAIS L'UNE POUR L'AUTRE :
// `porteur.ts` refuse un jeton d'agent, `porteur-agent.ts` un jeton humain — les
// deux sont signés par le MÊME secret, si bien qu'un seul relâchement les rend
// INTERCHANGEABLES (E5 de P3). Ce fichier consomme les deux sans en recopier une.
//
// 🔴 TOUT REFUS D'AUTORISATION EST INDISTINGUABLE D'UNE RESSOURCE INCONNUE —
// `404`, jamais `403` : distinguer serait un ORACLE D'ÉNUMÉRATION. Décision du
// propriétaire du dépôt, prise pour G1 (en-tête de `routes-applications.ts`),
// que G3 APPLIQUE sans la rouvrir. La contrepartie est une ligne de JOURNAL qui
// nomme le cas réel et **n'atteint jamais la réponse**.
//
// ⚠️ CE FICHIER NE POUSSE AUCUN ORDRE À L'AGENT : `POST /installation` ÉCRIT une
// ligne `en_attente`, que `canal-apps.ts::reemettreLesInstallations` met sur le
// fil à l'enrôlement suivant. **Une installation demandée pendant qu'un agent
// est déjà connecté attend donc sa prochaine connexion.** La refermer exigerait
// du `RegistreAgents` une méthode d'envoi qu'il n'a pas — il n'expose que
// `lancer`, qui attend une issue —, addition à un fichier que G3 n'ouvre pas.

import type { IncomingMessage, ServerResponse } from 'node:http';
import { pipeline } from 'node:stream/promises';
import { etatDe } from '../agents/fraicheur';
import type { MagasinTranches } from '../apps/magasin-tranches';
import { reemettreLesInstallations } from '../agents/canal-apps';
import type { RegistreAgents } from '../agents/registre';
import type { Pilote } from '../base/pilote';
import { lireParPrefixe } from '../depot/agent';
import { creer, lireParId as lireInstallation, type LigneInstallation } from '../depot/installation';
import { lireParId as lireTeleversement, type LigneTeleversement } from '../depot/televersement';
import { lireParId as lireVm } from '../depot/vm';
import { chaineNonVide, estObjetJson } from '../../../proto/ts/plateforme-gardes';
import { plan } from '../../../proto/ts/tranches';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';
import { lirePorteur } from './porteur';
import { lirePorteurAgent } from './porteur-agent';

export interface DependancesInstallation {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    /// 🔴 IL S'APPELLE `tranches`, PAS `magasin`, ET LE NOM EST PORTEUR.
    /// `serveur.ts` porte DÉJÀ un `magasin` — celui des ICÔNES du sous-bloc G2 —
    /// dans le même objet de dépendances. Deux routeurs qui emploieraient la
    /// même clé pour deux objets différents seraient un mauvais câblage que
    /// `tsc` n'attraperait que par chance : ici il l'a attrapé, les deux types
    /// différant, mais **le jour où deux magasins auraient la même forme, le
    /// service servirait des icônes à la place des tranches en silence.**
    tranches: MagasinTranches;
    /// 🔴 LE REGISTRE DES AGENTS, ET SANS LUI L'ORDRE N'EST JAMAIS LIVRÉ À UNE
    /// VM DÉJÀ EN LIGNE. Voir le commentaire de la poussée, plus bas.
    registre: RegistreAgents;
    maintenant: () => number;
}

/// Les en-têtes CORS d'une requête, ou `undefined` — origine non autorisée, donc
/// AUCUN en-tête, jamais `*` (`cors.ts`).
type Cors = Record<string, string> | undefined;

const CHEMIN_ORDRE = '/installation';

/// 4 Kio, pour un corps qui porte DEUX identifiants. 🔴 PLAFOND DE CETTE ROUTE
/// ET D'ELLE SEULE (D8) : celui de `routes-auth.ts` ne s'applique pas ici et
/// **ne doit surtout pas être relevé** pour arranger une route qui accepte des
/// octets. ⚠️ NON CALIBRÉ.
const CORPS_MAX_OCTETS = 4 * 1024;

/// 🔴 LA RÈGLE FAIT AUTORITÉ CÔTÉ AGENT (D15) ; CECI N'EST QU'UN REFUS PRÉCOCE,
/// qui achète un aller-retour de plusieurs centaines de mégaoctets. L'agent la
/// rejoue sur le nom reçu dans l'ordre : c'est lui qui exécute, et rien ne se
/// fie au maillon précédent. ⚠️ `.bat` est refusé — un script, dont l'interprète
/// et la politique d'exécution appellent leurs propres décisions.
export const EXTENSIONS_ACCEPTEES: readonly string[] = ['.exe', '.msi'];

/// ⚠️ INSENSIBLE À LA CASSE (Windows l'est), et le nom doit avoir un RADICAL :
/// `.exe` tout court se termine bien par `.exe`, mais c'est un nom sans corps.
export function extensionAcceptee(nom: string): boolean {
    const bas = nom.toLowerCase();
    return EXTENSIONS_ACCEPTEES.some((e) => bas.length > e.length && bas.endsWith(e));
}

function repondre(rep: ServerResponse, code: number, corps: unknown, cors: Cors): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ INCONDITIONNELS, sur TOUTE réponse — refus compris —, et étalés
        // AVANT `cors`, dont la politique est FACULTATIVE et ne doit jamais
        // pouvoir les écraser par mégarde.
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(corps === undefined ? undefined : JSON.stringify(corps));
}

/// Reconnaît `/installation/:id`, et RIEN d'autre. 🔴 DÉCOUPÉ PAR SEGMENTS,
/// JAMAIS PAR `startsWith` : G1 a MESURÉ qu'un `startsWith('/application')`
/// laissait DIX-SEPT tests verts — la route mangeait la famille et rendait SON
/// PROPRE 404 typé, indiscernable du générique. Ancré des DEUX bouts.
function installationDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'installation', '<id>'] — exactement trois.
    if (segments.length !== 3) return undefined;
    if (segments[1] !== 'installation') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}

/// Reconnaît `/televersement/:id/contenu`, et RIEN d'autre — même règle. ⚠️ LE
/// MOTIF S'ARRÊTE À `contenu` : c'est ce qui laisse la place aux autres routes
/// de la famille `/televersement/…`, qu'un `startsWith` mangerait toutes.
function contenuDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'televersement', '<id>', 'contenu'] — exactement quatre.
    if (segments.length !== 4) return undefined;
    if (segments[1] !== 'televersement' || segments[3] !== 'contenu') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}

/// Lit le corps, ou rend `undefined` si la borne est franchie — la requête est
/// alors ABANDONNÉE sans lire la suite : accumuler pour répondre poliment serait
/// le déni de service que la borne existe pour empêcher.
function lireCorps(req: IncomingMessage): Promise<string | undefined> {
    return new Promise((resoudre, rejeter) => {
        let recu = '';
        req.on('data', (morceau: Buffer) => {
            recu += morceau.toString('utf8');
            if (recu.length > CORPS_MAX_OCTETS) {
                req.destroy();
                resoudre(undefined);
            }
        });
        req.on('end', () => resoudre(recu));
        req.on('error', rejeter);
    });
}

/// La contrepartie du refus indistinguable : la seule chose, dans tout le
/// service, qui dise LEQUEL des cas s'est produit. ⚠️ `cas=` EST UN CHAMP, PAS UNE
/// PHRASE — c'est lui que l'exploitant `grep`e. Convention de G1.
function journaliser(cas: string, ressource: string, demandeur: string): void {
    console.warn(
        `refus d'acces a ${ressource} pour ${demandeur} : cas=${cas} — la reponse `
            + `HTTP, elle, est INDISTINGUABLE d'une ressource inconnue.`,
    );
}

/// Décide si cet utilisateur a le droit de voir cette VM.
///
/// ⚠️ TROISIÈME EXEMPLAIRE DE LA MÊME RÈGLE, et le dire vaut mieux que de le
/// taire : `routes-applications.ts::acces`, `routes-icone.ts::service`, et la
/// voici. Le facteur commun vivra dans un module tiers le jour où l'on rouvrira
/// ces fichiers — G3 n'en ouvre aucun, et une extraction à moitié coûterait une
/// indirection sans rien fermer. **Déclaré plutôt que subi, comme
/// `porteur-agent.ts` l'a fait pour son découpage d'en-tête : toute correction
/// se fait DANS LES TROIS FICHIERS.** ⚠️ La branche « non attribuée » JOURNALISE :
/// `vm.utilisateur_id` naît NULL, et tant qu'aucune VM n'est attribuée TOUT
/// UTILISATEUR AUTHENTIFIÉ VOIT TOUTES LES VMS.
async function acces(
    deps: DependancesInstallation, vmId: string, utilisateurId: string,
): Promise<'ok' | 'inconnue' | 'etrangere'> {
    const vm = await lireVm(deps.base, vmId);
    if (vm === undefined) {
        journaliser('inconnue', `la VM ${vmId}`, `l'utilisateur ${utilisateurId}`);
        return 'inconnue';
    }
    if (vm.utilisateur_id === null) {
        console.warn(`vm non attribuee, acces accorde sans isolation a la VM ${vmId}`);
        return 'ok';
    }
    if (vm.utilisateur_id !== utilisateurId) {
        journaliser('etrangere', `la VM ${vmId}`, `l'utilisateur ${utilisateurId}`);
        return 'etrangere';
    }
    return 'ok';
}

/// Existe-t-il, POUR CETTE VM, une installation NON TERMINÉE qui réclame ce
/// téléversement ?
///
/// 🔴 C'EST LA GARDE QUE L'EN-TÊTE ANNONCE. Le jeton dit « je suis l'agent du
/// préfixe P » ; la VM s'en déduit par `lireParPrefixe` ; et c'est CETTE VM que
/// l'on compare à `installation.vm_id`. Un agent parfaitement authentifié dont
/// aucune installation ne réclame ce téléversement n'a rien à faire des octets.
///
/// ⚠️ `etat <> 'terminee'` PLUTÔT QU'UNE LISTE — l'idiome des deux écritures de
/// `depot/installation.ts`. Les DEUX états passent : `en_attente` est le premier
/// téléchargement, `en_cours` sa REPRISE (progression rapportée, transfert
/// coupé, l'agent recommence) ; n'accepter qu'`en_attente` rendrait toute reprise
/// impossible sans qu'aucune trace ne le dise. Une installation TERMINÉE, elle,
/// n'a plus rien à télécharger.
///
/// ⚠️ CETTE REQUÊTE APPARTIENT À `depot/installation.ts`, qui ne sait lire que
/// par identifiant ou par (VM, `en_attente`) ; cette tâche n'ouvre aucun fichier
/// existant. **Le jour où il sera rouvert, elle descend d'un étage.** Aucune
/// valeur littérale — tout passe en paramètre, sans quoi `rendreMarqueurs`
/// lèverait côté Postgres.
async function autorisePourVm(base: Pilote, televersementId: string, vmId: string): Promise<boolean> {
    const lignes = await base.interroger<{ id: string }>(
        'SELECT id FROM installation WHERE televersement_id = ? AND vm_id = ? AND etat <> ?',
        [televersementId, vmId, 'terminee'],
    );
    return lignes.length > 0;
}

/// Ce qu'un humain lit d'une installation. ⚠️ `journal_tronque` DEVIENT UN BOOLÉEN
/// SUR LE FIL : la colonne est un entier parce que SQLite n'a pas de type
/// booléen, et rendre `0`/`1` obligerait le hub à connaître une convention de
/// stockage qui ne le regarde pas. ⚠️ Rien n'est omis : ce que
/// `routes-applications.ts` tait sont des CHEMINS DU DISQUE DE LA VM.
function vueDe(l: LigneInstallation): Record<string, unknown> {
    return {
        id: l.id, vm: l.vm_id, televersement: l.televersement_id,
        etat: l.etat, phase: l.phase,
        octets_faits: l.octets_faits, octets_total: l.octets_total, ecoule_ms: l.ecoule_ms,
        code_sortie: l.code_sortie, issue: l.issue, motif: l.motif,
        journal: l.journal, journal_tronque: l.journal_tronque !== 0,
        demandee_a: l.demandee_a, terminee_a: l.terminee_a, maj_a: l.maj_a,
    };
}

export async function servirInstallation(
    req: IncomingMessage, rep: ServerResponse, deps: DependancesInstallation,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    const idInstallation = installationDe(chemin);
    const idTeleversement = contenuDe(chemin);
    const estOrdre = chemin === CHEMIN_ORDRE;
    if (!estOrdre && idInstallation === undefined && idTeleversement === undefined) return false;
    const cors = entetesCors(req.headers.origin, deps.origineClient);
    // 🔴 LA REQUÊTE PRÉALABLE EST SERVIE, ET SANS ELLE RIEN N'EST ATTEIGNABLE
    // depuis un navigateur : les trois routes exigent `Authorization: Bearer`,
    // ce qui rend la requête NON SIMPLE, et un 404 sur l'`OPTIONS` ferait
    // abandonner le navigateur AVANT la vraie requête — le défaut exact que la
    // corroboration navigateur de P4 a trouvé, invisible à tout test de Node.
    if (req.method === 'OPTIONS') {
        repondre(rep, 204, undefined, cors);
        return true;
    }
    // Le chemin EXISTE, c'est la méthode qui ne convient pas : un 404 ferait
    // chercher une route absente.
    if (req.method !== (estOrdre ? 'POST' : 'GET')) {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }
    if (estOrdre) return ordre(req, rep, deps, cors);
    if (idInstallation !== undefined) return etat(req, rep, deps, cors, idInstallation);
    return contenu(req, rep, deps, cors, idTeleversement!);
}

/// `POST /installation` — l'humain demande.
async function ordre(
    req: IncomingMessage, rep: ServerResponse, deps: DependancesInstallation, cors: Cors,
): Promise<boolean> {
    // 🔴 L'AUTHENTIFICATION VIENT AVANT TOUTE LECTURE DE CORPS ET DE BASE : une
    // route qui lirait d'abord offrirait du travail gratuit à un pair anonyme.
    // ⚠️ Les en-têtes CORS sont posés sur le refus aussi — une 401 illisible par
    // le navigateur s'affiche comme une panne réseau.
    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }
    const brut = await lireCorps(req);
    if (brut === undefined) {
        repondre(rep, 413, { refus: 'taille' }, cors);
        return true;
    }
    let parse: unknown;
    try {
        parse = JSON.parse(brut);
    } catch {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return true;
    }
    if (!estObjetJson(parse) || !chaineNonVide(parse.vm) || !chaineNonVide(parse.televersement)) {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return true;
    }
    const vmId = parse.vm;
    const televersementId = parse.televersement;
    // 🔴 LES REFUS PERMANENTS PASSENT AVANT LE TRANSITOIRE (`503`) : à qui l'on
    // répond « la VM ne répond pas », on fait réessayer indéfiniment un ordre que
    // l'extension ou le scellement condamnent. 🔴 ET LE VERDICT N'EST PAS RELU :
    // les deux cas passent par la MÊME expression, donc les corps ne PEUVENT pas
    // différer.
    if ((await acces(deps, vmId, porteur.utilisateurId)) !== 'ok') {
        repondre(rep, 404, { refus: 'vm-inconnue' }, cors);
        return true;
    }
    // 🔴 INCONNU ET ÉTRANGER RENDENT LE MÊME REFUS, par la même expression : sans
    // quoi on apprendrait quels téléversements existent chez les autres.
    const tel = await lireTeleversement(deps.base, televersementId);
    if (tel === undefined || tel.utilisateur_id !== porteur.utilisateurId) {
        journaliser(
            tel === undefined ? 'inconnue' : 'etrangere',
            `le televersement ${televersementId}`, `l'utilisateur ${porteur.utilisateurId}`,
        );
        repondre(rep, 404, { refus: 'televersement-inconnu' }, cors);
        return true;
    }
    // 🔴 UN TÉLÉVERSEMENT NON SCELLÉ N'EST JAMAIS ORDONNÉ : l'agent recevrait un
    // fichier PARTIEL dont l'empreinte échouerait très loin d'ici — **un refus au
    // bon endroit vaut mieux qu'un refus au bon moment**.
    if (tel.scelle_a === null) {
        repondre(rep, 409, { refus: 'non-scelle' }, cors);
        return true;
    }
    if (!extensionAcceptee(tel.nom)) {
        repondre(rep, 400, { refus: 'extension' }, cors);
        return true;
    }
    // 🔴 UNE VM INJOIGNABLE REND 503, JAMAIS 201 : écrire la ligne sans le dire
    // ferait afficher au hub une installation « demandée » que personne ne
    // recevra — la panne la plus difficile à diagnostiquer qui soit, rien nulle
    // part ne la contredisant. Même code que `routes-applications.ts` sur le
    // lancement. ⚠️ La fraîcheur est celle d'`agents/fraicheur.ts`, PURE, et son
    // horloge celle de `deps` : la transition en devient assiégeable à la
    // milliseconde par un test.
    const vm = await lireVm(deps.base, vmId);
    if (vm === undefined || etatDe(vm.vu_a, deps.maintenant()) !== 'prete') {
        repondre(rep, 503, { refus: 'agent-injoignable' }, cors);
        return true;
    }
    const ligne = await creer(deps.base, { vmId, televersementId }, deps.maintenant());

    // 🔴 L'ORDRE EST POUSSÉ ICI, ET CETTE POUSSÉE MANQUAIT — LA RECETTE L'A
    // TROUVÉ, PAS LA RELECTURE. Ce fichier documentait que la ligne
    // `en_attente` était « mise sur le canal par
    // `canal-apps.ts::reemettreLesInstallations` », et c'est vrai : mais cette
    // fonction n'est appelée QU'À L'ENRÔLEMENT. Un agent DÉJÀ connecté ne se
    // réenrôle jamais, si bien qu'une installation demandée pendant que la VM
    // est en ligne — c'est-à-dire LE CAS NOMINAL, le seul que 503 laisse
    // passer — n'était livrée qu'au prochain redémarrage de l'agent.
    //
    // Mesuré sur la chaîne réelle : `POST /installation` rendait bien 201,
    // `GET /installation/:id` restait `en_attente` avec `phase: ""` et
    // `octets_faits: 0`, et le journal de l'agent ne portait **aucune ligne**
    // d'installation. Rien, nulle part, ne contredisait le 201.
    //
    // ⚠️ ON RÉEMPLOIE `reemettreLesInstallations`, ON NE RECONSTRUIT PAS LE
    // MESSAGE. Elle relit les `en_attente` de cette VM et les encode ; une
    // seconde construction d'`Installer` ici aurait divergé de celle du canal
    // le jour où l'une des deux aurait changé — et c'est la duplication que le
    // champ mort `socket` de ses dépendances rendait jusqu'ici obligatoire.
    //
    // ⚠️ `void … .catch(…)`, JAMAIS `await` : la ligne EST en base, le 201 est
    // dû, et une base momentanément lente ne doit pas le retarder. Si la
    // poussée échoue ou n'aboutit pas, le filet d'enrôlement reste — c'est
    // exactement ce qu'il est là pour faire.
    void reemettreLesInstallations({
        base: deps.base,
        vmId,
        envoyer: (brut) => {
            if (!deps.registre.pousser(vmId, brut)) {
                console.info(
                    `installation ${ligne.id} : aucun socket ouvert pour la VM ${vmId}, `
                    + "l'ordre attend le prochain enrôlement",
                );
            }
        },
    }).catch((cause) => {
        console.error(`installation ${ligne.id} : poussée impossible — ${String(cause)}`);
    });

    // 201 : une ressource est NÉE, et son identifiant est ce que le hub ira
    // relire par `GET /installation/:id`.
    repondre(rep, 201, { id: ligne.id }, cors);
    return true;
}

/// `GET /installation/:id` — l'humain suit.
async function etat(
    req: IncomingMessage, rep: ServerResponse, deps: DependancesInstallation, cors: Cors, id: string,
): Promise<boolean> {
    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }
    const ligne = await lireInstallation(deps.base, id);
    // 🔴 LE REFUS D'UNE INSTALLATION ÉTRANGÈRE EST CELUI D'UNE INSTALLATION
    // INCONNUE, ET NON `vm-inconnue` : rendre ici le motif de la VM DIRAIT que
    // l'installation existe, et rouvrirait l'oracle par la porte de derrière, sur
    // la ressource même que l'URL nomme. MÊME expression pour les deux cas ; la
    // ligne de journal, elle, les distingue.
    if (ligne === undefined || (await acces(deps, ligne.vm_id, porteur.utilisateurId)) !== 'ok') {
        repondre(rep, 404, { refus: 'installation-inconnue' }, cors);
        return true;
    }
    repondre(rep, 200, vueDe(ligne), cors);
    return true;
}

/// `GET /televersement/:id/contenu` — l'AGENT tire les octets.
async function contenu(
    req: IncomingMessage, rep: ServerResponse, deps: DependancesInstallation, cors: Cors, id: string,
): Promise<boolean> {
    // 🔴 UN JETON D'AGENT, ET LUI SEUL. `lirePorteur` refuserait cet appel par
    // `403 jeton-agent` : c'est la moitié SYMÉTRIQUE qu'il faut ici, celle qui
    // exige `type === 'agent'`. Accepter un jeton humain rendrait les deux
    // identités interchangeables sur cette route.
    const porteur = lirePorteurAgent(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }
    // Le SUJET d'un jeton d'agent est le PRÉFIXE DE SESSION, jamais l'identifiant
    // de VM : `agents/canal.ts` signe le préfixe, et la VM se résout ici.
    const enrole = await lireParPrefixe(deps.base, porteur.prefixe);
    const tel = await lireTeleversement(deps.base, id);
    // 🔴 C'EST ICI QUE LA VM DU JETON EST COMPARÉE À CELLE DE L'INSTALLATION : un
    // agent valide n'est pas un agent autorisé. Les trois cas — préfixe sans
    // enrôlement, téléversement inconnu, aucune installation vivante pour CETTE
    // VM — rendent le MÊME refus par la MÊME expression.
    // 🔴 ET L'AUTORISATION PASSE AVANT TOUT ÉTAT, L'ORDRE EST PORTANT : répondre
    // `409 non-scelle` à un agent sans droit APPRENDRAIT que ce téléversement
    // existe, et le refus d'état deviendrait l'oracle que le refus d'accès existe
    // pour fermer.
    const autorise = enrole !== undefined && tel !== undefined
        && (await autorisePourVm(deps.base, tel.id, enrole.vm_id));
    if (!autorise) {
        const cas = enrole === undefined ? 'prefixe-sans-enrolement'
            : tel === undefined ? 'inconnue' : 'etrangere';
        journaliser(cas, `le contenu du televersement ${id}`, `l'agent ${porteur.prefixe}`);
        repondre(rep, 404, { refus: 'televersement-inconnu' }, cors);
        return true;
    }
    // 🔴 UN TÉLÉVERSEMENT NON SCELLÉ N'EST JAMAIS SERVI : les tranches sont sur le
    // disque, mais rien n'a vérifié qu'elles y sont TOUTES ni que leur
    // concaténation a la bonne empreinte. L'agent recevrait un fichier partiel et
    // ne l'apprendrait qu'après l'avoir écrit en entier.
    if (tel!.scelle_a === null) {
        repondre(rep, 409, { refus: 'non-scelle' }, cors);
        return true;
    }
    return servirLesOctets(rep, deps, tel!, cors);
}

/// La concaténation des tranches, EN FLUX.
///
/// 🔴 RIEN N'EST ASSEMBLÉ NI ACCUMULÉ (D7) : un installeur de 800 Mo tiendrait
/// 1,6 Go sur le disque le temps d'un assemblage, et toute la mémoire du service
/// s'il passait par un tampon. ⚠️ LE PLAN DES RANGS EST CALCULÉ, PAS LISTÉ, et
/// c'est ce qui donne son sens au flux : `concatener` sert le plan qu'on lui
/// donne, si bien qu'une tranche disparue devient une ERREUR de flux au lieu d'un
/// flux plus court terminé proprement — que l'agent empreindrait sans savoir
/// pourquoi c'est faux. ⚠️ L'IDENTIFIANT PASSÉ AU MAGASIN VIENT DE LA BASE
/// (`tel.id`), JAMAIS DE L'URL : il devient un nom de RÉPERTOIRE, où `..` est
/// significatif.
async function servirLesOctets(
    rep: ServerResponse, deps: DependancesInstallation, tel: LigneTeleversement, cors: Cors,
): Promise<boolean> {
    const rangs = plan(tel.taille, tel.taille_tranche).map((t) => t.n);
    const flux = deps.tranches.concatener(tel.id, rangs);
    rep.writeHead(200, {
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
        // ⚠️ `application/octet-stream` ET RIEN D'AUTRE : ces octets sont un
        // exécutable, et laisser un intermédiaire deviner leur type est
        // exactement ce que le `nosniff` d'`ENTETES_SECURITE` interdit.
        'content-type': 'application/octet-stream',
        // 🔴 LA LONGUEUR EST CELLE DU CONTRAT, vérifiée au scellement contre la
        // somme des tranches : c'est elle qui rend une réponse TRONQUÉE
        // détectable au lieu de la laisser passer pour un fichier complet.
        'content-length': String(tel.taille),
        // ⚠️ AUCUN `Content-Disposition`, AUCUN NOM DE FICHIER : l'agent connaît
        // le nom, reçu dans l'ordre `installer` à côté de l'empreinte ; le
        // répéter en ferait une seconde source que rien ne comparerait.
    });
    try {
        await pipeline(flux, rep);
    } catch (cause) {
        // 🔴 ON DÉTRUIT LA RÉPONSE, ON NE LA TERMINE PAS : les en-têtes sont déjà
        // partis, et `rep.end()` rendrait un corps plus court que le
        // `Content-Length` annoncé — ce qu'un client verrait comme une réponse
        // close. La détruire coupe la connexion, et l'agent le lit comme le
        // transfert manqué que c'est.
        console.error(
            `contenu du televersement ${tel.id} interrompu : ${String(cause)} — une `
                + `tranche manque, ou le client a raccroche`,
        );
        rep.destroy();
    }
    return true;
}
