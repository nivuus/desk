// Where the sign-in page sends a freshly authenticated user.
//
// The value comes from the page's own address bar (`?suite=`), so it is
// untrusted input: the only producer (`hub/page.ts::start`) always builds a
// same-origin path, and anything else is refused here rather than followed.
//
// PURE AND WITHOUT DOM, like `adresse-plateforme.ts`: the page's origin is a
// PARAMETER, so the rule is decidable in Node.
//
// The value is RESOLVED by the WHATWG URL parser against the page's origin and
// the ORIGINS are compared, rather than inspecting the string: the parser is
// what the browser will apply on navigation, so a scheme-relative `//host`, a
// backslash read as a slash, or a `javascript:` URL (whose origin is the
// opaque `null`) are all judged exactly as they would be followed.

/// The root, where the sign-in page goes when nothing valid is named.
const RACINE = '/';

/// The post-sign-in destination: a same-origin path (path, query and
/// fragment) resolved from `valeur`, or the root for anything else —
/// absent, empty, unparsable, or on another origin.
export function destinationApresConnexion(valeur: string | null, origine: string): string {
    if (valeur === null || valeur === '') return RACINE;
    let cible: URL;
    try {
        cible = new URL(valeur, origine);
    } catch {
        // Unparsable against a valid base: not a destination.
        return RACINE;
    }
    if (cible.origin !== new URL(origine).origin) return RACINE;
    return `${cible.pathname}${cible.search}${cible.hash}`;
}
