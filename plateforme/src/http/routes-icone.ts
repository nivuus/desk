// The two icon routes: `PUT /icone/:sha256` (the AGENT uploads, under an agent
// token) and `GET /application/:id/icone?e=&v=&x=&s=` (the BROWSER reads,
// through a SIGNED URL, WITHOUT any header — decision of the repository owner
// of 30 August 2026, see below).
//
// 🔴 WHY THE FINGERPRINT GOES INTO THE URL OF THE `GET`. The specification writes
// « `GET /application/:id/icone` with an immutable `Cache-Control` keyed on
// the fingerprint » — **and the two halves contradict each other as they stand**. On
// a URL that DOES NOT CARRY the fingerprint, `immutable` is a LIE: the day
// the icon changes, all caches serve the old one, FOR A YEAR. The
// `?e=` parameter makes the URL truly content-addressed, and
// `Cache-Control: private, max-age=31536000, immutable` TELLS THE TRUTH.
//
// 🔴 AND THE `404` ON A STALE `e` IS NOT A CONVENIENCE: without it, an
// old URL would serve the CURRENT icon under an immutable header, which
// would poison the cache for a year with an image that is not the one
// the URL names.
//
// ⚠️ `private`, NEVER `public`: the response is authenticated by the bearer,
// and a shared cache has no business with an icon served under a token.
//
// 🔴 ✅ **SETTLED ON 30 AUGUST 2026 BY THE REPOSITORY OWNER — AND WHAT FOLLOWS
// IS NO LONGER THE STATE OF THE PRODUCT.** The whole struck-out paragraph below describes
// the legacy this batch closes; it is kept because it carries the diagnosis,
// and because this repository strikes out rather than erases.
//
// ❌ ~~A CONSEQUENCE NAMED HERE RATHER THAN DISCOVERED LATER, AND IT
// BELONGS TO SUB-BLOCK G5: an `<img src>` DOES NOT CARRY AN
// `Authorization` HEADER. A page that displayed these icons will have to fetch them through
// `fetch()` then `URL.createObjectURL`, and a PWA manifest WILL NOT BE ABLE TO
// point to this route as it stands. G2 does not settle it: settling it
// would require deciding whether an icon can be served without a token, which is
// a security decision.~~
//
// ✅ **THE DECISION, AND ITS TWO DISCARDED BRANCHES** (30 August 2026). The
// owner kept **THE SIGNED URL**, and discarded by name:
//   ① serving the icons WITHOUT a token — that would reveal the list of
//      applications installed on the VM to anyone who reaches the port;
//   ② inlining them as `data:` in the catalogue — uneven support for `data:`
//      in a PWA manifest.
// **This is not an implementation convenience**: it is a security
// trade-off, and that is why G5 had left it to the owner.
//
// 🔴 **THE OLD `Authorization` PATH DOES NOT SURVIVE, AND THAT IS SETTLED.**
// Two paths for the same resource are two authorisation guards to
// keep — and this repository has written ten times that two copies of a
// security guard drift silently, « the one being fixed and the one being
// forgotten » (`agents/canal.ts`, `porteur-agent.ts`). The `GET` therefore only
// accepts the signed URL, and **no authority is lost**: the URL is
// only minted by `routes-applications.ts::servirApplications`, which requires the
// bearer token AND checks the ownership of the VM before returning it.
//
// ⚠️ **WHAT THE CHANGE COSTS, SAID RATHER THAN KEPT QUIET.**
//   ① **The freshness of the authorisation.** The ownership of the VM is
//      checked at MINTING, no longer at reading: a reassigned VM leaves
//      the URLs already minted valid until they expire (5 to 6 min).
//      It is the price of a bearer capability, and it is what the short
//      lifetime bounds.
//   ② **The response no longer identifies anybody.** It cannot: an
//      `<img src>` carries nothing but its URL. What a leaked URL
//      gives is **an icon, that of one application, for five
//      minutes** — never the catalogue, never a launch, never a
//      session.
//
// ⚠️ **WHAT THIS BATCH DOES NOT ESTABLISH, AND IT MUST BE READ BEFORE
// INVOKING IT.** Behind Pomerium, an `<img src>` placed by an
// authenticated page travels with the session cookies (same-origin
// sub-resource) and gets through. **A MANIFEST `icons[].src`, however, was fetched
// WITHOUT the cookies** — that is what `client/src/hub/manifeste-hub-greffon.ts`
// measured on `<link rel="manifest">`, and the same mechanism holds for the
// icons a manifest names. A signed URL placed in a manifest
// would therefore require the proxy policy to open it: it ends neither with
// `.png`, nor with `.ico`, nor with `manifest.json`, the only three suffixes that
// `config.yaml` lets through. **This batch DOES NOT TOUCH Pomerium**, and that is
// why the per-application manifest keeps carrying its icon as
// `data:` (`client/src/hub/manifeste.ts`), a form G5 measured as
// installable.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import type { Magasin } from '../apps/icones';
import { empreinteValide } from '../apps/icones';
import { lireParId } from '../depot/application';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';
import { lirePorteurAgent } from './porteur-agent';
import { verifyIconUrl } from '../apps/url-icone';

