// `GET /applications` and `POST /application/:id/lancer` — the HTTP surface of the
// catalogue.
//
// 🔴 THE CONTRACT IS THAT OF `routes-auth.ts` AND `routes-vm.ts`:
// `Promise<boolean>`, `true` = served, `false` = not my path. The generic
// 404 of `http/serveur.ts` then answers alone, and it is not
// duplicated here.
//
// ⚠️ « THEN ANSWERS ALONE » IS NO LONGER UNCONDITIONALLY TRUE SINCE 22 AUGUST 2026, and
// the sentence is left as is because it stays right in the nginx
// deployment: when `PLATEFORME_PAGE` is armed, a TENTH router — the page
// server — is chained AFTER all the others, and it resolves any
// path. On a `GET`/`HEAD`, it is IT that answers `200 text/html` to the `false`
// returned here; outside `GET`/`HEAD` it steps aside, and the generic 404 takes
// over. See `http/chaine.ts`, which carries the count and the rule. (policy: allow-fr - file name)
//
// 🔴 AUTHENTICATION GOES THROUGH `http/porteur.ts`, NEVER THROUGH A COPY.
// The G1 plan (decision D9) prescribed calling `verifyToken` then
// requiring `verdict.type === 'user'` — that is, rewriting here,
// word for word, what P4 shipped in the meantime in `lirePorteur`. Copying a
// security decision is precisely what this repository refuses: « a copy
// would drift silently » (`agents/canal.ts`), and this one would drift on the
// day one of the two hardened its reading of the header. ACCEPTED
// consequence: the refusal codes are those of `porteur.ts` — `401 jeton-absent`,
// `401 jeton-invalide`, `401 jeton-expire`, `403 jeton-agent` — and not the
// uniform `401 {refus:'jeton'}` of the plan. The `403` on an agent token is
// better argued than the `401`: the token is VALID, it simply is not
// a human one, and a 401 would invite signing in again for nothing.
//
// 🔴 A SECURITY DIVERGENCE SETTLED BY THE REPOSITORY OWNER, AND
// IT IS THIS MODULE THAT GAVE WAY. Two sub-blocks had shipped two
// contradictory answers to the same question — a request about a VM that
// does not belong to the requester — and had each flagged it without
// settling it: decision D9 of the G1 plan kept here
// `403 {refus:'vm-etrangere'}`, DISTINCT from the refusal of an unknown VM, while
// `http/routes-vm.ts` (sub-block P4) kept an INDISTINGUISHABLE
// `404 vm-inconnue`, on the model of `routes-auth.ts` and `agents/enrolement.ts`.
//
// THE REPOSITORY OWNER KEPT THE INDISTINGUISHABLE REFUSAL, and the reason is
// that the `403` was an ENUMERATION ORACLE: it CONFIRMED the existence of a
// resource to someone with precisely no right to it, so that a user
// learnt by trial and error which VMs exist, without ever seeing a
// single one. The two routes of this file therefore now return the SAME
// `404 {refus:'vm-inconnue'}`, produced by the same path, for both cases.
//
// 🔴 THE TRADE-OFF IS A LOG LINE, AND IT IS NOT
// DECORATIVE: without it, making things uniform would cost the operator all the
// diagnosis — « the VM does not exist » and « it belongs to someone else » would
// read the same on BOTH sides, and nothing would tell a typing
// mistake from an enumeration attempt any more. `acces` writes it, it names the real
// case, and ⚠️ IT MUST NEVER REACH THE HTTP RESPONSE: that is the whole
// point. A test asserts that the body carries no trace of the real case.
//
// ⚠️ THE BODY CEILING OF `routes-auth.ts` DOES NOT APPLY HERE, and must
// ABOVE ALL NOT be raised: these two routes have no meaningful body —
// one is a `GET`, the other carries only its path.

import type { IncomingMessage, ServerResponse } from 'node:http';
import { randomUUID } from 'node:crypto';
import type { RegistreAgents } from '../agents/registre';
import type { Pilote } from '../base/pilote';
import { associationsDe, lireParId, lireParVm, sourceMaxDepuis } from '../depot/application';
import { lireParId as lireVm } from '../depot/vm';
import { signerUrlIcone } from '../apps/url-icone';
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

