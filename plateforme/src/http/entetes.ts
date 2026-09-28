// The two security headers the PLATFORM sets on EVERY JSON
// RESPONSE — not the whole set it sets: since the « page behind
// Pomerium » batch (22 August 2026), a second set exists for the DOCUMENT it
// can now serve (Content-Security-Policy, Referrer-Policy,
// X-Frame-Options), and lives in `page/entetes-page.ts` — never here.
//
// 🔴 THIS MODULE IS SEPARATE FROM `cors.ts`, AND THE SEPARATION IS THE POINT.
// `entetesCors` yields `undefined` when the origin is not allowed; these ones
// are UNCONDITIONAL. Merging them would make security depend on an
// OPTIONAL CORS configuration (`PLATEFORME_ORIGINE_CLIENT` is not
// mandatory, see `config.ts`) — that is, a deployment on a
// single origin, where no CORS header makes sense, would also lose its
// security headers, without anything saying so.
//
// THE SPLIT WITH THE PROXY, and it follows what each one SERVES:
//
//   platform (any JSON response)    | X-Content-Type-Options | it alone
//                                   |                        | knows its
//                                   |                        | content type
//   platform (any JSON response)    | Cache-Control          | see below
//   proxy OR platform (the HTML)    | Content-Security-Policy | applies to the
//                                   |                        | DOCUMENT — and
//                                   |                        | since the
//                                   |                        | « page behind
//                                   |                        | Pomerium » (22
//                                   |                        | August 2026) batch,
//                                   |                        | the platform MAY
//                                   |                        | serve it: see
//                                   |                        | page/entetes-page.ts
//   proxy                           | Strict-Transport-Security | it is the one
//                                   |                        | that ends TLS
//   proxy OR platform (the HTML)    | Referrer-Policy,       | ditto — same
//                                   | frame-ancestors        | reason and
//                                   |                        | module as the
//                                   |                        | CSP above:
//                                   |                        | `frame-ancestors`
//                                   |                        | is A
//                                   |                        | directive OF
//                                   |                        | the CSP, not a
//                                   |                        | separate header
//
// 🔴 WHY `no-store` IS HERE AND NOT AT THE PROXY: the responses of
// `/auth/*` CARRY TOKENS — the access token and the refresh
// token, in clear in the JSON body. An intermediate cache, or
// simply the browser disk, would keep them. It is the platform that
// knows which of its responses carries a secret; the proxy does not know.
//
// ⚠️ WHAT THESE TWO HEADERS DO NOT DO, AND WHAT HAS CHANGED: they
// replace NEITHER the CSP, NOR HSTS, NOR `frame-ancestors`. **HSTS stays
// exclusively at the proxy** — it alone ends TLS (spec §9), and asserting it
// from an origin served in clear would be a claim the platform
// is in no position to make (see `page/entetes-page.ts`). **CSP,
// `frame-ancestors` and `Referrer-Policy`, on the other hand, are NO LONGER
// exclusively at the proxy** since the « page behind Pomerium » batch: the
// platform can now serve the page itself, and then sets its
// own document headers — `ENTETES_DOCUMENT` of `page/entetes-page.ts`,
// never `ENTETES_SECURITE` from here, reserved for JSON responses. A deployment
// without a proxy is therefore NOT covered by THIS module (`entetes.ts`), but may
// be by `page/entetes-page.ts` for everything that depends on the document rather
// than on TLS.

export const ENTETES_SECURITE: Readonly<Record<string, string>> = Object.freeze({
    // The browser does not GUESS the type: a JSON response that an attacker
    // could get read as HTML would become an XSS vector.
    'X-Content-Type-Options': 'nosniff',
    // See the header: `/auth/*` yields tokens. `no-store` is stronger than
    // `no-cache`, which allows writing to disk provided it
    // revalidates.
    'Cache-Control': 'no-store',
});
