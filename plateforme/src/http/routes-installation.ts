// The three INSTALLATION routes: the order, the state, and the bytes served to
// the AGENT.
//
//   `POST /installation`               BEARER token (human)    -> { id }
//   `GET  /installation/:id`           BEARER token (human)    -> the state
//   `GET  /televersement/:id/contenu`  AGENT token, IT ALONE   -> the bytes
//
// 🔴 CONTRACT OF `routes-auth.ts`, `routes-vm.ts`, `routes-applications.ts` and
// `routes-icone.ts`: `Promise<boolean>`, `true` = served, `false` = not my
// path. The generic 404 of `http/serveur.ts` then answers alone.
//
// ⚠️ « THEN ANSWERS ALONE » IS NO LONGER UNCONDITIONALLY TRUE SINCE 22 AUGUST 2026, and
// the sentence is left as is because it stays right in the nginx
// deployment: when `PLATEFORME_PAGE` is armed, a TENTH router — the page
// server — is chained AFTER all the others, and it resolves any
// path. On a `GET`/`HEAD`, it is IT that answers `200 text/html` to the `false`
// returned here; outside `GET`/`HEAD` it steps aside, and the generic 404 takes
// over. See `http/chaine.ts`, which carries the count and the rule.
//
// 🔴 THE MOST IMPORTANT GUARD OF THE SUB-BLOCK LIVES IN THIS FILE:
// `GET …/contenu` COMPARES THE VM OF THE TOKEN WITH THAT OF THE INSTALLATION. Without it,
// any enrolled VM would download the installer of any
// other — the content a user uploaded for THEIR machine and for it
// alone. **The authorisation is not « a valid agent », it is « THIS PARTICULAR agent ».**
// The detail is at `autorisePourVm` and at `contenu`.
//
// 🔴 BOTH HALVES OF THE IDENTITY ARE USED, NEVER ONE FOR THE OTHER:
// `porteur.ts` refuses an agent token, `porteur-agent.ts` a human token — the
// two are signed by the SAME secret, so that a single loosening makes them
// INTERCHANGEABLE (E5 of P3). This file consumes both without copying either.
//
// 🔴 ANY AUTHORISATION REFUSAL IS INDISTINGUISHABLE FROM AN UNKNOWN RESOURCE —
// `404`, never `403`: telling them apart would be an ENUMERATION ORACLE. Decision of the
// repository owner, taken for G1 (header of `routes-applications.ts`),
// which G3 APPLIES without reopening it. The trade-off is a LOG line that
// names the real case and **never reaches the response**.
//
// ⚠️ THIS FILE PUSHES NO ORDER TO THE AGENT: `POST /installation` WRITES an
// `en_attente` row, which `canal-apps.ts::reemettreLesInstallations` puts on the
// wire at the next enrolment. **An installation requested while an agent
// is already connected therefore waits for its next connection.** Closing that would require
// from `RegistreAgents` a send method it does not have — it only exposes
// `lancer`, which waits for an outcome —, an addition to a file G3 does not open.

import type { IncomingMessage, ServerResponse } from 'node:http';
import { CHEMIN_ORDRE, contenuDe, installationDe } from './installation-chemins';
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
    /// 🔴 IT IS CALLED `tranches`, NOT `magasin`, AND THE NAME MATTERS.
    /// `serveur.ts` ALREADY carries a `magasin` — the ICON one of sub-block G2 —
    /// in the same dependency object. Two routers that used the
    /// same key for two different objects would be a miswiring that
    /// `tsc` would only catch by luck: here it caught it, the two types
    /// differing, but **the day two stores had the same shape, the
    /// service would silently serve icons instead of the slices.**
    tranches: MagasinTranches;
    /// 🔴 THE AGENT REGISTRY, AND WITHOUT IT THE ORDER IS NEVER DELIVERED TO A
    /// VM ALREADY ONLINE. See the comment on the push, further down.
    registre: RegistreAgents;
    maintenant: () => number;
}

/// The CORS headers of a request, or `undefined` — origin not allowed, hence
/// NO header, never `*` (`cors.ts`).
type Cors = Record<string, string> | undefined;

/// 4 KiB, for a body that carries TWO ids. 🔴 CEILING OF THIS ROUTE
/// AND OF IT ALONE (D8): the one of `routes-auth.ts` does not apply here and
/// **must above all not be raised** to suit a route that accepts
/// bytes. ⚠️ NOT CALIBRATED.
const CORPS_MAX_OCTETS = 4 * 1024;

/// 🔴 THE RULE IS AUTHORITATIVE ON THE AGENT SIDE (D15); THIS IS ONLY AN EARLY REFUSAL,
/// which saves a round trip of several hundred megabytes. The agent
/// replays it on the name received in the order: it is the one running things, and nothing
/// trusts the previous link. ⚠️ `.bat` is refused — a script, whose interpreter
/// and execution policy call for their own decisions.
export const EXTENSIONS_ACCEPTEES: readonly string[] = ['.exe', '.msi'];

