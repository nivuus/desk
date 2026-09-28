// CORS, and the refusal by default.
//
// 🔴 The value `*` is NEVER produced, and it is not an intention: it is
// an assertion, swept over every outcome of every case of this file.

import { describe, expect, it } from 'vitest';
import { entetesCors } from './cors';

const AUTORISEE = 'http://127.0.0.1:5173';

describe('entetesCors', () => {
    it('emits NO header when no origin is allowed', () => {
        // The default is refusal, never opening: without
        // `PLATEFORME_ORIGINE_CLIENT`, the browser refuses to read the response,
        // which the operator sees immediately.
        expect(entetesCors(AUTORISEE, undefined)).toBeUndefined();
        expect(entetesCors(undefined, undefined)).toBeUndefined();
    });

    it('emits nothing to whoever does not ask — a non-browser request', () => {
        // Returning the allowed origin to a caller that has no `Origin`
        // makes no sense and discloses the configuration.
        expect(entetesCors(undefined, AUTORISEE)).toBeUndefined();
    });

    it('REFUSES a different origin, with no prefix and no inclusion', () => {
        // A comparison by `startsWith` would accept
        // `http://127.0.0.1:5173.attaquant.test`.
        expect(entetesCors('http://mechant.test', AUTORISEE)).toBeUndefined();
        expect(entetesCors('http://127.0.0.1:5173.mechant.test', AUTORISEE)).toBeUndefined();
        expect(entetesCors('http://127.0.0.1:517', AUTORISEE)).toBeUndefined();
    });

    it('🔴 announces GET, POST, OPTIONS — `GET /vm` needs them', () => {
        // 🔴 The red: leaving `POST, OPTIONS`. The preflight request of
        // `GET /vm` would then receive a list of methods that does not contain
        // its own, and the browser would refuse the real request — WITHOUT
        // any Node test seeing it, since the tests speak Node `fetch`,
        // which does not apply the origin policy (see the header of
        // `cors.ts`).
        //
        // ⚠️ Sub-block G1 needs the SAME change: it is
        // identical and idempotent, and the second branch to arrive will find it
        // done.
        const entetes = entetesCors(AUTORISEE, AUTORISEE);
        expect(entetes!['Access-Control-Allow-Methods']).toBe('GET, POST, PUT, OPTIONS');
    });

    it('🔴 announces `PUT` — otherwise the UPLOAD of G3 is unreachable', () => {
        // 🔴 THIRD TIME THIS CLASS BITES, and P4 had named it while
        // declaring it WITHOUT AN AUTOMATIC GUARD: "what a browser requires and
        // a server test does not see". It bit twice there
        // (`Authorization` not allowed, then the preflight not handled); it
        // bites here on the METHOD.
        //
        // `PUT /televersement/:id/tranche/:n` is the FIRST `PUT` route of
        // the whole service, and its caller IS the browser — it is the browser that
        // chunks the file and drops the chunks. Carrying `Authorization`,
        // it is NON-SIMPLE: the browser sends a preflight carrying
        // `Access-Control-Request-Method: PUT` and **gives up without ever
        // sending the real request** if the response does not announce it.
        //
        // ⚠️ NO EFFECT WITH A SINGLE ORIGIN — the `deploiement` profile puts the page
        // and the API behind the same nginx —, BITING IN DEVELOPMENT, where
        // `vite` serves the client on 5173 and the service listens on 8080. The
        // upload would therefore fail where it is being developed, and nowhere
        // else: the worst place for a defect.
        //
        // ⚠️ THIS ASSERTION IS THE ONLY POSSIBLE GUARD. No Node `fetch`
        // applies the origin policy, so no end-to-end test
        // can make this defect red — not even a test that mounted the
        // service and sent a real `PUT`, since it would succeed.
        //
        // 🔴 The red: returning `'GET, POST, OPTIONS'`, the value from before G3.
        const entetes = entetesCors(AUTORISEE, AUTORISEE);
        const permises = entetes!['Access-Control-Allow-Methods'].split(', ');
        expect(permises).toContain('PUT');
        // The other three remain, and saying so is what prevents a hasty
        // fix from replacing the list instead of extending it: `GET` serves P4 and
        // G1, `POST` serves P2 and G3's sealing, `OPTIONS` is the preflight
        // itself.
        expect(permises).toContain('GET');
        expect(permises).toContain('POST');
        expect(permises).toContain('OPTIONS');
    });

    it('🔴 allows the `authorization` header — otherwise NO route of P4 is reachable', () => {
        // 🔴 A DEFECT OF THE PLAN, FOUND AND NOT COPIED. Task 8 only prescribed
        // `GET` in `Access-Control-Allow-Methods`. But both routes of
        // P4 require `Authorization: Bearer` (`http/porteur.ts`), and an
        // `Authorization` header makes the request NON-SIMPLE: the browser sends a
        // preflight request carrying `Access-Control-Request-Headers:
        // authorization`, to which a server that only answers
        // `content-type` opposes a refusal. Both routes would therefore be
        // UNREACHABLE from the browser, and P4 would ship an HTTP surface
        // its own client cannot call.
        //
        // ⚠️ NO NODE TEST COULD SEE IT — it is exactly what
        // the header of `cors.ts` announces about itself: "otherwise the browser
        // refuses to read the response, without any server-side test seeing
        // it". The guard is therefore this assertion, and nothing else.
        //
        // 🔴 La rouge : laisser `content-type` seul.
        const entetes = entetesCors(AUTORISEE, AUTORISEE);
        const permis = entetes!['Access-Control-Allow-Headers'].split(', ');
        expect(permis).toContain('authorization');
        // `content-type` remains: `POST /auth/connexion` needs it, and
        // removing it would break P2 without any line of P4 asking for it.
        expect(permis).toContain('content-type');
    });

    it('emits the origin AND Vary: Origin when it matches exactly', () => {
        const entetes = entetesCors(AUTORISEE, AUTORISEE);
        expect(entetes).toBeDefined();
        expect(entetes!['Access-Control-Allow-Origin']).toBe(AUTORISEE);
        // 🔴 Without `Vary`, an intermediate cache would serve the response of one
        // origin to another.
        expect(entetes!['Vary']).toBe('Origin');
    });

    it('NEVER produces the value `*`, whatever the input', () => {
        const entrees: Array<[string | undefined, string | undefined]> = [
            ['*', '*'],
            ['*', AUTORISEE],
            [AUTORISEE, '*'],
            [AUTORISEE, AUTORISEE],
            [undefined, '*'],
            ['null', 'null'],
        ];
        for (const [demandee, autorisee] of entrees) {
            const entetes = entetesCors(demandee, autorisee);
            if (entetes) expect(entetes['Access-Control-Allow-Origin']).not.toBe('*');
        }
    });
});