export interface DependancesIcone {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    magasin: Magasin;
    maintenant: () => number;
}

/// ⚠️ **AN UPPER BOUND BY EYE, AND NOT CALIBRATED.** The largest icon measured weighs
/// under 30 KiB on average and the whole corpus 4.4 MB for 153 — **but
/// NO individual size was recorded**, only an average. Saying
/// so beats presenting it as settled.
///
/// ⚠️ IT HAS NOTHING TO DO WITH THE BODY CEILING OF `routes-auth.ts`
/// (4 KiB), WHICH MUST NOT BE RAISED: a route that accepts images has
/// its own ceiling, and mixing the two up would open the body of the
/// authentication routes to a megabyte.
export const ICONE_MAX_OCTETS = 1_048_576;

const CHEMIN_PUT = '/icone/';

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ UNCONDITIONAL, and set on EVERY response — including the
        // refusals. Spread BEFORE `cors`, whose policy is optional and must
        // never be able to overwrite them.
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(corps === undefined ? undefined : JSON.stringify(corps));
}

/// Matches `/icone/:sha256`, and NOTHING else.
///
/// 🔴 SPLIT BY SEGMENTS, NEVER BY `startsWith`. G1 MEASURED that a
/// `startsWith('/application')` left its seventeen tests GREEN: the route
/// swallowed the whole family and returned ITS OWN typed 404, indistinguishable from the
/// generic 404 as long as one only read the status. The pattern is anchored at
/// BOTH ends.
function empreinteDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'icone', '<sha256>'] — exactement trois.
    if (segments.length !== 3) return undefined;
    if (segments[1] !== 'icone') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}

/// Matches `/application/:id/icone`, and NOTHING else.
function iconeDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'application', '<id>', 'icone'] — exactement quatre.
    if (segments.length !== 4) return undefined;
    if (segments[1] !== 'application' || segments[3] !== 'icone') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}

/// Reads the body, stopping AS SOON AS it overflows.
///
/// 🔴 THE CEILING IS CHECKED DURING THE READ, NOT AFTER: piling up
/// first and measuring afterwards would let a peer fill the memory of the service
/// before the refusal arrives.
async function lireCorps(req: IncomingMessage): Promise<Buffer | 'trop-gros'> {
    const morceaux: Buffer[] = [];
    let total = 0;
    for await (const morceau of req) {
        const b = morceau as Buffer;
        total += b.length;
        if (total > ICONE_MAX_OCTETS) return 'trop-gros';
        morceaux.push(b);
    }
    return Buffer.concat(morceaux);
}