/// ⚠️ CASE-INSENSITIVE (Windows is), and the name must have a STEM:
/// `.exe` on its own does end with `.exe`, but it is a name with no body.
export function extensionAcceptee(nom: string): boolean {
    const bas = nom.toLowerCase();
    return EXTENSIONS_ACCEPTEES.some((e) => bas.length > e.length && bas.endsWith(e));
}

function repondre(rep: ServerResponse, code: number, corps: unknown, cors: Cors): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ UNCONDITIONAL, on EVERY response — refusals included —, and spread
        // BEFORE `cors`, whose policy is OPTIONAL and must never
        // be able to overwrite them by mistake.
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(corps === undefined ? undefined : JSON.stringify(corps));
}

/// Reads the body, or yields `undefined` if the bound is crossed — the request is
/// then ABANDONED without reading the rest: piling up to answer politely would be
/// the denial of service the bound exists to prevent.
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

/// The trade-off of the indistinguishable refusal: the only thing, in the whole
/// service, that says WHICH of the cases happened. ⚠️ `cas=` IS A FIELD, NOT A
/// SENTENCE — it is what the operator `grep`s for. Convention of G1.
function journaliser(cas: string, ressource: string, demandeur: string): void {
    console.warn(
        `refus d'acces a ${ressource} pour ${demandeur} : cas=${cas} — la reponse `
            + `HTTP, elle, est INDISTINGUABLE d'une ressource inconnue.`,
    );
}

/// Decides whether this user has the right to see this VM.
///
/// ⚠️ THIRD COPY OF THE SAME RULE, and saying so beats keeping it
/// quiet: `routes-applications.ts::acces`, `routes-icone.ts::service`, and
/// here it is. The common factor will live in a third module the day these
/// files are reopened — G3 opens none of them, and a half extraction would cost an
/// indirection without closing anything. **Declared rather than endured, as
/// `porteur-agent.ts` did for its header splitting: any fix
/// is made IN THE THREE FILES.** ⚠️ The « not assigned » branch LOGS:
/// `vm.utilisateur_id` is born NULL, and as long as no VM is assigned EVERY
/// AUTHENTICATED USER SEES ALL THE VMS.
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

/// Is there, FOR THIS VM, an UNFINISHED installation that calls for this
/// upload?
///
/// 🔴 THIS IS THE GUARD THE HEADER ANNOUNCES. The token says « I am the agent of
/// prefix P »; the VM follows from it through `lireParPrefixe`; and it is THIS VM that
/// is compared with `installation.vm_id`. A perfectly authenticated agent that
/// no installation calls this upload for has no business with the bytes.
///
/// ⚠️ `etat <> 'terminee'` RATHER THAN A LIST — the idiom of the two writes of
/// `depot/installation.ts`. BOTH states pass: `en_attente` is the first
/// download, `en_cours` its RESUMPTION (progress reported, transfer
/// cut, the agent starts over); accepting only `en_attente` would make any resumption
/// impossible without any trace saying so. A FINISHED installation, for its part,
/// has nothing left to download.
///
/// ⚠️ THIS QUERY BELONGS TO `depot/installation.ts`, which can only read
/// by id or by (VM, `en_attente`); this task opens no existing
/// file. **The day it is reopened, it goes down one floor.** No
/// literal value — everything goes as a parameter, otherwise `rendreMarqueurs`
/// would throw on the Postgres side.
async function autorisePourVm(base: Pilote, televersementId: string, vmId: string): Promise<boolean> {
    const lignes = await base.interroger<{ id: string }>(
        'SELECT id FROM installation WHERE televersement_id = ? AND vm_id = ? AND etat <> ?',
        [televersementId, vmId, 'terminee'],
    );
    return lignes.length > 0;
}

