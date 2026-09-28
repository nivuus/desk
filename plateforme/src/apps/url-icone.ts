// The SIGNED URL of an application icon. PURE module: no database, no DOM,
// no clock read here — `maintenant` is a PARAMETER, as in
// `identite/jeton.ts` and `depot/session.ts`. That is what makes expiry
// testable on three distinct instants instead of being inert.
//
// 🔴 DECISION OF THE REPOSITORY OWNER, TAKEN ON 30 AUGUST 2026 — IT IS NOT
// AN IMPLEMENTATION CONVENIENCE. The legacy open since sub-block G5
// said: "an `<img src>` carries no `Authorization` header", so
// `GET /application/:id/icone` was unreachable by an image tag, and
// the hub not installable for lack of an icon the browser could load. G5
// explicitly left the decision to the owner, because it is a
// SECURITY decision and not a writing choice. He settled it: **signed
// URL**, and he explicitly RULED OUT the two other paths —
//   ① serving the icons WITHOUT a token: would reveal the list of applications
//      installed on the VM to anyone reaching the port;
//   ② inlining them as `data:` in the catalogue: uneven support of `data:`
//      in a PWA manifest.
//
// 🔴 WHAT THIS MECHANISM IS, STATED WITHOUT EMBELLISHMENT: a BEARER CAPABILITY.
// Whoever holds the URL can read the icon, without identifying, until its
// expiry. That is exactly what is asked of it — an `<img src>` can
// carry nothing other than its URL — and that is what bounds its reach: it is
// valid ONLY for one icon, ONLY for one application, ONLY for one VM, and ONLY
// for `DUREE_URL_ICONE_MS`. It opens neither the catalogue, nor the
// launch, nor the session.

import { createHmac, hkdfSync, timingSafeEqual } from 'node:crypto';

/* ── THE KEY ──────────────────────────────────────────────────────────── */

/// The derivation label. ⚠️ IT IS PART OF THE CONTRACT: changing it
/// invalidates all URLs in flight, which is harmless (they live
/// five minutes) but must be INTENDED.
const ETIQUETTE_DERIVATION = 'nivuus-desk/url-icone/v1';

/// The HKDF salt. ⚠️ IT IS NOT SECRET, and it does not have to be: HKDF-Extract
/// accepts a public salt — it is the input material that carries the entropy.
/// It is FIXED because a random salt per startup would make every URL
/// invalid when the service restarts, which is the exact defect that a
/// random token secret would have produced on sessions
/// (`config.ts`, token secret line).
const SEL_DERIVATION = 'nivuus-desk/sel/url-icone';

/// 🔴 WHY A SUBKEY AND NOT THE TOKEN SECRET ITSELF.
/// The platform token secret already signs the session JWTs AND the
/// agent tokens. Using the SAME key for a third purpose is the
/// classic key reuse mistake: two mechanisms that sign
/// messages of different formats with the same key expose each other on the
/// day one accepts a message the other has produced. Here the threat is
/// concrete and not theoretical — the two formats are close, both in
/// `base64url`, both HMAC-SHA256 — and the countermeasure costs one line.
///
/// The derivation is HKDF-SHA256 (`node:crypto`, no dependency added —
/// the constraint of `base/pilote.test.ts`). The subkey is 32 bytes, the
/// output block size of SHA-256.
///
/// ⚠️ IT IS DERIVED ON EVERY CALL, AND THIS IS DELIBERATE: `hkdfSync` on 32
/// bytes is two HMACs, that is, cheaper than the database read that
/// follows. A memoised cache would keep a key alive in a global state, which the
/// tests would then have to know how to clear.
export function iconSubkey(secretJeton: string): Buffer {
    return Buffer.from(hkdfSync('sha256', secretJeton, SEL_DERIVATION, ETIQUETTE_DERIVATION, 32));
}

/* ── THE DURATION ─────────────────────────────────────────────────────── */

