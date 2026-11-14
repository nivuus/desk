// Where the client gets the platform's address from, and why it is no longer a
// literal.
//
// 🔴 THIS MODULE EXISTS FOR A FAILURE THAT P5's DEPLOYMENT WOULD HAVE MADE
// CERTAIN. Three files carried the hardcoded address:
//
//     connexion.ts   `http://${window.location.hostname}:8080`
//     main.ts        `ws://${window.location.hostname}:8080`
//     shell-page.ts  `ws://${window.location.hostname}:8080`
//
// A page served over **`https://`** by the proxy that opens `ws://…:8080` is
// **MIXED CONTENT**: the browser refuses the connection. And it refuses it
// silently for anything that is not the console — the page loads, the token
// is accepted, and the media never establishes. **No Node test can
// see it**; `cors.ts` already writes this sentence for its own subject. The
// deployment would therefore have shipped non-functional AND GREEN.
//
// Port 8080 was wrong for the same reason: the proxy serves on 443, and nothing
// listens on 8080 from outside.
//
// 🔴 PURE AND WITHOUT DOM, on the precedent of `resize.ts`, `prefixe.ts` and
// `jeton.ts`: `client/` has no `vitest.config.*`, so the test
// environment is the default Node — there is neither `window` nor `location`. What
// is needed from `location` is a PARAMETER, and that is what makes `https:` testable
// without a browser.
//
// ⚠️ TWO FUNCTIONS AND NOT ONE, although they only differ by scheme:
// their callers do not have the same escape parameter (`?plateforme=` is
// an http URL, `?signaling=` a ws URL), and a single function returning
// both would not know what to do with an explicit one covering only one.

/// What this module needs from `location`, and nothing more.
///
/// ⚠️ `host` AND NOT `hostname`: `host` carries THE PAGE'S PORT when it is
/// not the scheme's. That is precisely what the previous code threw away to
/// replace it with 8080.
export interface Emplacement {
    protocol: string;
    host: string;
}

/// An explicit address this build does not honour. Thrown, never replaced by
/// the default: the caller shows the message, so that a refused parameter is
/// visible instead of silently turning into another server.
export class RefusedAddressError extends Error {
    constructor(parameter: string, value: string, reason: string) {
        super(`the "${parameter}" parameter is refused (${reason}): ${value}`);
        this.name = 'RefusedAddressError';
    }
}

/// The scheme an explicit address must carry to stay on the page's own
/// origin: the page's scheme for HTTP, its WebSocket twin for signalling.
function expectedScheme(emplacement: Emplacement, family: 'http' | 'ws'): string {
    const secure = emplacement.protocol === 'https:';
    if (family === 'http') return secure ? 'https:' : 'http:';
    return secure ? 'wss:' : 'ws:';
}

/// Checks an explicit address against the page's origin and returns it AS IS.
///
/// 🔴 THE BEARER TOKEN AND THE SESSION GO TO THIS ADDRESS, so in a production
/// build it must designate the page's own origin: same `host` (hence same
/// port, the URL parser drops a default port on both sides) and the scheme of
/// the matching class (`https:`/`wss:` for an `https:` page, `http:`/`ws:`
/// otherwise). `foreignAllowed` lifts the check; it is true only in a
/// bench build (`address-policy.ts`), never in `npm run build`.
function checkExplicitAddress(
    emplacement: Emplacement,
    parameter: string,
    explicite: string,
    family: 'http' | 'ws',
    foreignAllowed: boolean,
): string {
    if (foreignAllowed) return explicite;
    let target: URL;
    try {
        target = new URL(explicite);
    } catch {
        throw new RefusedAddressError(parameter, explicite, 'not an absolute address');
    }
    const scheme = expectedScheme(emplacement, family);
    if (target.protocol !== scheme) {
        throw new RefusedAddressError(parameter, explicite, `this page only accepts ${scheme} here`);
    }
    if (target.host !== emplacement.host) {
        throw new RefusedAddressError(parameter, explicite, "it is not this page's origin");
    }
    return explicite;
}

/// The platform's HTTP address: `?plateforme=` if set, otherwise
/// the page's origin.
///
/// `foreignAllowed` says whether an explicit value may name another
/// origin; callers pass `FOREIGN_ADDRESSES_ALLOWED`.
export function adressePlateforme(
    emplacement: Emplacement,
    explicite: string | null | undefined,
    foreignAllowed: boolean,
): string {
    if (explicite) {
        return checkExplicitAddress(emplacement, 'plateforme', explicite, 'http', foreignAllowed);
    }
    return `${emplacement.protocol === 'https:' ? 'https' : 'http'}://${emplacement.host}`;
}