export async function servirIcone(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesIcone,
): Promise<boolean> {
    const chemin = (req.url ?? '').split('?')[0];
    const empreintePut = empreinteDe(chemin);
    const idApplication = iconeDe(chemin);
    if (empreintePut === undefined && idApplication === undefined) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    // ⚠️ THE `PUT` REQUIRES `Authorization`, SO THE REQUEST IS NOT SIMPLE: the
    // browser first sends an `OPTIONS`, and **gives up without ever
    // sending the real request** if the response does not suit it. That is the
    // exact defect that the browser corroboration of sub-block P4 found,
    // and that no Node test could see.
    //
    // ⚠️ THE `GET`, FOR ITS PART, NO LONGER NEEDS IT SINCE THE SIGNED URL — it carries
    // no header, so an `<img src>` emits no preflight request.
    // **The preflight response is still served for both**, and that is not
    // negligence: a bearer `fetch` is still possible on the page side (that is
    // what `client/src/hub/catalogue.ts::lireIcone` does to read the bytes
    // of the PNG), and it does trigger an `OPTIONS` as soon as it comes from another
    // origin — the `PLATEFORME_ORIGINE_CLIENT` deployment.
    if (req.method === 'OPTIONS') {
        repondre(rep, 204, undefined, cors);
        return true;
    }

    if (empreintePut !== undefined) {
        if (req.method !== 'PUT') {
            repondre(rep, 405, { refus: 'methode' }, cors);
            return true;
        }
        return depot(req, rep, deps, cors, empreintePut);
    }

    if (req.method !== 'GET') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }
    return service(req, rep, deps, cors, idApplication!);
}

/// `PUT /icone/:sha256` — the agent uploads.
async function depot(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesIcone,
    cors: Record<string, string> | undefined,
    empreinte: string,
): Promise<boolean> {
    // 🔴 AN AGENT TOKEN, NOT A HUMAN TOKEN, and the refusal is the
    // MIRROR of that of `porteur.ts`: `403` and not `401`, because the
    // token is VALID — it simply is not an agent one. A `401`
    // would invite signing in again for nothing.
    //
    // 🔴 THIS READING LIVED HERE INLINE, COPIED FROM `porteur.ts`, AND IT
    // MOVED INTO `porteur-agent.ts` (G3). The reason is not
    // aesthetics: `GET /televersement/:id/contenu` needs the SAME
    // reading, and two copies of a security guard drift silently —
    // the one being fixed and the one being forgotten. The module moreover yields the
    // SESSION PREFIX, which this route has no use for and which the other one
    // will resolve into a VM through `depot/agent.ts::lireParPrefixe`.
    //
    // ⚠️ THE FOUR REASONS AND THEIR CODES ARE UNCHANGED, to the letter:
    // `jeton-absent` 401, `jeton-invalide` 401, `jeton-expire` 401,
    // `jeton-utilisateur` 403, and in that order. The ONLY behaviour (policy: allow-fr - frozen wire key or SQLite column)
    // difference is a REPEATED `Authorization` header, which the copy filed
    // with `jeton-absent` and which the module refuses as `jeton-invalide` — an
    // ambiguous request is not an empty request. **It is UNREACHABLE
    // from a real HTTP request, and that is MEASURED**: on Node v24.9.0, two
    // `Authorization` headers yield `typeof req.headers.authorization ===
    // 'string'`, the parser keeping the first and dropping the second.
    const porteur = lirePorteurAgent(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }

    // 🔴 THE SHAPE GUARD COMES BEFORE ANY BODY READ. `:sha256` is
    // a PATH COMPONENT SUPPLIED BY THE NETWORK, and `..` is meaningful in it.
    if (!empreinteValide(empreinte)) {
        repondre(rep, 400, { refus: 'empreinte-invalide' }, cors);
        return true;
    }

    const corps = await lireCorps(req);
    if (corps === 'trop-gros') {
        // ⚠️ TYPED AND LOGGED, NEVER SILENT: the application will enter the
        // catalogue WITHOUT an icon, and one must be able to know it.
        console.warn(`icon refused, body beyond ${ICONE_MAX_OCTETS} bytes: ${empreinte}`);
        repondre(rep, 413, { refus: 'taille' }, cors);
        return true;
    }

    try {
        // 🔴 THE STORE RECOMPUTES THE FINGERPRINT. It is the third of the three
        // checks — « no hop trusts the previous one ». Without
        // it, content addressing would not be content addressing.
        deps.magasin.write(empreinte, corps);
    } catch (cause) {
        repondre(rep, 400, { refus: 'empreinte' }, cors);
        return true;
    }
    repondre(rep, 204, undefined, cors);
    return true;
}