const LIST_PATH = '/applications';

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ UNCONDITIONAL, and set on EVERY response — including the
        // ERROR responses (401, 405, 413, 429, 500, 503), which
        // often carry more information than a normal response. They are spread
        // BEFORE `cors` so that the origin policy, which is optional,
        // can never overwrite them by mistake.
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(JSON.stringify(corps));
}

/// Matches `/application/:id/lancer`, and NOTHING else.
///
/// 🔴 THE PATH IS SPLIT BY SEGMENTS, NEVER BY `startsWith` — the rule
/// that `http/serveur.ts` and `http/routes-vm.ts` already follow: a prefix
/// would open a whole family of paths nobody decided on. The pattern
/// is therefore anchored at BOTH ends: `/application/x/lancer/y` is not served,
/// and neither is `/application/x`.
function lancementDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'application', '<id>', 'lancer'] — exactement quatre.
    if (segments.length !== 4) return undefined;
    if (segments[1] !== 'application' || segments[3] !== 'lancer') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}

/// Decides whether this user has the right to see this VM.
///
/// ⚠️ THE « NOT ASSIGNED » BRANCH LOGS, AND THAT IS NOT DECORATIVE.
/// `vm.utilisateur_id` is NULL after `npm run admin:agent` — reread: (policy: allow-fr - frozen wire key or SQLite column)
/// `enrolerLaVm` does `INSERT INTO vm(id, nom, adresse)` and never passes
/// a user. As long as no VM is assigned, EVERY AUTHENTICATED
/// USER SEES ALL THE VMS: this is NOT isolation, and the log
/// line is the only thing that makes this state visible to the operator.
/// Serving silently would make it invisible, and it would stay so until
/// someone read this file.
///
/// The behaviour HARDENS BY ITSELF the day sub-block P4 fills the
/// column: the NULL branch will stop being reached, without a line changing
/// here.
///
/// 🔴 `'etrangere'` SURVIVES AS AN INTERNAL VERDICT WHILE THE WIRE REASON
/// `'vm-etrangere'` HAS GONE, and the two facts do not contradict each other.
/// The wire reason had only dead emission sites left once
/// uniformity was reached — an unreachable reason is a dead variant that
/// the next reader would believe alive, and they could have made it
/// reachable again while believing they were fixing something. It is therefore removed, and it survives
/// nowhere: it was a member of no union (`orchestration/refus.ts`
/// only ever knew `vm-inconnue`), only two literals and their two
/// test assertions, all four gone with it.
/// The VERDICT, on the other hand, is what feeds the log line: flattening it into a
/// boolean would remove the only distinction the decision deliberately
/// keeps. It is alive, tested, and it never crosses the wire.
async function acces(
    deps: DependancesApplications,
    vmId: string,
    userId: string,
): Promise<'ok' | 'inconnue' | 'etrangere'> {
    const vm = await lireVm(deps.base, vmId);
    if (vm === undefined) return journaliserLeRefus('inconnue', vmId, userId);
    if (vm.utilisateur_id === null) { // policy: allow-fr - frozen wire key or SQLite column
        console.warn(
            `unassigned vm, access granted without isolation to VM ${vmId} `
                + `for user ${userId} (assignment = sub-block P4)`,
        );
        return 'ok';
    }
    return vm.utilisateur_id === userId // policy: allow-fr - frozen wire key or SQLite column
        ? 'ok'
        : journaliserLeRefus('etrangere', vmId, userId);
}