/// **FIVE MINUTES.**
///
/// 🔴 ITS REASON, WRITTEN DOWN RATHER THAN ASSUMED — this repository carries a whole legacy
/// about uncalibrated constants, and adding one silently makes it worse.
///
///   ① **The floor.** A hub page loads forty icons at once;
///      all the URLs are minted within the second following the read of the
///      catalogue. Five minutes also let a tab survive that stayed
///      open a few minutes before the user looks at it, and an
///      image reloaded by the browser after coming back from the background.
///
///   ② **The ceiling, and it is THAT which sets the value.** The URL is minted
///      WITH the bearer token, by the catalogue, which is authenticated. It must
///      therefore never outlive the token that gave birth to it:
///      `DUREE_JETON_ACCES_MS` is 600,000 ms (`identite/jeton.ts`), and
///      300,000 is a clean half of it. A URL that lived longer
///      than the token would be a capability that OUTLIVES the session — exactly
///      what a short duration must prevent.
///
/// ⚠️ **IT IS NOT CALIBRATED FOR ALL THAT**: no load measurement, no
/// browser reading has judged it. It joins the list of uncalibrated
/// constants of `CLAUDE.md`, and its 1/2 ratio to the access token is a
/// TRADE-OFF, not a result. ⚠️ It and `DUREE_JETON_ACCES_MS` are
/// recalibrated TOGETHER: lowering the token below five minutes would make this
/// bound wrong without any test saying so, and that is why a test
/// pins it by name (`url-icone.test.ts`).
export const DUREE_URL_ICONE_MS = 300_000;

/// **ONE MINUTE** — the STEP to which the expiry is rounded up.
///
/// 🔴 IT EXISTS SO THAT `Cache-Control: immutable` KEEPS A MEANING, AND WITHOUT IT
/// THIS BATCH WOULD HAVE DESTROYED THE CACHE IT CLAIMS TO SERVE. The icon response
/// carries `max-age=31536000, immutable` (`http/routes-icone.ts`): it is
/// content-addressed, and caching it is the whole point of the route.
/// But a SIGNED URL changes on every minting — `x` and `s` are part of it —,
/// so the cache key would change on every read of the catalogue and NO
/// entry would ever be read again. The cache would fill with dead entries.
///
/// By rounding the expiry up to the next step, all the URLs minted
/// within the same minute are IDENTICAL, byte for byte: a page reload
/// within that minute lands on the cache.
///
/// ⚠️ WHAT IT COSTS, SAID RATHER THAN KEPT QUIET: the real lifetime lies
/// between `DUREE_URL_ICONE_MS` and `DUREE_URL_ICONE_MS + PAS_URL_ICONE_MS`,
/// that is, between 5 and 6 minutes. **The FLOOR is guaranteed** — that is the
/// point of rounding UP —, and it is the floor that carried
/// the requirement "forty icons must not expire along the way".
/// The ceiling stays well below the access token (10 min).
///
/// ⚠️ UNCALIBRATED, like its neighbour: no measurement of the cache hit rate
/// has judged it.
export const PAS_URL_ICONE_MS = 60_000;

/* ── WHAT THE SIGNATURE COVERS ────────────────────────────────────────── */