/// `GET /application/:id/icone?e=&v=&x=&s=` — the BROWSER reads, through a
/// SIGNED URL, with no header.
///
/// 🔴 THE ORDER OF THE CHECKS IS THAT OF `routes-applications.ts`: THE SHAPE,
/// THEN THE SIGNATURE, THEN ONLY THE DATABASE. « A route that read the
/// catalogue and then refused the token would leak nothing through its response, but
/// it would offer free work to an anonymous peer » — it is to keep
/// that rule that the VM travels in the URL (`v=`) rather than being read from the
/// database to check the signature.
async function service(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesIcone,
    cors: Record<string, string> | undefined,
    idApplication: string,
): Promise<boolean> {
    const url = new URL(req.url ?? '', 'http://interne');
    const attendue = url.searchParams.get('e');
    if (attendue === null || attendue === '') {
        repondre(rep, 400, { refus: 'empreinte-absente' }, cors);
        return true;
    }

    // 🔴 THE SIGNATURE IS CHECKED IN CONSTANT TIME, AND THE EXPIRY AGAINST
    // THE SERVER CLOCK — never by trusting a client field. Both
    // rules live in `apps/url-icone.ts`, with their reasons; this
    // route only translates the verdict into an HTTP response.
    const verdict = verifyIconUrl(
        idApplication,
        url.searchParams,
        deps.secretJeton,
        deps.maintenant(),
    );
    if (!verdict.ok) {
        // ⚠️ `400` FOR AN INCOMPLETE SHAPE, `403` FOR A REFUSAL: a URL
        // missing a parameter is not a refused URL, it is
        // a URL nobody has finished writing — and saying so avoids looking for
        // an authorisation where a piece is missing.
        if (verdict.motif === 'parametre-absent') {
            repondre(rep, 400, { refus: 'signature-absente' }, cors);
            return true;
        }
        // 🔴 `url-expiree` IS TOLD APART FROM `signature-invalide`, AND IT IS
        // NOT AN ORACLE. Both reasons already assume the application
        // id is known, and neither says anything about the existence or
        // the ownership of a VM — what making the `404` uniform protects.
        // What they tell apart is ACTIONABLE on the client side: an expired URL
        // is repaired by reading the catalogue again, a false signature never.
        // ⚠️ THE REVERSE WOULD BE AN ORACLE, and `url-icone.ts` guards against it:
        // a forged AND stale URL is told « signature », never
        // « expired » — without which the refusal would inform a forger about the
        // half of their work that succeeded.
        repondre(rep, 403, { refus: verdict.motif }, cors);
        return true;
    }

    const application = await lireParId(deps.base, idApplication);
    if (application === undefined) {
        repondre(rep, 404, { refus: 'application-inconnue' }, cors);
        return true;
    }

    // 🔴 THE SIGNED VM IS CHECKED AGAIN AGAINST THE ONE IN THE DATABASE. Without that,
    // covering it with the signature would be an ornament: a signature that does not
    // check what it claims to authorise authorises nothing. It is this
    // check that makes an application repointed to ANOTHER VM stop
    // being served by the URLs already minted.
    //
    // ⚠️ THE REFUSAL IS THAT OF G1, WORD FOR WORD — `404 vm-inconnue`: the
    // same code and the same reason as the catalogue route, so as not to
    // reopen through a back door the enumeration oracle the
    // owner removed.
    if (application.vm_id !== verdict.vm) {
        repondre(rep, 404, { refus: 'vm-inconnue' }, cors);
        return true;
    }

    // 🔴 AN `e` THAT DOES NOT MATCH THE CURRENT FINGERPRINT RETURNS `404`, and
    // that is what makes `immutable` honest: without that refusal, an old URL
    // would serve the CURRENT icon under an immutable header, poisoning the
    // cache for a year with an image that is not the one the URL names.
    //
    // ⚠️ THE FINGERPRINT IS NOT SIGNED, AND THAT IS INTENDED: it is a VERSION,
    // not an authorisation. See `apps/url-icone.ts::PorteeIcone`.
    if (application.icone === null || application.icone !== attendue) {
        repondre(rep, 404, { refus: 'icone-inconnue' }, cors);
        return true;
    }

    const octets = deps.magasin.lire(attendue);
    if (octets === undefined) {
        // The database knows the fingerprint, the disk does not have it yet: that is
        // the NORMAL state between the announcement and the upload, and it is also
        // that of a lost store. Both repair themselves.
        repondre(rep, 404, { refus: 'icone-inconnue' }, cors);
        return true;
    }

    // 🔴 THE ORDER IS THE REVERSE OF THAT OF `repondre`, AND IT IS A DECLARED
    // EXCEPTION, THE ONLY ONE IN THE SERVICE.
    //
    // `ENTETES_SECURITE` carries `Cache-Control: no-store`, set for the
    // JSON responses — among them `/auth/*`, which yields tokens. This response
    // is not JSON: it is a CONTENT-ADDRESSED image, whose URL
    // carries the fingerprint, and caching it is the whole point of the route.
    // The two headers are therefore spread FIRST and `cache-control` is
    // overwritten AFTERWARDS, deliberately.
    //
    // ⚠️ WHAT IS NOT OVERWRITTEN IS `X-Content-Type-Options: nosniff`, and
    // it is the only one of the two that is a security guard: without it, a
    // browser could guess a type other than `image/png` on
    // bytes a peer uploaded. A test asserts it by name on the 200
    // response, so that this exception cannot widen silently.
    rep.writeHead(200, {
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
        'content-type': 'image/png',
        'content-length': String(octets.length),
        // ⚠️ `private` AND NOT `public`, AND THE REASON CHANGED ON 30 AUGUST
        // 2026 WITHOUT THE VALUE CHANGING: the response is no longer authenticated
        // by a bearer but by a SIGNED URL, that is, by a
        // capability its URL carries entirely. A SHARED cache would
        // therefore serve it to anyone who replays that URL — which is true
        // anyway, but a shared cache would keep it **after
        // expiry**, that is, beyond the bound that gives the mechanism
        // its meaning. `private` confines it to the browser that asked for it.
        //
        // ⚠️ `immutable` TELLS THE TRUTH because the URL carries the fingerprint — and it does not
        // fill the cache with dead entries because the expiry is
        // ROUNDED: two mints within the same minute yield the SAME URL. Without
        // that rounding, the cache key would have changed on each read of the
        // catalogue. See `apps/url-icone.ts::PAS_URL_ICONE_MS`.
        //
        // 🔴 THE CASE OF THIS KEY IS NOT FREE: it must be THAT OF
        // `ENTETES_SECURITE`, to the letter. A JavaScript object tells
        // `Cache-Control` from `cache-control`, and `writeHead` THEN emits
        // BOTH — the client reads `no-store, private, max-age=…`, that is,
        // a response that calls itself both non-storable and immutable. Found
        // by execution, not by rereading.
        'Cache-Control': 'private, max-age=31536000, immutable',
    });
    rep.end(octets);
    return true;
}
