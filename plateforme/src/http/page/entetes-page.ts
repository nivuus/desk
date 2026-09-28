// The headers the platform sets on what it SERVES as a page.
//
// 🔴 THIS MODULE EXISTS BECAUSE `../entetes.ts` COULD NOT SERVE HERE, and
// the reason is an inversion, not a gap: its `Cache-Control: no-store`
// is UNCONDITIONAL, and it is there for the responses of `/auth/*`, which carry
// tokens in clear. FINGERPRINTED assets have the EXACTLY
// OPPOSITE need — cacheable for a year. Reusing `ENTETES_SECURITE` as is is the
// natural move, and it is the defect.
//
// 🔴 BUT « THE ASSETS » WAS TOO BROAD A WORD, AND IT COST A YEAR OF
// CACHE ON THE HUB PWA MANIFEST. The first draft had only TWO
// header sets, document and asset, and classified by EXTENSION: the MIME
// list admits `webmanifest`, `json`, `ico`, `png`, which Vite NEVER fingerprints
// at the root. MEASURED on the real `client/dist`: `/hub.webmanifest` returned
// `public, max-age=31536000, immutable`, thus **not revisable for a year**
// in any browser that had seen it. Hence THREE sets, not two — the
// distinction itself lives in `resolution.ts` (`empreinte`), because
// it is the RULE that classifies, not the server.
//
// 🔴 HSTS IS NOT HERE, AND THAT IS DELIBERATE. The dividing line with the proxy
// becomes: what depends on the DOCUMENT follows the document; what depends on TLS
// stays with whoever ends TLS. The platform is reachable in clear.

/// The content security policy.
///
/// 🔴 IT IS COPIED FROM `deploiement/nginx.conf`, AND `entetes-page.test.ts`
/// READS BOTH AND COMPARES THEM. Without that test, a hardening applied on
/// one side only would ship two deployments with differing security.
///
/// 🔴 `script-src 'self'` WITHOUT `'unsafe-inline'` OR A HASH, AND THAT IS DELIBERATE —
/// measured broken on 29 August 2026 (`https://app.allanic.me`, Chrome) as long as
/// the anti-FOUC bootstrap of `client/` went INLINE in the HTML: this module
/// and `client/vite.config.ts` live in two packages, and nothing tied them
/// together before `client/src/design/amorce-theme.csp.test.ts`. Fixed on the
/// CLIENT side (the bootstrap is now an external `'self'` file, never
/// inline) rather than here by a hash: this file has a STATIC copy in
/// `deploiement/nginx.conf` (line above) that can NOT compute a
/// hash on the fly over the content it serves — a hash would therefore have had to be
/// copied by hand into BOTH files, the « shipwreck of 487 » that
/// `CLAUDE.md` forbids. Read the large comment above
/// `BOOTSTRAP_FILE_NAME` in `client/vite.config.ts` before proposing a
/// hash here again.
///
/// 🔴 `manifest-src 'self'` IS EXPLICIT, AND NOT A FALLBACK TO `default-src` —
/// measured broken on 29 August 2026 (`https://app.allanic.me/hub.html`, Chrome):
/// « manifest-src was not explicitly set, so default-src is used as a
/// fallback » (the browser SAYS so itself in the violation message).
/// The real defect was not the missing directive but a
/// `<link rel="manifest">` without `crossorigin="use-credentials"` — fixed
/// on the CLIENT side (`client/src/hub/manifeste-hub-greffon.ts`), on the same
/// principle as `script-src` two paragraphs above: an implicit fallback
/// is a rule nobody wrote, so we write it, even when it
/// was not the one blocking.
///
/// 🔴 `blob:` HAD TO JOIN `manifest-src` ON 30 AUGUST 2026 — a regression
/// ordered by THIS repository THE DAY BEFORE, and found IN PRODUCTION by the
/// owner (`https://app.allanic.me`, a loop of violations):
///
///   Loading a manifest from 'blob:https://app.allanic.me/…' violates the
///   following Content Security Policy directive: "manifest-src 'self'".
///
/// The HUB manifest is served over HTTP (`hub.webmanifest`, covered by
/// `'self'`), but the PER-APPLICATION manifest cannot be: no
/// authenticated route serves it (⑤ sets no cookie), so
/// `client/src/hub/page.ts::publierLeManifeste` builds it in memory and
/// publishes it via `URL.createObjectURL` — path V1 of G5, documented in
/// `client/hub.html` and `client/src/hub/manifeste.ts`. By writing ONLY
/// `'self'`, the explicit rule of yesterday made VISIBLE — and hence BLOCKING — what
/// the implicit fallback to `default-src 'self'` was already blocking silently
/// (`default-src` did not carry `blob:` either): the owner's loop
/// is the symptom of a pre-existing defect, not a pure behaviour
/// regression — but a VISIBILITY regression is enough to break a shipped
/// feature (G5), and that is indeed what happened.
///
/// ⚠️ WHAT `blob:` ADMITS HERE, AND WHY IT IS ACCEPTABLE: a `blob:`
/// origin is not a third party — it is a URL that THE PAGE ITSELF makes,
/// from bytes IT built (`new Blob([JSON.stringify(...)])`),
/// and that no network request can produce from outside: an
/// attacker who does not already have active JavaScript in this origin cannot
/// make `<link rel="manifest">` navigate to a `blob:` of their choosing.
/// Admitting `manifest-src blob:` thus amounts to saying « I trust what
/// MY script produces », not « I trust an external origin » —
/// it is the same trust that `script-src 'self'` already grants to all the
/// code of this page, one notch lower. The theoretical risk is that a
/// successful XSS injection could make its own `blob:` anyway
/// (or worse, run script directly): `manifest-src blob:` therefore opens
/// no surface that an XSS would not already open. **Acceptable.**
export const CSP =
    "default-src 'self'; connect-src 'self' wss: https:; img-src 'self' data: blob:; " +
    "media-src 'self' blob:; script-src 'self'; style-src 'self' 'unsafe-inline'; " +
    "font-src 'self'; manifest-src 'self' blob:; frame-ancestors 'none'; base-uri 'self'; " +
    "form-action 'self'";