/// The three bound fields, plus the expiry.
///
/// 🔴 EACH ONE IS CHECKABLE ON THE SERVER SIDE, AND THAT IS THE ADMISSION CRITERION:
/// a signature that covered what the server cannot check again is
/// an ornament. The detail, field by field:
///
///   - `application` — the identifier, READ FROM THE PATH of the request. It is
///     what prevents a URL signed for one application from being valid for
///     another.
///   - `vm` — the identifier of the VM. Checked again against `application.vm_id`
///     after the database read. It carries the AUTHORISATION: it is because
///     the ownership of the VM was checked at minting time (catalogue
///     route, bearer token in hand) that the URL is worth anything. Binding
///     it here means that an application repointed to another VM stops
///     being served by the URLs already minted.
///   - `expiration` — in MILLISECONDS, as everywhere in this package (see the
///     divergence declared at the top of `identite/jeton.ts`).
///
/// ⚠️ WHAT THE SIGNATURE DOES NOT COVER, AND WHY: the identity of the
/// USER. It would be unverifiable — the request of an `<img src>`
/// carries nothing that allows checking it. Writing it into the URL without being able
/// to verify it would give the illusion of a link that does not exist.
///
/// ⚠️ THE ICON HASH IS NOT IN THIS STRUCTURE, AND THIS IS INTENDED:
/// it is not an authorisation but a VERSION. The route checks it again
/// against `application.icone`, and a stale `?e=` returns 404 — that is what
/// makes `Cache-Control: immutable` honest (see `routes-icone.ts`). Signing
/// it WOULD BIND the capability to a version, so that an updated icon
/// would invalidate URLs already minted AND already served: two mechanisms for
/// one and the same thing, one of which says nothing more than the other.
export interface PorteeIcone {
    application: string;
    vm: string;
    /// ⚠️ A STRING, AND NOT A NUMBER — see `messageCanonique` just
    /// below: it is the TEXTUAL form that is signed.
    expiration: string;
}

/// The SIGNED string, canonical and UNAMBIGUOUS.
///
/// 🔴 EACH FIELD IS PREFIXED WITH ITS LENGTH, AND IT IS NOT AN AFFECTATION.
/// A plain `a|b|c` is FORGEABLE as soon as a field can contain the
/// separator: `application='x|y'` and `vm='z'` would produce the same string
/// as `application='x'` and `vm='y|z'`, hence the same signature — a URL
/// signed for one pair would be valid for ANOTHER pair. The application
/// identifiers are UUIDs generated by the platform, but the VM one
/// comes from `npm run admin:agent`, hence from a human, hence from any
/// characters at all. The length prefix settles the question for good,
/// whatever the fields of tomorrow.
///
/// ⚠️ `v1` AT THE HEAD: a future version that covered one more field
/// cannot be confused with this one, even with an equal key.
///
/// 🔴 THE EXPIRY IS SIGNED IN ITS EXACT TEXTUAL FORM, the one that travels
/// in the URL — never converted back to a number then reformatted. Without that,
/// `x=010` and `x=10` would become the SAME message, hence the SAME signature:
/// a single minted URL would be worth a whole family, and the shape check
/// on the other end could be bypassed by changing how the number is written.
function messageCanonique(portee: PorteeIcone): string {
    const champ = (v: string): string => `${String(v.length)}:${v}`;
    return ['v1', champ(portee.application), champ(portee.vm), champ(portee.expiration)].join('\n');
}

/* ── FRAPPER ──────────────────────────────────────────────────────────── */

/// The signature alone, in `base64url` (43 characters with SHA-256).
export function signature(portee: PorteeIcone, secretJeton: string): string {
    return createHmac('sha256', iconSubkey(secretJeton))
        .update(messageCanonique(portee), 'utf8')
        .digest('base64url');
}

/// The RELATIVE URL of an icon, ready to put in a `src`.
///
/// 🔴 RELATIVE, AND NEVER ABSOLUTE. The platform does not know the public
/// origin under which a proxy publishes it — `deploiement/nginx.conf` and
/// Pomerium each set one. Making up an origin here would make it
/// diverge from that of the page, and the image would then be either unreachable,
/// or refused by the CSP (`img-src 'self'`). The client resolves it against
/// `location.origin`, which is the only right origin by construction.
///
/// ⚠️ EACH VALUE IS ENCODED: the VM identifier comes from a human.
export function signerUrlIcone(
    application: string,
    vm: string,
    empreinte: string,
    secretJeton: string,
    maintenant: number,
    dureeMs: number = DUREE_URL_ICONE_MS,
): string {
    // 🔴 ROUNDED UP, NEVER DOWN: rounded down, a URL
    // minted just before a step would live a few milliseconds, and the
    // duration floor would no longer be guaranteed. See `PAS_URL_ICONE_MS`.
    const brute = maintenant + dureeMs;
    const expiration = String(Math.ceil(brute / PAS_URL_ICONE_MS) * PAS_URL_ICONE_MS);
    const q = new URLSearchParams({
        e: empreinte,
        v: vm,
        x: expiration,
        s: signature({ application, vm, expiration }, secretJeton),
    });
    return `/application/${encodeURIComponent(application)}/icone?${q.toString()}`;
}