/// What a human reads of an installation. ⚠️ `journal_tronque` BECOMES A BOOLEAN
/// ON THE WIRE: the column is an integer because SQLite has no boolean
/// type, and returning `0`/`1` would force the hub to know a storage
/// convention that is none of its concern. ⚠️ Nothing is left out: what
/// `routes-applications.ts` keeps quiet is PATHS ON THE VM DISK.
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
    // 🔴 THE PREFLIGHT REQUEST IS SERVED, AND WITHOUT IT NOTHING IS REACHABLE
    // from a browser: the three routes require `Authorization: Bearer`,
    // which makes the request NOT SIMPLE, and a 404 on the `OPTIONS` would make
    // the browser give up BEFORE the real request — the exact defect that the
    // browser corroboration of P4 found, invisible to any Node test.
    if (req.method === 'OPTIONS') {
        repondre(rep, 204, undefined, cors);
        return true;
    }
    // The path EXISTS, it is the method that does not fit: a 404 would send people
    // looking for a missing route.
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
    // 🔴 AUTHENTICATION COMES BEFORE ANY BODY OR DATABASE READ: a
    // route that read first would offer free work to an anonymous peer.
    // ⚠️ The CORS headers are set on the refusal too — a 401 unreadable by
    // the browser shows up as a network failure.
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
    // 🔴 PERMANENT REFUSALS COME BEFORE THE TRANSIENT ONE (`503`): whoever is
    // told « the VM does not answer » will retry indefinitely an order that
    // the extension or the sealing dooms. 🔴 AND THE VERDICT IS NOT REREAD:
    // both cases go through the SAME expression, so the bodies CANNOT
    // differ.
    if ((await acces(deps, vmId, porteur.utilisateurId)) !== 'ok') {
        repondre(rep, 404, { refus: 'vm-inconnue' }, cors);
        return true;
    }
    // 🔴 UNKNOWN AND FOREIGN RETURN THE SAME REFUSAL, through the same expression: otherwise
    // one would learn which uploads exist at other people's.
    const tel = await lireTeleversement(deps.base, televersementId);
    if (tel === undefined || tel.utilisateur_id !== porteur.utilisateurId) {
        journaliser(
            tel === undefined ? 'inconnue' : 'etrangere',
            `le televersement ${televersementId}`, `l'utilisateur ${porteur.utilisateurId}`,
        );
        repondre(rep, 404, { refus: 'televersement-inconnu' }, cors);
        return true;
    }
    // 🔴 AN UNSEALED UPLOAD IS NEVER ORDERED: the agent would receive a
    // PARTIAL file whose fingerprint would fail very far from here — **a refusal in the
    // right place beats a refusal at the right time**.
    if (tel.scelle_a === null) {
        repondre(rep, 409, { refus: 'non-scelle' }, cors);
        return true;
    }
    if (!extensionAcceptee(tel.nom)) {
        repondre(rep, 400, { refus: 'extension' }, cors);
        return true;
    }
    // 🔴 AN UNREACHABLE VM RETURNS 503, NEVER 201: writing the row without saying so
    // would make the hub display a « requested » installation that nobody will
    // receive — the hardest failure to diagnose there is, nothing anywhere
    // contradicting it. Same code as `routes-applications.ts` on the
    // launch. ⚠️ The freshness is that of `agents/fraicheur.ts`, PURE, and its
    // clock that of `deps`: the transition thereby becomes pinnable to the
    // millisecond by a test.
    const vm = await lireVm(deps.base, vmId);
    if (vm === undefined || etatDe(vm.vu_a, deps.maintenant()) !== 'prete') {
        repondre(rep, 503, { refus: 'agent-injoignable' }, cors);
        return true;
    }
    const ligne = await creer(deps.base, { vmId, televersementId }, deps.maintenant());

    // 🔴 THE ORDER IS PUSHED HERE, AND THAT PUSH WAS MISSING — ACCEPTANCE TESTING
    // FOUND IT, NOT REREADING. This file documented that the
    // `en_attente` row was « put on the channel by
    // `canal-apps.ts::reemettreLesInstallations` », and that is true: but that
    // function is only called AT ENROLMENT. An agent ALREADY connected never
    // enrols again, so that an installation requested while the VM
    // is online — that is, THE NOMINAL CASE, the only one 503 lets
    // through — was only delivered at the next restart of the agent.
    //
    // Measured on the real chain: `POST /installation` did return 201,
    // `GET /installation/:id` stayed `en_attente` with `phase: ""` and
    // `octets_faits: 0`, and the agent log carried **no installation
    // line**. Nothing, anywhere, contradicted the 201.
    //
    // ⚠️ WE REUSE `reemettreLesInstallations`, WE DO NOT REBUILD THE
    // MESSAGE. It rereads the `en_attente` rows of this VM and encodes them; a
    // second construction of `Installer` here would have drifted from the channel one
    // the day one of the two changed — and that is the duplication that the
    // dead `socket` field of its dependencies made mandatory until now.
    //
    // ⚠️ `void … .catch(…)`, NEVER `await`: the row IS in the database, the 201 is
    // due, and a momentarily slow database must not delay it. If the
    // push fails or does not complete, the enrolment safety net remains — that is
    // exactly what it is there for.
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

    // 201: a resource was BORN, and its id is what the hub will
    // read again through `GET /installation/:id`.
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
    // 🔴 THE REFUSAL OF A FOREIGN INSTALLATION IS THAT OF AN UNKNOWN
    // INSTALLATION, AND NOT `vm-inconnue`: returning here the VM reason WOULD SAY that
    // the installation exists, and would reopen the oracle through the back door, on
    // the very resource the URL names. SAME expression for both cases; the
    // log line, for its part, tells them apart.
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
    // 🔴 AN AGENT TOKEN, AND IT ALONE. `lirePorteur` would refuse this call with
    // `403 jeton-agent`: it is the MIRROR half that is needed here, the one that
    // requires `type === 'agent'`. Accepting a human token would make the two
    // identities interchangeable on this route.
    const porteur = lirePorteurAgent(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }
    // The SUBJECT of an agent token is the SESSION PREFIX, never the VM
    // id: `agents/canal.ts` signs the prefix, and the VM is resolved here.
    const enrole = await lireParPrefixe(deps.base, porteur.prefixe);
    const tel = await lireTeleversement(deps.base, id);
    // 🔴 IT IS HERE THAT THE VM OF THE TOKEN IS COMPARED WITH THAT OF THE INSTALLATION: a
    // valid agent is not an authorised agent. The three cases — prefix without
    // enrolment, unknown upload, no live installation for THIS
    // VM — return the SAME refusal through the SAME expression.
    // 🔴 AND AUTHORISATION COMES BEFORE ANY STATE, THE ORDER IS LOAD-BEARING: answering
    // `409 non-scelle` to an agent without rights WOULD TEACH it that this upload
    // exists, and the state refusal would become the oracle that the access refusal exists
    // to close.
    const autorise = enrole !== undefined && tel !== undefined
        && (await autorisePourVm(deps.base, tel.id, enrole.vm_id));
    if (!autorise) {
        const cas = enrole === undefined ? 'prefixe-sans-enrolement'
            : tel === undefined ? 'inconnue' : 'etrangere';
        journaliser(cas, `le contenu du televersement ${id}`, `l'agent ${porteur.prefixe}`);
        repondre(rep, 404, { refus: 'televersement-inconnu' }, cors);
        return true;
    }
    // 🔴 AN UNSEALED UPLOAD IS NEVER SERVED: the slices are on the
    // disk, but nothing has checked that ALL of them are there nor that their
    // concatenation has the right fingerprint. The agent would receive a partial file and
    // would only learn it after having written it in full.
    if (tel!.scelle_a === null) {
        repondre(rep, 409, { refus: 'non-scelle' }, cors);
        return true;
    }
    return servirLesOctets(rep, deps, tel!, cors);
}

