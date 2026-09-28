// The SHAPE of the tag the Vite plugin injects for the anti-FOUC bootstrap —
// extracted from `client/vite.config.ts` for a single reason: to STAY
// TYPECHECKED.
//
// 🔴 `vite.config.ts` IS NEVER TYPECHECKED (see its header): it is outside
// both patterns of `client/tsconfig.json:12` (`src/**/*.ts`,
// `../proto/ts/**/*.ts`). A test importing it DIRECTLY would drag its
// type errors into `npx tsc --noEmit` — which MUST stay a step
// DISTINCT from `npx vitest run`, `CLAUDE.md` says so in black and white: "VITEST
// TRANSPILES WITHOUT CHECKING TYPES". This file lives under `src/`, SO it
// is typechecked, and `vite.config.ts` IMPORTS it — the reverse direction poses
// no problem, Rollup never typechecks its own config file.
//
// ⚠️ THIS FILE IS DELIBERATELY NOT THE ONE THAT READS THE BOOTSTRAP'S CONTENT.
// `vite.config.ts` must read `amorce-theme.js` through `node:fs` — a `?raw`
// import DOES NOT RESOLVE when Vite bundles ITS OWN configuration (measured:
// `npm run build` fails with "No matching export … for import "default"",
// esbuild treating `?raw` as a literal file path outside the
// plugin pipeline that this loading mode does not enable). And
// `client/` does not have `@types/node` (`src/presse-papier.test.ts` carries the same
// finding), so a `readFileSync` HERE would break `npx tsc --noEmit`. Hence the
// split: `vite.config.ts` keeps ITS `node:fs` read (not typechecked,
// as before this batch), and this module only carries what needs
// NO disk read — the NAME of the emitted file and the SHAPE of the tag
// that references it. The content itself, `amorce-theme.csp.test.ts` reads it
// through `?raw` (which, for its part, resolves perfectly well under Vitest).
//
// See `client/vite.config.ts` (the big comment above
// `NOM_FICHIER_AMORCE`) for the FULL REASONING of batch `csp-amorce`
// (August 29th, 2026): why the bootstrap is an external `'self'` file and not
// a `sha256-…` hash in the CSP.

/// The name under which the bootstrap is emitted into `dist/` — a STABLE name, outside
/// `assets/` (the only directory Vite fingerprints), hence never a year
/// of `immutable` on content that changes without the name moving.
export const NOM_FICHIER_AMORCE = 'amorce-theme.js';

/// The tag the Vite plugin injects into the `<head>` of EVERY page.
///
/// 🔴 `src`, NEVER `children` — an INLINE script, AND THAT IS THE WHOLE DEFECT
/// measured on August 29th, 2026 (`https://app.allanic.me`, Chrome): the
/// platform's CSP (`plateforme/src/http/page/entetes-page.ts::CSP`) carries
/// `script-src 'self'` without `'unsafe-inline'` or a hash, and blocked this script
/// as long as it went out inline.
///
/// 🔴 NEITHER `async`, NOR `defer`, NOR `type: 'module'` — all three would DEFER
/// execution until after `<body>` is parsed, hence after first paint,
/// and would break the anti-FOUC this bootstrap exists to avoid. A
/// CLASSIC `<script src>` blocks document parsing until its
/// execution, exactly like the inline script it replaces — give or take one
/// network round trip, on the SAME origin.
export function baliseAmorce() {
    return {
        tag: 'script' as const,
        attrs: { src: `/${NOM_FICHIER_AMORCE}` },
        injectTo: 'head' as const,
    };
}