export const ENTETES_DOCUMENT: Readonly<Record<string, string>> = Object.freeze({
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': 'no-store',
    'Content-Security-Policy': CSP,
    'Referrer-Policy': 'no-referrer',
    'X-Frame-Options': 'DENY',
});

/// For the only names that REALLY carry a fingerprint — those of the
/// Vite assets directory (`resolution.ts::REPERTOIRE_ACTIFS`).
///
/// 🔴 `immutable` IS A PROMISE THAT CANNOT BE TAKEN BACK: a browser
/// that saw it will not ask for the asset again before a year, whatever gets
/// deployed. It only holds if the NAME changes with each content — which
/// the fingerprint guarantees, and which nothing else guarantees.
export const ENTETES_RESSOURCE_EMPREINTEE: Readonly<Record<string, string>> = Object.freeze({
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': 'public, max-age=31536000, immutable',
});

/// For ANY other asset: `hub.webmanifest`, `favicon.ico`, an icon
/// placed at the root — STABLE names whose content changes.
///
/// 🔴 REVALIDATABLE, NEVER A YEAR AND NEVER `no-store`. Both extremes are
/// wrong here: a year makes the asset not revisable (that is the defect being
/// fixed), and `no-store` would forbid even storing it, so would make every visit
/// pay the whole transfer again for a file that, most of the
/// time, has not changed. `max-age=0, must-revalidate` keeps the copy and requires
/// it to be revalidated: a `304` is then enough, and a revision shows up
/// at once.
///
/// ⚠️ IT IS MORE PERMISSIVE THAN NGINX, AND THAT IS ACCEPTED: `location /` of
/// `deploiement/nginx.conf` emits NO `Cache-Control`, which leaves the
/// browser to apply heuristics. Emitting an explicit policy is a
/// tightening, not a loosening — the reverse of the one-year `immutable`, which
/// was a real one.
export const ENTETES_RESSOURCE_REVALIDABLE: Readonly<Record<string, string>> = Object.freeze({
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': 'public, max-age=0, must-revalidate',
});
