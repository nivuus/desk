// `GET /auth/moi`: the identity set by Pomerium, exchanged for the internal
// token — the SAME token as the password one, byte for byte.
//
// 🔴 WHY THIS FILE EXISTS RATHER THAN ONE MORE ROUTE IN
// `routes-auth.ts`: the latter weighed 397 lines on 21 August 2026, and the
// 500-line rule wants a substantial addition to come with an
// extraction. These are moreover two different ISSUANCES of the same token,
// and they share no rule: one hashes a password, the other
// reads a header.
//
// 🔴 THE INTERNAL TOKEN IS NOT REPLACED, AND THAT IS THE HEART OF THE WHOLE
// WORK. It authenticates the relay handshake, which Pomerium cannot
// guard — the Windows agent has no browser, no cookie, no Google
// session. Removing it would cut the agent off.
//
// ⚠️ NO REFRESH TOKEN IS ISSUED, and that is not an oversight:
// the Pomerium cookie lives 8640 h, and keeping an anti-replay rotating chain that
// nobody needs any more would be living code that nothing exercises.
//
// 🔴 BUT « ON EXPIRY, THE CLIENT CALLS THIS ROUTE AGAIN » WAS FALSE, AND THE
// SENTENCE IS FIXED RATHER THAN DELETED (cross-cutting review of the work,
// 21 August 2026). **No client code calls this route again on
// expiry**: `client/src/jeton.ts::rafraichirSiNecessaire` has no
// production caller, and `connexion.ts::tenterPomerium` only runs at
// LOAD of the login page. What really calls `/auth/moi` again
// is therefore a page reload — a user gesture, which the 8640 h
// cookie makes silent for them, but which remains a gesture. The reasoning
// on refreshing does not change; the description of the product does.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { createUser, lireParEmail } from '../depot/utilisateur';
import { signer } from '../identite/jeton';
import { pairDeConfiance } from './adresse-source';
import { entetesCors } from './cors';
import { repondreIntrouvable } from './introuvable';
import { ENTETES_SECURITE } from './entetes';

export const CHEMIN_MOI = '/auth/moi';

/// The header Pomerium sets when the route declares
/// `pass_identity_headers: true`. **In lower case**: Node normalises the names
/// of incoming headers, and a comparison on the original case would
/// never match.
export const ENTETE_IDENTITE = 'x-pomerium-claim-email';

/// What we write in `empreinte_mdp`, which is `NOT NULL` (`0001-socle.sql`).
///
/// ⚠️ NEVER AN EMPTY STRING: it could one day meet a permissive
/// checker. This marker cannot match any format that
/// `identite/mot-de-passe.ts` can read (`scrypt$N$r$p$sel$empreinte`).
export const MARQUEUR_SANS_MOT_DE_PASSE = 'pomerium$aucun-mot-de-passe';

export type VerdictIdentite =
    | { ok: true; email: string }
    | { ok: false; motif: 'identite-absente' };

export interface DependancesIdentite {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    maintenant: () => number;
    auth: 'pomerium' | 'motdepasse';
    /// The set of addresses whose `X-Pomerium-Claim-Email` header is trusted.
    /// See the guard below, and `http/adresse-source.ts::pairDeConfiance`.
    proxyDeConfiance: ReadonlySet<string>;
}

/// 🔴 PURE: no database, no socket, no clock. That is what makes it testable
/// without standing up a server, and it is the convention of this whole directory.
export function lireIdentitePomerium(
    entetes: Record<string, string | string[] | undefined>,
): VerdictIdentite {
    const brut = entetes[ENTETE_IDENTITE];
    // ⚠️ THIS COMMENT SAID « a REPEATED header is refused », AND THAT IS FALSE
    // FOR THIS PRECISE PATH (measured, task 6, review « fix round 1 »,
    // 22 August 2026): Node does NOT yield an array for two occurrences of
    // `x-pomerium-claim-email` — that name is not in the short list
    // of headers Node exposes as an array (`set-cookie` is in it; this one
    // is not). Node JOINS them into ONE SINGLE string separated by `, ` before
    // this code even runs. The `Array.isArray` guard below is
    // therefore DEAD for this path: measured, two distinct headers from a
    // trusted peer today return `200` and create an account with the
    // joined email (`"a@b.c, evil@x.y"`). **It is a PRE-EXISTING defect,
    // deferred to the final review — not fixed here, only this comment is.**
    // What this guard really closes: the different case where an internal
    // CALLER builds `entetes` itself with an array (the tests of this
    // file do so), and the literal precedent of `porteur.ts`, which thus refuses
    // to disambiguate an ambiguous value when it DOES SHOW UP in
    // that shape.
    if (Array.isArray(brut) || brut === undefined) {
        return { ok: false, motif: 'identite-absente' };
    }
    const email = brut.trim();
    if (email === '') return { ok: false, motif: 'identite-absente' };
    return { ok: true, email };
}

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(JSON.stringify(corps));
}

