// The RULE resolving a URL path to a file of the built page.
// PURE: no `fs`, no `http`, no environment variable. That is what
// makes it testable on the host, WITHOUT A DISK.
//
// 🔴 THIS RULE DOES NOT CARRY « ALL » THE SECURITY OF THE SERVER, AND A REVIEW
// BY EXECUTION ESTABLISHED IT (round 2, Critical 3) — THIS SENTENCE STILL SAID SO
// HERE WHILE IT WAS ALREADY FALSE IN `page/routes-page.ts`.
// This rule closes LEXICAL traversal (`..`, encoded or not) — which
// WAS ENOUGH as long as nothing read the disk. `page/routes-page.ts`, however,
// READS the disk, and a symbolic link dropped in the built root goes through
// this rule without any `..` ever showing up in the URL: the real
// guard against links (`realpath`, on the CANONICAL path) therefore lives
// in that module, not here. What THIS rule guarantees stays true and
// necessary — it is simply no longer ENOUGH on its own.

/// 🔴 CLOSED LIST. An extension missing from here REFUSES.
export const TYPES_MIME: ReadonlyMap<string, string> = new Map([
    ['html', 'text/html; charset=utf-8'],
    ['js', 'text/javascript; charset=utf-8'],
    ['css', 'text/css; charset=utf-8'],
    ['webmanifest', 'application/manifest+json'],
    ['json', 'application/json; charset=utf-8'],
    ['ico', 'image/x-icon'],
    ['png', 'image/png'],
    ['svg', 'image/svg+xml'],
    ['woff2', 'font/woff2'],
]);

// 🔴 `hub.html`, NOT `index.html` — DECISION OF THE REPOSITORY OWNER
// (« Serve the hub at the root », 30 August 2026), after a defect measured IN
// PRODUCTION: `https://app.allanic.me/` returned `index.html`, the SESSION
// page — and with no `?session=` parameter, `client/src/main.ts` then made up
// a `demo` session WITHOUT a token (fixed in the same batch, see
// `client/src/session-id.ts`). The agent logged « handshake
// refused: handshake without a token on the demo session », and the
// owner saw « Session failed » — a failure INDISTINGUISHABLE
// from a real one for whoever just opens the address of the service.
//
// The root and any path without an extension now fall back to the HUB, the
// only surface that opens a session with a real token. `index.html` (the
// session page) IS NOT REMOVED: it is still served at ITS OWN EXPLICIT
// path, `/index.html` — a segment with an extension, so never resolved by
// `PAGE` below (see `resoudre()`). Only the fallback changes target.
const PAGE = 'hub.html';

/// 🔴 THE DIRECTORY WHOSE NAMES VITE FINGERPRINTS **ALL**, AND THE ONLY ONE.
///
/// It is `build.assetsDir`, whose default is `assets` and which `client/
/// vite.config.ts` does not override. Read off the really built page, on
/// 22 August 2026:
///   ls client/dist         -> assets/ + connexion.html design.html hub.html
///                             hub.webmanifest index.html primitives.html
///                             shell.html
///   ls client/dist/assets  -> adresse-plateforme-uutwZeXQ.js,
///                             main-DOC38JmJ.css, hub-B8O-1KAt.js, …
/// **The root carries NO fingerprinted name**; the assets directory carries
/// nothing else. It is that partition, and not a regular expression on the
/// shape of a name, that decides the cache — a name can be disguised, a location
/// cannot.
export const REPERTOIRE_ACTIFS = 'assets';

export type Resolution =
    | {
          readonly ok: true;
          readonly fichier: string;
          readonly mime: string;
          readonly document: boolean;
          /// 🔴 DOES THE NAME REALLY CARRY A FINGERPRINT? That is the question
          /// the first draft did NOT ask: it classified by
          /// EXTENSION, and thus gave a year of `immutable` to `hub.webmanifest`
          /// and `favicon.ico` — names Vite NEVER fingerprints. MEASURED
          /// on the real `client/dist`: `/hub.webmanifest` returned
          /// `public, max-age=31536000, immutable`, which makes **the hub PWA
          /// manifest not revisable for a year** in any browser
          /// that saw it. Under nginx, `location /` emits NO
          /// `Cache-Control`: it was a regression that only the Pomerium
          /// deployment introduced.
          ///
          /// ⚠️ THE DISTINCTION LIVES HERE, IN THE PURE RULE, AND NOT IN THE
          /// SERVER: that is where the classification already lives, and that is what
          /// makes it testable WITHOUT A DISK.
          readonly empreinte: boolean;
      }
    | {
          readonly ok: false;
          readonly motif:
              | 'chemin-invalide'
              | 'octet-nul'
              | 'traversee'
              | 'extension-inconnue'
              | 'nom-vide';
      };

