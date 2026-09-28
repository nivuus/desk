// `GET /sante`: does the database answer? And nothing else.
//
// 🔴 WHAT THIS ROUTE DOES NOT RETURN, AND THAT IS THE ESSENCE: no version, no
// session count, no database URL, no engine name, no
// uptime. A chatty health page is an INVENTORY offered to an
// anonymous party. Its test compares the WHOLE object, never a substring,
// precisely so that a future addition turns it red.
//
// 🔴 « SINCE P2, IT IS BY CONSTRUCTION THE ONLY UNAUTHENTICATED ROUTE OF THE
// SERVICE » — THAT SENTENCE WAS HERE, AND IT DIED ON 22 AUGUST 2026: the
// page server (`http/page/routes-page.ts`, armed by `PLATEFORME_PAGE`) is
// a SECOND one, and it consults no token. It is rewritten rather than
// deleted, because what it protected stays true in a NARROWER
// and more useful form: `/sante` is the only unauthenticated route that
// TOUCHES THE DATABASE. That is exactly what the cache below exists to
// bound — the page server, for its part, only reads a disk, and nothing anonymous there
// translates an HTTP request into an SQL query.
//
// 🔴 THE VERDICT IS CACHED, AND THE CACHE IS THE POINT OF THIS ROUTE, NOT
// A REFINEMENT. Without it, `/sante` translates an ANONYMOUS HTTP request into
// an SQL query, at will: that is an amplification, on the very route a
// load balancer calls in a loop. An attacker would only need to
// hammer it to shift their load onto the database.
//
// ⚠️ IT IS NOT BRAKED, AND THAT IS DELIBERATE: a braked load balancer probe
// would declare the service DEAD, and would cause the failure it
// watches for. The cache is what makes it safe WITHOUT a brake — that is why the
// two decisions live in the same paragraph.
//
// ⚠️ IT IS NOT A CORRECTNESS PROBE. It says the database ANSWERS, never
// that the service SERVES: a route can be broken, a channel silent, a session
// never paired, and `/sante` will return `ok`. Nor does it say that the
// service is READY — `demarrage.ts` already guarantees that the port only opens
// after the database and its migrations, and its order is commented « NOT
// NEGOTIABLE ». `/sante` only REPORTS; it must never become
// a second place that decides whether the service is ready.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';

/// The duration for which a verdict is reused.
///
/// ⚠️ NOT CALIBRATED: one second is an order of magnitude, chosen to be
/// far below the usual interval of a load balancer probe (5 to 30 s) —
/// so that the cache never hides a failure from whoever is watching —
/// while absorbing a burst. No measurement set it.
export const PERIODE_SANTE_MS = 1000;

const CHEMIN = '/sante';

export interface DependancesSante {
    base: Pilote;
    origineClient?: string;
    /// 🔴 THE CLOCK IS A PARAMETER, never `Date.now()` read here: that is what
    /// makes the cache expiry assertable on an EXACT value within
    /// a test run, where there would otherwise be only one instant.
    maintenant: () => number;
    cache: CacheSante;
}

/// The verdict, and its date. Lives for the lifetime of the service, like
/// `ProprieteDeSession` and `RegistreAgents`.
export class CacheSante {
    private verdictRetenu: boolean | undefined;
    private prisA = 0;
    /// 🔴 THE IN-FLIGHT QUERY, AND WITHOUT IT THE CACHE IS USELESS UNDER THE
    /// LOAD IT EXISTS TO ABSORB. The real case is several load balancer
    /// probes in flight at the same instant: without deduplication, each would
    /// launch its own query, and the cache would only act AFTER the burst.
    private enVol: Promise<boolean> | undefined;

    /// Yields `true` if the database answers, reusing the verdict younger than
    /// `PERIODE_SANTE_MS`.
    verdict(base: Pilote, maintenant: number): Promise<boolean> {
        if (this.verdictRetenu !== undefined && maintenant - this.prisA < PERIODE_SANTE_MS) {
            return Promise.resolve(this.verdictRetenu);
        }
        if (this.enVol !== undefined) return this.enVol;

        this.enVol = this.demander(base, maintenant);
        return this.enVol;
    }

    private async demander(base: Pilote, maintenant: number): Promise<boolean> {
        let vivante: boolean;
        try {
            // ⚠️ `SELECT 1` CARRIES A LITERAL VALUE, AND THAT IS HARMLESS
            // HERE: the repository rule — « no literal value in a
            // query » — targets values that come from a REQUESTER, and the
            // lint of `base/sous-ensemble.test.ts` only applies to the (policy: allow-fr - file name)
            // MIGRATIONS. `rendreMarqueurs` only refuses literal STRINGS
            // (apostrophe or double quote); `1` is not one, and
            // the query takes no parameter.
            await base.interroger('SELECT 1', []);
            vivante = true;
        } catch {
            // ⚠️ THE CAUSE IS NOT LOGGED HERE, and that is not an oversight:
            // an unreachable database makes ALL the routes fail, which
            // already log their own failure (`http/serveur.ts` does so
            // for the five). One more line PER PROBE, on a route a
            // load balancer calls in a loop, would make the service an
            // amplifier at the precise moment it is unwell — that is the rule
            // « never trace per packet » of the TURN work, applied at the worst
            // possible moment to break it.
            vivante = false;
        }
        this.verdictRetenu = vivante;
        this.prisA = maintenant;
        this.enVol = undefined;
        return vivante;
    }
}

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

/// Yields `true` if the request was served, `false` if it is not about
/// health — the server then answers 404, like the nine other routers.
///
/// ⚠️ THIS SENTENCE IS NO LONGER UNCONDITIONALLY TRUE SINCE 22 AUGUST 2026:
/// when `PLATEFORME_PAGE` is armed, a TENTH router — the page
/// server — is chained AFTER all the others, and it resolves any
/// path. On a `GET`, it is IT that answers `200 text/html` to the `false`
/// returned here; outside `GET`/`HEAD` it steps aside, and the generic 404 takes
/// over. See `http/chaine.ts`, which carries the count and the rule. (policy: allow-fr - file name)
export async function servirSante(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesSante,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    // EXACT comparison, never a `startsWith`: `/santelle` is not
    // `/sante`, and a prefix would open a family of paths nobody
    // decided on.
    if (chemin !== CHEMIN) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    if (req.method === 'OPTIONS') {
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }
    if (req.method !== 'GET') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    const vivante = await deps.cache.verdict(deps.base, deps.maintenant());
    // 503 and not 500: the service is TEMPORARILY unavailable, which is
    // exactly what a load balancer must read to take the instance out of
    // service without declaring it dead for good.
    repondre(rep, vivante ? 200 : 503, { etat: vivante ? 'ok' : 'degrade' }, cors);
    return true;
}
