// The `404` of the service, in ONE SINGLE place.
//
// 🔴 WHY THIS MODULE EXISTS, AND IT IS NOT A TASTE FOR FACTORING. The
// `404` of the service carried a CONTRACT documented at both ends: the two
// mode guards (`routes-identite.ts` and `routes-auth.ts`) stepped aside
// to let it answer, and `client/src/connexion.ts` READS that `404` as
// « this deployment authenticates by password ». The « page behind
// Pomerium » batch chained a file server LAST, whose SPA fallback
// resolves any path at all: the `404` the guards relied on has
// become, in `motdepasse` mode with the page armed, a `200 text/html`. MEASURED:
// `GET /auth/moi` returned `200 text/html` instead of `404`.
//
// 🔴 THE DEFECT HAD A SYMMETRIC TWIN, and that is what decided the remedy.
// `routes-auth.ts` carries the guard of OPPOSITE polarity — it steps aside in
// `pomerium` mode —, so that `GET /auth/connexion` was swallowed the same way
// in the other mode. The two guards PARTITION the modes: they therefore have
// the SAME defect, each in the other mode.
//
// 🔴 THE REMEDY CHOSEN: THE TWO GUARDS ANSWER THE `404` THEMSELVES, instead
// of delegating it. The alternatives weighed, and why they give way:
//   - exclude the API prefixes from the SPA fallback — one would have to keep a LIST,
//     and an API route added without updating the list would fall back
//     silently into the fallback: one would swap one silent failure for
//     another;
//   - make the fallback depend on the `Accept` header — self-maintained, but it
//     DIVERGES from nginx, whose `try_files` is unconditional, and makes
//     the behaviour depend on a header the client does not
//     always control;
//   - document that the promise no longer holds — that would knowingly ship
//     a broken mechanism.
// Answering directly is surgical, needs no list, and keeps
// EXACTLY the documented contract: « the 404 tells the truth ».
//
// ⚠️ THE BODY IS NOT TOUCHED, AND THAT IS THE WHOLE POINT OF HAVING IT HERE. It
// comes from P1, `routes-auth.test.ts` freezes it, and a SECOND form of 404 —
// written by hand in each guard — would drift from the server one without
// anything saying so. One text, one set of headers, three callers.

import type { ServerResponse } from 'node:http';
import { ENTETES_SECURITE } from './entetes';

/// The exact body, kept WORD FOR WORD since P1.
export const CORPS_INTROUVABLE = 'not found\n';

/// ⚠️ `nosniff` IS NOT OPTIONAL HERE: without `ENTETES_SECURITE`, an unknown
/// path would be the ONLY response of the service not to carry it — the reason
/// why `serveur.ts` already handled its 404 itself rather than
/// leaving it to Node.
export function repondreIntrouvable(rep: ServerResponse): void {
    rep.writeHead(404, {
        'content-type': 'text/plain; charset=utf-8',
        ...ENTETES_SECURITE,
    });
    rep.end(CORPS_INTROUVABLE);
}
