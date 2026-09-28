// `GET /vm` and `POST /vm/:id/:operation` — the HTTP surface of the inventory.
//
// 🔴 THE CONTRACT IS THAT OF `routes-auth.ts`: `Promise<boolean>`, `true` =
// served, `false` = not my path. The generic 404 of `http/serveur.ts` is
// then alone in answering, and it is not duplicated here.
//
// ⚠️ "THEN ALONE" HAS NOT BEEN UNCONDITIONALLY TRUE SINCE 22 AUGUST 2026, and
// the sentence is left as is because it stays right in the nginx
// setup: when `PLATEFORME_PAGE` is armed, a TENTH router — the page
// server — is chained AFTER all the others, and it resolves any
// path. On a `GET`/`HEAD`, IT is what answers `200 text/html` to the `false`
// returned here; outside `GET`/`HEAD` it steps aside, and the generic 404 takes
// over again. See `http/chaine.ts`, which holds the count and the rule.
//
// 🔴 `attribuer` IS NOT EXPOSED, and it is not an oversight. There is NO
// administration role in this service: `identite/jeton.ts` only knows
// `user` and `agent`, and `config.ts` has no administrator
// variable. An assignment route would therefore be, at best, open to
// any authenticated user — a privilege escalation on a plate.
// Assignment goes through `npm run admin:attribuer` (D8). The allow list
// `OPERATIONS_HTTP` is IMPORTED, never copied, and a test asserts
// by name that `attribuer` is not in it.
//
// 🔴 `vm-inconnue` COVERS TWO CASES — the VM does not exist, OR it belongs to
// someone else — and the body is the SAME, character for character.
// Telling the two apart would make an ENUMERATION ORACLE: a user
// would learn which VMs exist by reading the status code. Third
// application of the rule after `routes-auth.ts` and `agents/enrolement.ts`.
//
// 🔴 THE DIVERGENCE WITH SUB-BLOCK G1 IS SETTLED, BY THE REPOSITORY
// OWNER, IN FAVOUR OF THIS FILE — and this module therefore did not change by a single
// line. Its decision D9 kept `403 {refus:'vm-etrangere'}` on a VM
// belonging to someone else, that is an ENUMERATION ORACLE distinct from the 404
// of an unknown VM; `http/routes-applications.ts` aligned on the
// indistinguishable refusal above, and the `vm-etrangere` reason no longer exists anywhere
// in the service.
//
// ⚠️ THE COUNTERPART LIVES OVER THERE, NOT HERE, and it is a deliberate asymmetry:
// `routes-applications.ts` writes a log line that names the real case,
// so the operator keeps the diagnosis that the HTTP response denies them.
// THIS FILE HAS NONE, and not by oversight — it CANNOT tell the two
// cases apart: its lookup is done in `siennes`, where someone else's VM is missing
// exactly like a nonexistent VM. There is no verdict here to
// log, and making one up would require a second database read
// whose only use would be the trace.

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
    /// 🔴 THE "ANY REQUEST" BRAKE, SHARED with `routes-session.ts` AND
    /// `signaling/relais.ts` — see `securite/frein.ts::BUDGET_REQUETES`. Neither
    /// `GET /vm` nor `POST /session` has any notion of failure: their abuse is
    /// a VOLUME, never a run of failed attempts, and it is this budget
    /// that bounds it — never `BUDGET_COMPTE` nor `BUDGET_ADRESSE`, which
    /// count authentication FAILURES and therefore have nothing to do here.
    frein: Frein;
    /// The proxies whose `X-Forwarded-For` header we believe — same set
    /// as `routes-auth.ts`, never a second one: see `http/adresse-source.ts`.
    proxyDeConfiance: ReadonlySet<string>;
}

const LIST_PATH = '/vm';

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ UNCONDITIONAL, and set on EVERY response — including the
        // ERROR responses (401, 405, 413, 429, 500, 503), which often carry
        // more information than a normal response. They are spread
        // BEFORE `cors` so that the origin policy, which is optional,
        // can never overwrite them by mistake.
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(JSON.stringify(corps));
}

/// Recognises `/vm/:id/:operation`, and NOTHING else.
///
/// 🔴 THE PATH IS SPLIT BY SEGMENTS, NEVER BY `startsWith` — the rule
/// that `http/serveur.ts` already follows for routing upgrades: a
/// prefix would open a whole family of paths that nobody decided on.
///
/// 🔴 AND THE OPERATION IS FILTERED HERE, BEFORE EVERYTHING ELSE. A verb missing from
/// the allow list does NOT produce a refusal: it produces `undefined`, the route
/// returns `false`, and the generic 404 applies. A 501 on a made-up verb
/// would claim that the operation exists and is not supported, which is false.
/// ⚠️ The page server does not override it HERE, and for a precise reason
/// rather than by luck: these paths only arrive via `POST`, and the
/// page server steps aside outside `GET`/`HEAD`. See `http/chaine.ts`.
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

