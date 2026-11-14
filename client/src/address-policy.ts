/// <reference types="vite/client" />
// Whether an explicit `?plateforme=` / `?signaling=` may name ANOTHER origin
// than the page — the build-time switch of `adresse-plateforme.ts`.
//
// Vite replaces `import.meta.env.DEV` and `import.meta.env.MODE` statically
// at build time (https://vite.dev/guide/env-and-mode): `DEV` is true only
// under the dev server, and `MODE` is `production` for a plain `vite build`
// (`npm run build`). A bench that drives the page against a platform on
// another origin therefore uses the dev server, or builds with
// `vite build --mode banc`. A production build never sets either, so the
// constant folds to `false` and only same-origin values are honoured.

/// The mode name a bench build passes to `vite build --mode`.
const BENCH_MODE = 'banc';

export const FOREIGN_ADDRESSES_ALLOWED: boolean =
    import.meta.env.DEV || import.meta.env.MODE === BENCH_MODE;