/// Normalises a path into segments, refusing any climb that GOES OUT.
///
/// ⚠️ THE COUNT IS MADE ON THE RESULT, NEVER ON THE PRESENCE OF `..`: that is
/// what makes `/assets/../index.html` legitimate and `/assets/../../x` refused,
/// where a substring filter would refuse both or accept both.
function normaliser(brut: string): string[] | undefined {
    const sortie: string[] = [];
    for (const segment of brut.split('/')) {
        if (segment === '' || segment === '.') continue;
        // A backslash is not a separator under Linux, but a path that
        // carries one comes from no page built by Vite: refusing it costs
        // nothing and closes the Windows variant of the traversal.
        if (segment === '..' || segment.includes('\\')) {
            if (segment === '..' && sortie.length > 0) {
                sortie.pop();
                continue;
            }
            return undefined;
        }
        sortie.push(segment);
    }
    return sortie;
}

export function resoudre(cheminUrl: string): Resolution {
    let decode: string;
    try {
        decode = decodeURIComponent(cheminUrl);
    } catch {
        // `decodeURIComponent` THROWS on a malformed `%`. A server that
        // let that exception through would return a 500 where a refusal
        // is enough — and the server `catch` would log a « failing
        // route » for a request that is merely badly written.
        return { ok: false, motif: 'chemin-invalide' };
    }
    if (decode.includes('\0')) return { ok: false, motif: 'octet-nul' };

    const segments = normaliser(decode);
    if (segments === undefined) return { ok: false, motif: 'traversee' };

    const dernier = segments[segments.length - 1];
    // Root, or path without an extension: the SPA fallback — the equivalent of
    // nginx's `try_files … /index.html`, except that the TARGET, here, is
    // `hub.html` since 30 August 2026 (see `PAGE` above).
    const fichier = dernier === undefined || !dernier.includes('.') ? PAGE : segments.join('/');

    const point = fichier.lastIndexOf('.');
    const extension = fichier.slice(point + 1).toLowerCase();
    const mime = TYPES_MIME.get(extension);
    if (mime === undefined) return { ok: false, motif: 'extension-inconnue' };

    // 🔴 A NAME REDUCED TO A BARE EXTENSION (`/.json`, `/assets/.webmanifest`…)
    // HAS AN EMPTY NAME BEFORE THE EXTENSION — THIS IS NOT A LEGITIMATE CASE, IT IS A
    // HOLE IN THE RULE. An earlier review had judged it NOT EXPLOITABLE
    // because THIS MODULE reads nothing itself; but the one that applies it
    // (`page/routes-page.ts`) READS THE DISK, and would serve as is a file
    // literally named `.json` if it existed at the built root. The guard
    // is set HERE, AFTER the MIME list and never before: `/.env` and
    // `/.htaccess` are already refused by `extension-inconnue` (their
    // « extension » is not in it), and that verdict stays unchanged — moving
    // it would have lied about the reason for THEIR refusal. What this line
    // closes is the CLASS that the MIME list lets through by accident:
    // any KNOWN extension (`.json` is one of them) carried by
    // an empty name. Closing the class avoids depending case by case on a
    // list that was not written to settle this question.
    if (dernier !== undefined && dernier.lastIndexOf('.') === 0) {
        return { ok: false, motif: 'nom-vide' };
    }

    return {
        ok: true,
        fichier,
        mime,
        document: extension === 'html',
        // ⚠️ THE PREFIX CARRIES THE SEPARATOR: without it, a file named
        // `assetsX.js` placed at the root would pass for a fingerprinted asset.
        // `fichier` is already NORMALISED (segments put back together, no surviving
        // climb), so this test really applies to the first segment.
        empreinte: fichier.startsWith(`${REPERTOIRE_ACTIFS}/`),
    };
}