/// The trade-off of the indistinguishable refusal: the only thing, in the whole
/// service, that says WHICH of the two cases happened.
///
/// 🔴 IT IS WRITTEN IN `acces`, AND NOT AT THE CALL SITES. The two routes
/// of this file refuse through the same path, and a third caller
/// will turn up one day; writing the line at the caller would make it
/// FORGETTABLE — and forgetting it would break nothing visible, which is
/// exactly the failure mode this repository calls a silent failure.
/// Here, the refusal and its trace are born together or not at all.
///
/// ⚠️ `cas=` IS A FIELD, NOT A SENTENCE: it is what the operator `grep`s for,
/// and a sentence gets rewritten without anything breaking. A test pins it, and it
/// ALSO pins that the two cases do not yield the same thing — without which a
/// single line saying « refusal » would satisfy a check that only looked for the
/// presence of the trace.
///
/// ⚠️ THE RETURN IS THE VERDICT ITSELF, so that the line cannot
/// drift from the decision it describes: there is no path where one
/// logs one case and returns the other.
function journaliserLeRefus(
    cas: 'inconnue' | 'etrangere',
    vmId: string,
    userId: string,
): 'inconnue' | 'etrangere' {
    console.warn(
        `access to VM ${vmId} refused for user ${userId}: `
            + `cas=${cas} — the HTTP response itself is the same 404 « vm-inconnue » `
            + `in both cases (decision of the repository owner: no enumeration `
            + `oracle).`,
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
    const isList = chemin === LIST_PATH;
    if (!isList && idApplication === undefined) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    // 🔴 THE PREFLIGHT REQUEST IS SERVED, AND WITHOUT IT NOTHING IS REACHABLE
    // from a browser: both routes require `Authorization: Bearer`,
    // which makes the request NOT SIMPLE. A 404 on the `OPTIONS` would make
    // the browser give up before even sending the real request. Same
    // choice as `routes-vm.ts` and `routes-auth.ts`.
    if (req.method === 'OPTIONS') {
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }

    // The path EXISTS, it is the method that does not fit: a 404 would send people
    // looking for a missing route.
    if (isList && req.method !== 'GET') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }
    if (idApplication !== undefined && req.method !== 'POST') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    // 🔴 AUTHENTICATION COMES BEFORE ANY DATABASE READ. A route that
    // read the catalogue and then refused the token would leak nothing through its
    // response, but it would offer free work to an anonymous peer.
    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        // ⚠️ THE CORS HEADERS ARE SET ON THE REFUSAL TOO: a 401 that the
        // browser cannot read shows up as a network failure, not
        // as an invitation to sign in again.
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }

    if (isList) {
        const vmId = requete.searchParams.get('vm');
        if (vmId === null || vmId === '') {
            // 400 and not 404: the path is right, it is the request that is
            // incomplete — and saying so avoids looking for a missing route.
            repondre(rep, 400, { refus: 'vm-absente' }, cors);
            return true;
        }
        const verdict = await acces(deps, vmId, porteur.userId);
        if (verdict !== 'ok') {
            // 🔴 THE VERDICT IS NOT REREAD HERE, and that is the core of the
            // decision: « unknown » and « foreign » go through the SAME
            // expression, so that the two bodies CANNOT differ.
            // A ternary, even one returning the same value twice, would leave
            // the door open to one of the two branches changing
            // one day. Same move as `routes-vm.ts`.
            repondre(rep, 404, { refus: 'vm-inconnue' }, cors);
            return true;
        }
        const lignes = await lireParVm(deps.base, vmId);
        // 🔴 ONE SINGLE QUERY FOR ALL THE ASSOCIATIONS. Asking for them
        //    application by application would make 156 round trips per
        //    hub display, on the corpus of the development VM.
        const associations = await associationsDe(
            deps.base,
            lignes.map((l) => l.id),
        );
        // ⚠️ NO `cible`, NO `arguments`, NO `repertoire`, NO `chemin`: these are
        // paths on the VM DISK, which the browser has no use for
        // and which describe the inside of a machine. Launching goes through
        // the id, never through a path the client would provide — that is
        // what stops a page from requesting the execution of an arbitrary
        // program. Same discipline as `routes-vm.ts`, which keeps `adresse` quiet.
        repondre(
            rep,
            200,
            {
                applications: lignes.map((l) => ({
                    id: l.id,
                    nom: l.nom,
                    // ⚠️ THESE TWO GO THROUGH, AND THE REASONING ABOVE
                    // DOES NOT OBJECT: a fingerprint is the path of
                    // nothing, and `source_max` is a property of the IMAGE. What
                    // the previous paragraph keeps quiet is paths on the
                    // VM disk; these are not.
                    //
                    // 🔴 `source_max` GOES THROUGH `sourceMaxDepuis`, WRITTEN
                    // ONCE IN THE REPOSITORY. Rebuilding it here would make the
                    // rule `null -> non-mesuree` live in two places, and the two
                    // would drift the day one decided that `null`
                    // is worth `0` — that is, would make an UNKNOWN provenance
                    // claim it is worth something.
                    icone: l.icone,
                    // 🔴 THE SIGNED ICON URL — DECISION OF THE REPOSITORY
                    // OWNER, 30 AUGUST 2026, AND NOT A CONVENIENCE. It is here, and
                    // NOWHERE ELSE, that an icon URL is minted:
                    // this route requires the bearer token (`lirePorteur`,
                    // above) and has already checked the ownership of the VM
                    // (`acces`, above). **A signed URL is obtained WITH a
                    // token, never freely**, and that chaining is what makes it
                    // legitimate — see `apps/url-icone.ts` for the derived
                    // key, what the signature covers, and the lifetime.
                    //
                    // ⚠️ `null` WHEN THERE IS NO ICON, never a URL
                    // that would return 404: the browser must not have to
                    // tell « no icon » from « icon not found ».
                    icone_url:
                        l.icone === null
                            ? null
                            : signerUrlIcone(
                                  l.id,
                                  vmId,
                                  l.icone,
                                  deps.secretJeton,
                                  deps.maintenant(),
                              ),
                    source_max: sourceMaxDepuis(l.source_max_px),
                    // ⚠️ THESE TWO GO THROUGH TOO, AND THE REASONING
                    // ABOVE DOES NOT OBJECT: a colour is the
                    // path of nothing, and a file extension is a
                    // public convention — neither one describes
                    // the inside of a machine.
                    //
                    // 🔴 `associations` IS ALWAYS AN ARRAY, NEVER MISSING.
                    // An application without an association is the most
                    // frequent case, and the browser must not have to
                    // tell « none » from « not filled in ».
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
    const verdict = await acces(deps, application.vm_id, porteur.userId);
    if (verdict !== 'ok') {
        // 🔴 WITHOUT THIS GUARD, AN APPLICATION ID WOULD BE ENOUGH TO LAUNCH
        // A PROGRAM ON SOMEONE ELSE'S MACHINE — and the agent, for its part,
        // has no way to know who asked: it runs what it is
        // told to run.
        // The refusal is that of the listing, word for word: a single reason, a
        // single code, for both routes as for both cases.
        repondre(rep, 404, { refus: 'vm-inconnue' }, cors);
        return true;
    }

    // 🔴 THE REQUEST ID IS DRAWN HERE, and it is what pairs the order with its
    // answer. Using the key in its place would mix up two concurrent
    // launches of the same application — two hub tabs are enough.
    const issue = await deps.registre.lancer(application.vm_id, application.cle, randomUUID());

    if (issue === 'agent-injoignable') {
        // 503: the service is fine, it is the VM that does not answer. Returning 200
        // would make the hub display a success for a launch that did not
        // happen — the hardest failure to diagnose there is, because
        // nothing anywhere contradicts it.
        repondre(rep, 503, { refus: 'agent-injoignable' }, cors);
        return true;
    }
    if (issue === 'delai') {
        // 504: the order HAS GONE, and nobody answered. ⚠️ It is not
        // cancelled for all that — the agent may very well have launched
        // the application and answered too late. The code says « I do not
        // know », never « it did not happen ».
        repondre(rep, 504, { refus: 'delai' }, cors);
        return true;
    }

    // 🔴 THE OUTCOME IS RETURNED AS IS, never flattened into a boolean:
    // `raccourci` versus `cible` is what says whether it is really the `.lnk` that was
    // launched or a rebuilt target, and a boolean would make the acceptance
    // criterion lose all discrimination.
    //
    // ⚠️ `echec` RETURNS 200, AND THAT IS DELIBERATE: the order went through, the platform
    // did its job, and the agent answered. A 5xx would say the SERVICE
    // failed, which is false — and would make `echec` indistinguishable
    // from `agent-injoignable`, while these are two opposite situations.
    repondre(rep, 200, { issue }, cors);
    return true;
}
