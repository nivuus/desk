// The MATCHING OF THE THREE PATHS of `routes-installation.ts`, extracted on
// 22 August 2026. PURE: no `http`, no database, no clock — that is what makes it
// testable without standing up a server.
//
// 🔴 WHY THIS EXTRACTION EXISTS, AND SAYING IT BEATS LETTING IT
// LOOK GRATUITOUS: `routes-installation.ts` was EXACTLY at 500 lines
// out of 500, and the fix wave of the final review had to add eight
// comment lines to it — the sentence « the generic 404 then answers alone », which
// the page server made false unconditionally. `CLAUDE.md` forbids
// compressing to win back the margin (« extract, never compress »), and it
// equally forbids letting a file cross the ceiling. The extraction
// is therefore the only way out, and it covers what the file had that was most
// self-contained: three declarations with no dependency at all.
//
// ⚠️ AN EXTRACTION IS NEVER STRICTLY VERBATIM, and `CLAUDE.md` says
// so: here, the two functions go from private `function` to `export`, and
// that is the ONLY change — no body is touched, no comment is
// rewritten.

/// The path of the installation ORDER, compared EXACTLY.
export const CHEMIN_ORDRE = '/installation';

/// Matches `/installation/:id`, and NOTHING else. 🔴 SPLIT BY SEGMENTS,
/// NEVER BY `startsWith`: G1 MEASURED that a `startsWith('/application')`
/// left SEVENTEEN tests green — the route swallowed the family and returned ITS
/// OWN typed 404, indistinguishable from the generic one. Anchored at BOTH ends.
export function installationDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'installation', '<id>'] — exactement trois.
    if (segments.length !== 3) return undefined;
    if (segments[1] !== 'installation') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}

/// Matches `/televersement/:id/contenu`, and NOTHING else — same rule. ⚠️ THE
/// PATTERN STOPS AT `contenu`: that is what leaves room for the other routes
/// of the `/televersement/…` family, which a `startsWith` would swallow whole.
export function contenuDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'televersement', '<id>', 'contenu'] — exactement quatre.
    if (segments.length !== 4) return undefined;
    if (segments[1] !== 'televersement' || segments[3] !== 'contenu') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}