/// The relay's path on the service. See `plateforme/src/http/serveur.ts`.
const CHEMIN_SIGNAL = '/signal';

/// The signaling WebSocket address: `?signaling=` if set, otherwise
/// the page's origin, **with the scheme that matches its own**.
///
/// 🔴 IT IS THE SCHEME MATCH THAT COUNTS: `https:` ⇒ `wss:`, and
/// never `ws:`. A `ws:` from an `https:` page is mixed content, and the
/// browser refuses it.
export function adresseSignaling(
    emplacement: Emplacement,
    explicite: string | null | undefined,
    foreignAllowed: boolean,
): string {
    // ⚠️ AN EXPLICIT VALUE STAYS EXPLICIT, AND DOES NOT GET THE SUFFIX. It is the
    // contract of this parameter since P5: it replaces the WHOLE address, not
    // its authority. Adding `/signal` to it would give `/signal/signal` for whoever had
    // already written it, and nobody would know which of the two behaviours is the
    // right one. CONSEQUENCE TO KNOW: an acceptance run that sets `?signaling=` must
    // now write the path.
    //
    // 🔴 THIS COMMENT CLAIMED "NONE SETS ONE", AND IT WAS WRONG — the
    // SCOPE of its `grep` did not cover what the sentence claimed to cover.
    // It searched `client/*.mjs client/recette/*.mjs scripts/*.sh`, where there
    // is indeed nothing; **but ALL of this repository's acceptance drivers
    // live in `docs/superpowers/plans/journaux-*/`**, which the command
    // did not reach. It is the "487 wreck" pattern of `docs/claude/pitfalls-docs-size-vm.md`:
    // a completeness claim whose command did not sweep its own
    // scope.
    //
    // 🔴 THE CORRECT READING, August 21st, 2026, on the files TRACKED BY GIT:
    //
    //     git grep -n 'signaling=' -- 'docs/superpowers/plans/journaux-*'
    //
    // **ELEVEN files under `journaux-*/instrument/` set `?signaling=`** —
    // the drivers of `accent-a1`, `micro-e3` (the driver and its
    // `injection-e3.js`), `pont-fichiers` f1 to f5, and `presse-papier` p1 to p3 —, (policy: allow-fr, archived plan names)
    // **plus a TWELFTH outside that directory**,
    // `journaux-micro-e2/pilote-recette-e2.mjs`. All twelve pass a URL
    // **without a path** (`ws://192.168.3.1:8080`, `ws://<host>:8090`, or the
    // platform's URL with `http` -> `ws`), so they target the root `/` — **which this
    // workstream has just closed**.
    //
    // ⚠️ THEY ALL THEREFORE NEED REPAIRING, AND NONE WAS REPAIRED HERE: it is a
    // separate workstream, listed in the "Open legacies" of `docs/claude/leftovers-auth-pomerium-decisions.md`. ⚠️ The
    // `grep` above also catches the output `.log` and `.json` files, which are
    // READINGS and not drivers: the selection rule is "driver
    // sources", not "any occurrence".
    //
    // 🔴 THOSE DRIVERS NAME ANOTHER ORIGIN THAN THE PAGE (a platform on
    // another port or host), and a production build now REFUSES that: the
    // explicit value is honoured only when it designates the page's own origin
    // (`checkExplicitAddress`). A bench that needs another origin runs the dev
    // server or a build made with `vite build --mode banc`, where
    // `FOREIGN_ADDRESSES_ALLOWED` (`address-policy.ts`) is true.
    if (explicite) {
        return checkExplicitAddress(emplacement, 'signaling', explicite, 'ws', foreignAllowed);
    }
    const schema = emplacement.protocol === 'https:' ? 'wss' : 'ws';
    return `${schema}://${emplacement.host}${CHEMIN_SIGNAL}`;
}

// ⚠️ THE SCHEME TEST IS AN EQUALITY TO `'https:'`, NOT AN ABSENCE OF
// `'http:'`. The difference shows on `file:`, which occurs when opening the
// HTML from disk: equality makes it fall back to plain — useless,
// but readable —, whereas a negation would have produced `wss://` on a local
// page, whose error message would have pointed to no cause.
//
// ⚠️ AN EMPTY PARAMETER IS NOT A PARAMETER. `URLSearchParams.get` returns `''`
// on `?signaling=` and `null` on an absent parameter; the `if (explicite)`
// rules out both. Treating `''` as explicit would produce an empty address,
// hence a failure without a message.
