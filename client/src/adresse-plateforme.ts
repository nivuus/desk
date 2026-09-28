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

/// The platform's HTTP address: `?plateforme=` if set, otherwise
/// the page's origin.
export function adressePlateforme(
    emplacement: Emplacement,
    explicite?: string | null,
): string {
    if (explicite) return explicite;
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
    explicite?: string | null,
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
    // did not reach. It is the "487 wreck" pattern of `CLAUDE.md`:
    // a completeness claim whose command did not sweep its own
    // scope.
    //
    // 🔴 THE CORRECT READING, August 21st, 2026, on the files TRACKED BY GIT:
    //
    //     git grep -n 'signaling=' -- 'docs/superpowers/plans/journaux-*'
    //
    // **ELEVEN files under `journaux-*/instrument/` set `?signaling=`** —
    // the drivers of `accent-a1`, `micro-e3` (the driver and its
    // `injection-e3.js`), `pont-fichiers` f1 to f5, and `presse-papier` p1 to p3 —,
    // **plus a TWELFTH outside that directory**,
    // `journaux-micro-e2/pilote-recette-e2.mjs`. All twelve pass a URL
    // **without a path** (`ws://192.168.3.1:8080`, `ws://<host>:8090`, or the
    // platform's URL with `http` -> `ws`), so they target the root `/` — **which this
    // workstream has just closed**.
    //
    // ⚠️ THEY ALL THEREFORE NEED REPAIRING, AND NONE WAS REPAIRED HERE: it is a
    // separate workstream, listed in the "Open legacies" of `CLAUDE.md`. ⚠️ The
    // `grep` above also catches the output `.log` and `.json` files, which are
    // READINGS and not drivers: the selection rule is "driver
    // sources", not "any occurrence".
    if (explicite) return explicite;
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
