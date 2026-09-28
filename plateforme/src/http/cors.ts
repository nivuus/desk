// The CORS headers, or nothing. PURE function, no regular expression, one
// string equality.
//
// 🔴 WHY THIS MODULE EXISTS, and why the spec does not mention it: found on
// 19 August 2026, `client/vite.config.ts` serves the browser on 5173 while
// `config.ts` listens on 8080. A browser `POST` to `/auth/connexion`
// is therefore CROSS-ORIGIN, and without `Access-Control-Allow-Origin` the browser
// refuses to read the response — without any server-side test seeing it,
// since the tests speak through Node `fetch`.
//
// 🔴 THE DEFAULT IS REFUSAL: allowed origin absent, no header. And the
// value `*` is produced UNDER NO CONDITION — a wildcard would allow
// any site to talk to this API on behalf of the browser of a
// signed-in user.
//
// ⚠️ The requested origin is COMPARED, never sent back as is: sending back
// the requester's `Origin` amounts to allowing everybody while saying it in another
// way. And the comparison is an EQUALITY, never a prefix — a
// `startsWith` would accept `http://127.0.0.1:5173.attaquant.test`.

export function entetesCors(
    origineDemandee: string | undefined,
    origineAutorisee: string | undefined,
): Record<string, string> | undefined {
    if (origineAutorisee === undefined || origineAutorisee === '') return undefined;
    if (origineDemandee === undefined || origineDemandee === '') return undefined;
    if (origineDemandee !== origineAutorisee) return undefined;
    // Never `*`: the value returned is the CONFIGURED origin, which we have just
    // verified is also the requested one.
    if (origineAutorisee === '*') return undefined;
    return {
        'Access-Control-Allow-Origin': origineAutorisee,
        // Without `Vary`, an intermediate cache would serve the response of one
        // origin to another.
        Vary: 'Origin',
        // `GET` since P4: `GET /vm` is the first route of this service that
        // the browser reaches other than through `POST`.
        //
        // ✅ AND G1 HAD NOTHING TO CHANGE HERE: `GET /applications` is the
        // second consumer of the same value, and the sub-block found it
        // already set. The change its plan prescribed was therefore
        // idempotent, and its "free red" was no longer playable — P4
        // had played it, and its test announced it in so many words
        // (`cors.test.ts`). Checked rather than assumed done.
        //
        // 🔴 `PUT` SINCE G3, AND WITHOUT IT THE UPLOAD IS UNREACHABLE
        // FROM A CROSS-ORIGIN BROWSER. `PUT /televersement/:id/
        // tranche/:n` is the FIRST `PUT` route of the whole service, and its
        // caller IS the browser: it is the one that slices the file and
        // uploads the slices. A `PUT` carrying `Authorization` is a NON-SIMPLE
        // request — the browser first sends a preflight carrying
        // `Access-Control-Request-Method: PUT`, and **gives up without ever
        // sending the real request** if the response does not announce it. No
        // effect with a single origin (the `deploiement` profile, where nginx serves the
        // page and the API on the same origin); **biting in development**,
        // where `vite` serves the client on 5173 and the service listens on 8080.
        //
        // ⚠️ THIS IS THE CLASS THAT P4 NAMED AND DECLARED WITHOUT AN AUTOMATIC
        // GUARD — « what a browser demands and a server test cannot
        // see ». It bit twice in P4 (`Authorization` not allowed,
        // preflight not handled) and a third time here. The only possible
        // guard remains an ASSERTION ON THE VALUE, in `cors.test.ts`:
        // no Node `fetch` applies the origin policy, so no
        // end-to-end test can turn it red.
        'Access-Control-Allow-Methods': 'GET, POST, PUT, OPTIONS',
        // 🔴 `authorization` SINCE P4, AND WITHOUT IT NOTHING IS REACHABLE.
        // The two P4 routes — and the two G1 ones — require
        // `Authorization: Bearer`
        // (`http/porteur.ts`), and that header makes the request NOT SIMPLE: the
        // browser sends a preflight request carrying
        // `Access-Control-Request-Headers: authorization`, which a server
        // answering only `content-type` refuses. ⚠️ Like everything this file
        // settles, NO Node test can see it — see the header: the tests
        // speak through Node `fetch`, which does not apply the origin policy.
        // The guard is the assertion of `cors.test.ts`, and nothing else.
        'Access-Control-Allow-Headers': 'content-type, authorization',
    };
}