/// The concatenation of the slices, AS A STREAM.
///
/// 🔴 NOTHING IS ASSEMBLED OR PILED UP (D7): an 800 MB installer would take
/// 1.6 GB on the disk for the time of an assembly, and all the memory of the service
/// if it went through a buffer. ⚠️ THE PLAN OF THE RANKS IS COMPUTED, NOT LISTED, and
/// that is what gives the stream its meaning: `concatener` serves the plan it is
/// given, so that a vanished slice becomes a stream ERROR instead of a
/// shorter stream cleanly finished — which the agent would fingerprint without knowing
/// why it is wrong. ⚠️ THE ID PASSED TO THE STORE COMES FROM THE DATABASE
/// (`tel.id`), NEVER FROM THE URL: it becomes a DIRECTORY name, where `..` is
/// meaningful.
async function servirLesOctets(
    rep: ServerResponse, deps: DependancesInstallation, tel: LigneTeleversement, cors: Cors,
): Promise<boolean> {
    const rangs = plan(tel.taille, tel.taille_tranche).map((t) => t.n);
    const flux = deps.tranches.concatener(tel.id, rangs);
    rep.writeHead(200, {
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
        // ⚠️ `application/octet-stream` AND NOTHING ELSE: these bytes are an
        // executable, and letting an intermediary guess their type is
        // exactly what the `nosniff` of `ENTETES_SECURITE` forbids.
        'content-type': 'application/octet-stream',
        // 🔴 THE LENGTH IS THAT OF THE CONTRACT, checked at sealing against the
        // sum of the slices: it is what makes a TRUNCATED response
        // detectable instead of letting it pass for a complete file.
        'content-length': String(tel.taille),
        // ⚠️ NO `Content-Disposition`, NO FILE NAME: the agent knows
        // the name, received in the `installer` order next to the fingerprint;
        // repeating it would make a second source that nothing would compare.
    });
    try {
        await pipeline(flux, rep);
    } catch (cause) {
        // 🔴 WE DESTROY THE RESPONSE, WE DO NOT END IT: the headers have already
        // gone, and `rep.end()` would yield a body shorter than the
        // announced `Content-Length` — which a client would see as a closed
        // response. Destroying it cuts the connection, and the agent reads it as the
        // failed transfer it is.
        console.error(
            `contenu du televersement ${tel.id} interrompu : ${String(cause)} — une `
                + `tranche manque, ou le client a raccroche`,
        );
        rep.destroy();
    }
    return true;
}