/* ── VERIFY ───────────────────────────────────────────────────────────── */

export type MotifUrlIcone = 'parametre-absent' | 'signature-invalide' | 'url-expiree';

export type VerdictUrlIcone = { ok: true; vm: string } | { ok: false; motif: MotifUrlIcone };

/// Verifies the signature of an icon URL. ALWAYS returns a verdict, never
/// an exception: everything comes from the network, and a throw would answer 500 where
/// it must refuse.
///
/// 🔴 THE ORDER OF THE CHECKS IS THAT OF `verifyToken`, AND IT IS DELIBERATE:
/// the SIGNATURE first, the EXPIRY next. A forged AND stale URL must
/// be told "invalid signature", never "expired" — otherwise the
/// refusal would inform a forger about the half of his work that succeeded.
///
/// 🔴 THE EXPIRY IS JUDGED HERE, AGAINST THE SERVER CLOCK, AND THE `x` FIELD
/// OF THE URL IS BELIEVED ONLY BECAUSE IT IS SIGNED. Reading it without signing it
/// would let the client pick its own expiry date, that is,
/// none.
///
/// 🔴 THE COMPARISON IS CONSTANT-TIME (`timingSafeEqual`), NEVER `===`.
/// A `===` on a string stops at the first differing byte, and the duration of the
/// refusal then tells how many bytes were right — enough to rebuild a
/// signature byte by byte. The LENGTHS are compared first: measured,
/// `timingSafeEqual` THROWS when they differ.
export function verifyIconUrl(
    application: string,
    params: URLSearchParams,
    secretJeton: string,
    maintenant: number,
): VerdictUrlIcone {
    const vm = params.get('v');
    const brutExpiration = params.get('x');
    const recue = params.get('s');
    if (vm === null || vm === '') return { ok: false, motif: 'parametre-absent' };
    if (brutExpiration === null || brutExpiration === '') {
        return { ok: false, motif: 'parametre-absent' };
    }
    if (recue === null || recue === '') return { ok: false, motif: 'parametre-absent' };

    // The signature carries the TEXTUAL form of `x`, as it arrived.
    const attendue = Buffer.from(
        signature({ application, vm, expiration: brutExpiration }, secretJeton),
        'utf8',
    );
    const fournie = Buffer.from(recue, 'utf8');
    if (attendue.length !== fournie.length) return { ok: false, motif: 'signature-invalide' };
    if (!timingSafeEqual(attendue, fournie)) return { ok: false, motif: 'signature-invalide' };

    // 🔴 THIS GUARD COMES AFTER THE SIGNATURE, AND IT IS NOT DECORATIVE:
    // `Number('not-a-number')` returns `NaN`, and `maintenant >= NaN` is FALSE —
    // an unreadable expiry would therefore be ACCEPTED, that is, eternal.
    // ⚠️ IT IS UNREACHABLE BY THE PRODUCT, which only mints integers:
    // the only path that reaches it is a signature computed with the REAL
    // key on a non-integer `x` — and that is exactly how its test
    // makes it go red, rather than leaving it green by construction.
    const expiration = Number(brutExpiration);
    if (!Number.isInteger(expiration)) return { ok: false, motif: 'signature-invalide' };

    // STRICT bound, written so that the test can besiege it from both sides
    // — same gesture as `verifyToken`.
    if (maintenant >= expiration) return { ok: false, motif: 'url-expiree' };

    return { ok: true, vm };
}