/// Records the request on the "any request" budget, and logs IF AND
/// ONLY IF the brake has just bitten — same rule and same reason as
/// `routes-auth.ts::compterLEchec`: the next request will be refused at the very
/// top of `servirVm`, before ever calling this function again.
///
/// ⚠️ **THIS LINE LOGS ON THE TRANSITION, AND NOT ON EVERY ADMITTED
/// REQUEST — that is what sets it apart from a per-packet trace.** A line on
/// EVERY request, even after the brake has started refusing, would make
/// the service write at a rate the attacker controls at no further
/// cost — the rule of the TURN work (`CLAUDE.md`): "count or
/// sample, never trace per packet". Logging on the transition
/// closes that: a hammered address writes ONE line, never one per request.
function compterLaRequete(
    frein: Frein,
    cles: readonly (readonly [string, Budget])[],
    adresse: string,
    instant: number,
    // 🔴 MINOR FIXED (correction round 1): this parameter was missing, and
    // the line UNCONDITIONALLY logged `LIST_PATH` (`/vm`),
    // including for `POST /vm/:id/:operation` — the trace named the
    // wrong route.
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
    const isList = chemin === LIST_PATH;
    if (!isList && action === undefined) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    // 🔴 THE PREFLIGHT REQUEST IS SERVED, AND WITHOUT IT NOTHING IS REACHABLE.
    // Both routes require `Authorization: Bearer`, which makes the request
    // NON SIMPLE: the browser first sends an `OPTIONS`, and a 404 would
    // make it give up without ever sending the real request. ⚠️ The P4 plan
    // did not prescribe it; it is a defect spotted, not copied — twin of
    // the `Access-Control-Allow-Headers` one (see `cors.ts`).
    //
    // 204 even without a CORS header: the preflight is served, but without
    // permission the browser will refuse the real request — a NOISY refusal,
    // which the operator sees. Same choice as `routes-auth.ts`.
    if (req.method === 'OPTIONS') {
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }

    if (isList && req.method !== 'GET') {
        // The path EXISTS, it is the method that does not fit: a 404
        // would send people looking for a missing route.
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }
    if (action !== undefined && req.method !== 'POST') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    // 🔴 THE "ANY REQUEST" BRAKE IS CONSULTED HERE — BEFORE `lirePorteur`,
    // hence before any HMAC check and before any database access.
    // Same position as the FAILURES brake of `routes-auth.ts`, and the same
    // reason: counting AFTER the work one seeks to bound does not bound
    // it. `OPTIONS` consumes nothing — the preflight only costs
    // 204 bytes and must not deprive the browser of its real request.
    const adresseRequete = adresseSource(
        req.socket.remoteAddress,
        Array.isArray(req.headers['x-forwarded-for'])
            ? req.headers['x-forwarded-for'].join(',')
            : req.headers['x-forwarded-for'],
        deps.proxyDeConfiance,
    );
    // 🔴 **THE SEVERITY OF A WRONGLY SET `PLATEFORME_PROXY_DE_CONFIANCE`
    // CHANGED WITH THIS BATCH (correction round 1, criticism ③), AND THIS IS
    // WHERE IT MUST BE SAID — this is the first of the three consultations of
    // `BUDGET_REQUETES` (`routes-session.ts`, `signaling/relais.ts` point
    // back to this paragraph).**
    //
    // The trap itself is not new: `adresseSource` (`http/
    // adresse-source.ts`) only believes `X-Forwarded-For` FROM a peer whose
    // `remoteAddress` is in `proxyDeConfiance`. A set that is too
    // wide — or a value that is no longer the proxy's — makes
    // `adresseRequete` become the SAME string for everyone: the one
    // the first comer chose to write into the header, or the one
    // of the proxy itself. The per-address brake then degenerates into a
    // GLOBAL brake.
    //
    // 🔴 **AND IT MUST BE SAID WITHOUT AN ATTACKER — THIS PARAGRAPH SAID "and
    // the first attacker blocks everyone", WHICH HAS BEEN FALSE SINCE
    // THIS BATCH STARTED BRAKING THE VOLUME** (falsified by the review, correction
    // round 4). **NO ATTACKER IS REQUIRED**: in the setup that
    // this repository SHIPS (`docker-compose.plateforme.yml`, nginx in front of the
    // platform even in `motdepasse` mode), `remoteAddress` is ALWAYS
    // the address of the nginx container for any real request — the
    // degeneration is therefore AUTOMATIC as soon as this setup exists, and
    // ORDINARY traffic is enough to exhaust the common budget. What the false
    // sentence suggested is that malice was needed to
    // reach it; users are all it takes.
    //
    // ⚠️ **THIS CORRECTION HOLDS FOR ALL THREE CONSULTATIONS**, this
    // paragraph being the one that `routes-session.ts` and
    // `signaling/relais.ts` point to: the error spread there by
    // reference, without being written there.
    //
    // 🔴 WHAT IS NEW: BEFORE THIS BATCH, this degeneration only capped
    // `ECHECS_MAX_ADRESSE` = 50 AUTHENTICATION failures per quarter
    // hour at that merged address — annoying, limited to the
    // `/auth/*` and `/agent` routes. **SINCE THIS BATCH, THE SAME DEGENERATION
    // CAPS THE WHOLE SERVICE AT `REQUETES_MAX_ADRESSE` EVENTS PER
    // WINDOW, HTTP AND WebSocket COMBINED**: `GET /vm`, `POST /session`
    // AND the `/signal` relay share that same counter (`cleRequetes`), at
    // that same merged address. An anonymous peer opens or consumes this
    // common budget, and nobody else — authenticated or not — can then
    // open a VM, a session, or a `/signal` connection behind that proxy.
    //
    // ⚠️ **AGGRAVATING, AND IT MUST BE SAID TOO**: the shipped profile
    // (`docker-compose.plateforme.yml`) sets `PLATEFORME_AUTH: motdepasse`,
    // a mode where `PLATEFORME_PROXY_DE_CONFIANCE` is OPTIONAL — nothing
    // forces anyone to set it correctly, or even to set it at all; it
    // lives in an UNVERSIONED `.env` file, hence invisible to any repository
    // review; and its correct value is the CONTAINER IP of nginx, which
    // CHANGES when the docker network is recreated — a value right yesterday can
    // be wrong today without any deployment having touched the
    // code.
    //
    // 🔴 **THE ONLY WITNESS**: the `frein-requetes` line that `compterLaRequete`
    // emits below (and its twins in `routes-session.ts` and
    // `relais.ts`), which NAMES the retained address — see its `adresse` field.
    // One same address on every line, all paths combined, IS the
    // signal. See also `PLATEFORME_PROXY_DE_CONFIANCE` in `CLAUDE.md`,
    // which documents the second role of this variable (the authorisation
    // of `X-Pomerium-Claim-Email`) — this note only concerns the
    // first, the credit given to `X-Forwarded-For`.
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

    // 🔴 AUTHENTICATION COMES BEFORE ANY DATABASE READ. A route that
    // read the inventory then refused the token would leak nothing through its
    // response, but it would offer free work to an anonymous peer.
    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        // ⚠️ THE CORS HEADERS ARE SET ON THE REFUSAL TOO: a 401 that the
        // browser cannot read shows up as a network failure, not
        // as an invitation to sign in again.
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }

    const orchestrateur = inventaireStatique(deps.base, deps.maintenant);
    // Filtering is done by the PURE module, never by an SQL clause written
    // here: a filter defect living in this layer would leak
    // the whole inventory, and there would be no place to make it fail without
    // standing up a server.
    const siennes = vmsDe(await orchestrateur.lister(), porteur.userId);

    if (isList) {
        // ⚠️ THE COUNT IS THE USER'S, NOT THE VM'S, and it
        // is only exact per VM because the partial index `vm_un_utilisateur`
        // guarantees AT MOST ONE VM per user. The day that invariant
        // falls, this field would become the user's total copied onto
        // every row — hence wrong. It is written here rather than discovered
        // later; `depot/session.ts` knows nothing about VMs, its table only carrying
        // `vm_id` since P3 and for the trace.
        const ouvertes = await compterOuvertesDe(deps.base, porteur.userId);
        const vms = [];
        for (const v of siennes) {
            vms.push({
                id: v.id,
                nom: v.nom,
                // The state is ASKED of the orchestrator, the sole holder of
                // the clock and the threshold: recomputing it here would duplicate the
                // rule, and the two copies would diverge the day one
                // changed.
                etat: await orchestrateur.etat(v.id),
                prefixe: v.prefixe,
                // ⚠️ NEITHER `adresse` NOR `userId`: the first is internal
                // topology the browser has no use for (D7), the
                // second is the requester's own, which it already knows.
                sessions_ouvertes: ouvertes,
            });
        }
        repondre(rep, 200, { vms }, cors);
        return true;
    }

    const { vmId, operation } = action!;
    // 🔴 "UNKNOWN" AND "SOMEONE ELSE'S" ARE THE SAME REFUSAL: the
    // lookup is done in `siennes`, so someone else's VM is missing
    // exactly like a nonexistent VM, and the body is produced by the same
    // path — so it CANNOT differ.
    // ⚠️ `BACKEND_STATIQUE` comes from `refus.ts`, never from a copied literal:
    // it is the SAME constant the orchestrator puts in its
    // refusals, so the two bodies cannot diverge.
    if (!siennes.some((v) => v.id === vmId)) {
        repondre(
            rep,
            CODE_HTTP['vm-inconnue'],
            { motif: 'vm-inconnue', operation, backend: BACKEND_STATIQUE },
            cors,
        );
        return true;
    }

    // The three verbs all refuse, and that is criterion ①. The `Outcome`
    // is not rebuilt here: it comes from the orchestrator, whose backend
    // name it carries.
    const issue =
        operation === 'demarrer'
            ? await orchestrateur.start(vmId)
            : operation === 'arreter'
              ? await orchestrateur.arreter(vmId)
              : await orchestrateur.instantane(vmId, '');
    if (issue.ok) {
        // Unreachable with the v1 backend — the three verbs refuse. Written
        // anyway: the day a hypervisor backend succeeds, this
        // branch exists and returns 200 rather than a silent `undefined`.
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
