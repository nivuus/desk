// The SHAPE of the tag the Vite plugin `guac-manifeste-hub` injects into
// `hub.html` — extracted from `client/vite.config.ts` for the SAME reason as
// `design/amorce-theme-greffon.ts`: STAYING TYPECHECKED. `vite.config.ts`
// is never typechecked (outside the two patterns of `client/tsconfig.json:12`),
// and a defect in the shape of this tag would therefore never be seen by
// `npx tsc --noEmit` if it stayed written there.
//
// 🔴 THE DEFECT THIS FILE FIXES, measured on August 29th, 2026
// (`https://app.allanic.me/hub.html`, Chrome, the owner's console):
//
//   Loading a manifest from 'https://authenticate.allanic.me/.pomerium/
//   sign_in?...&pomerium_redirect_uri=…%2Fhub.webmanifest&...' violates the
//   following Content Security Policy directive: "default-src 'self'".
//
// A `<link rel="manifest">` WITHOUT `crossorigin` is fetched by the
// browser WITHOUT the session cookies (that is the HTML rule: a manifest
// link only travels in `credentials: same-origin` mode IF it carries
// `crossorigin="use-credentials"` — the absence of the attribute means
// `crossorigin="anonymous"`, hence NO cookie). Pomerium, receiving an
// unauthenticated request on an otherwise protected route
// (`config.yaml`: only `email: maxime.g.allanic@gmail.com` or the extensions
// `.ico`/`.png`/`manifest.json` pass), answers with a redirect to its
// own domain `authenticate.allanic.me` — ANOTHER origin, which
// `default-src 'self'` (and `manifest-src 'self'`, now explicit —
// `entetes-page.ts::CSP`) refuses to load. The CSP message is the
// SYMPTOM; the unauthenticated redirect is the CAUSE.
//
// 🔴 WHY `crossorigin="use-credentials"` RATHER THAN OPENING THE ROUTE IN
// POMERIUM: the hub's manifest stays PRIVATE behaviour — no new
// entry in the `ends_with: …` policy of `config.yaml`, no hole. The
// browser now sends the cookies with the manifest request,
// exactly as it does for `hub.html` itself: Pomerium authenticates
// the request normally and serves `/hub.webmanifest`, which answers `200`
// (checked: `curl http://192.168.3.1:3445/hub.webmanifest`).
export function baliseManifesteHub() {
    return {
        tag: 'link' as const,
        attrs: { rel: 'manifest', href: '/hub.webmanifest', crossorigin: 'use-credentials' },
        injectTo: 'head' as const,
    };
}