/// Yields `true` if the request was served.
///
/// 🔴 IN `motdepasse` MODE, IT RETURNS THE `404` ITSELF — and that is what
/// carries the mode all the way to the client: the page is built statically by Vite and
/// cannot read any server variable, so it ASKS. A `403`
/// would say « the route exists, you have no right to it », which would invite
/// trying again; `404` tells the truth.
///
/// 🔴 IT RETURNED `false` UNTIL 22 AUGUST 2026, TO LET THE GENERIC 404
/// OF THE SERVER ANSWER — AND THAT MECHANISM DIED SILENTLY in the
/// « page behind Pomerium » batch. The file server, chained LAST,
/// folds every path without an extension onto the page (`hub.html` since
/// 30 August 2026, `index.html` before — see `page/resolution.ts::PAGE`):
/// `GET /auth/moi` in `motdepasse` mode with `PLATEFORME_PAGE` armed returned
/// `200 text/html` (measured). The client only broke by accident — its
/// `.catch(() => undefined)` made the form fall in the right place.
///
/// ⚠️ IT IS NOT A SECOND 404: it is THE SAME ONE, `http/introuvable.ts`, the one
/// `serveur.ts` returns too. A text written by hand here would drift from
/// the server one without anything saying so.
export async function servirIdentite(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesIdentite,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    // EXACT comparison, never a `startsWith`.
    if (chemin !== CHEMIN_MOI) return false;
    // 🔴 THE INVARIANT OF THE TWO MODE GUARDS, WRITTEN HERE AND IN `routes-auth.ts`
    // BECAUSE IT BELONGS TO NEITHER ONE NOR THE OTHER: **the two guards have
    // OPPOSITE POLARITIES** — this one steps aside if the mode is NOT
    // `pomerium`, the one of `routes-auth.ts` if it is NOT `motdepasse` —, and
    // that is what makes them PARTITION the modes: with TWO modes, every mode
    // opens exactly one of the two doors.
    //
    // 🔴 WITH THREE MODES, BOTH ANSWER `404` TOGETHER and the service has
    // NO authentication route left, **silently**: two `404`s, each right
    // taken alone, and nothing saying that no door is open.
    // **Adding a value to `AUTHS` (`config.ts`) FORCES coming back here** to
    // decide which of the two doors the new mode opens — TypeScript will not
    // ask, as these guards compare strings rather than an exhaustive
    // `switch`.
    if (deps.auth !== 'pomerium') {
        repondreIntrouvable(rep);
        return true;
    }

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

    // 🔴 THE GUARD THAT CLOSES THE BYPASS. Without it, `/auth/moi` returns a
    // valid internal token for ANY email set in a header
    // that NO SIGNATURE CHECKS: anyone who reaches the port — hence the Windows
    // VM, which § 7.1 of the `auth-pomerium` spec places by name within this
    // perimeter — authenticates under the identity of their choosing.
    //
    // ⚠️ IT IS PLACED BEFORE THE HEADER IS READ, NOT AFTER. After, it
    // would be right too — but the service would already have read an identity it
    // refuses, and a successor could move the read without seeing that the
    // guard depended on it.
    //
    // ⚠️ WHAT IT DOES NOT PROMISE: that only Pomerium carries that address. That
    // stays the operator's responsibility, as the listen guard of
    // `PLATEFORME_HOTE` already says of itself.
    if (!pairDeConfiance(req.socket.remoteAddress, deps.proxyDeConfiance)) {
        repondre(rep, 401, { refus: 'pair-non-de-confiance' }, cors);
        return true;
    }

    const identite = lireIdentitePomerium(req.headers);
    if (!identite.ok) {
        repondre(rep, 401, { refus: identite.motif }, cors);
        return true;
    }

    const maintenant = deps.maintenant();
    const id = await identifiantDe(deps.base, identite.email, maintenant);
    repondre(rep, 200, { acces: signer(id, deps.secretJeton, maintenant) }, cors);
    return true;
}

/// The id of the account, created if it does not exist.
///
/// ⚠️ THE SECOND READ IS NOT DEFENSIVE, IT CLOSES A REAL RACE:
/// two simultaneous requests from the same unknown user would both
/// pass the first read, and the second insert would violate the UNIQUE index
/// on the email (`0001-socle.sql`) — a `500` on the very first page
/// opening. We then read again, and only rethrow the error if the account is
/// still missing, in which case it says something other than a race.
async function identifiantDe(base: Pilote, email: string, maintenant: number): Promise<string> {
    const existant = await lireParEmail(base, email);
    if (existant !== undefined) return existant.id;
    try {
        return await createUser(base, email, MARQUEUR_SANS_MOT_DE_PASSE, maintenant);
    } catch (cause) {
        const rattrape = await lireParEmail(base, email);
        if (rattrape !== undefined) return rattrape.id;
        throw cause;
    }
}
