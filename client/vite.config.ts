// ⚠️ `defineConfig` COMES FROM `vitest/config`, NOT FROM `vite` — see the `test`
// block at the bottom of this file. It is the same function, with the typing of the
// `test` key on top, and Vite ignores it at build time. No new dependency:
// Vitest is already a development dependency.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
// 🔴 THE HUB MANIFEST PLUGIN READS THE TOKENS THROUGH THEIR PARSER, never through
// a home-made regular expression: a check that has its own copy of the
// values validates its copy (⑥'s spec §7.1). Node v24 imports a `.ts`
// natively, which three tools of the base already exploit — and that is why
// any module of `client/src/design/` imported by a tool must stay
// "erasable": no `enum`, no `namespace`, no decorator.
// ⚠️ THE `.ts` EXTENSION IS MANDATORY HERE, AND ITS ABSENCE BREAKS TWO
// OF ⑥'S CHECKS — not this file. `outils/tokens-orphelins.mjs` and
// `outils/classes-employees.mjs` import THIS file to derive their
// scope from it, and they are loaded by NODE, whose resolver requires
// the extension where Vite does without. Written without it, the line left the
// build GREEN and brought §7.6 and §7.9 down with `ERR_MODULE_NOT_FOUND`.
import { lireBlocsDeTheme, valeurDePropriete } from './src/design/tokens.ts';
// 🔴 `NOM_FICHIER_AMORCE` AND `baliseAmorce` LIVE UNDER `src/`, NOT HERE —
// extracted on August 29th, 2026 (batch `csp-amorce`) precisely to stay
// TYPECHECKED, which this file never is (see below). `AMORCE`, for its part,
// STAYS read here, through `node:fs` — see the comment of
// `amorce-theme-greffon.ts` on why a `?raw` import does NOT resolve
// when Vite bundles its OWN configuration. Read that file for the
// complete reasoning: why the bootstrap is an external `'self'` file
// (never inline `children:`) and why a `sha256-…` hash in the CSP was
// ruled out.
import { NOM_FICHIER_AMORCE, baliseAmorce } from './src/design/amorce-theme-greffon.ts';
// 🔴 `baliseManifesteHub` LIVES UNDER `src/hub/`, NOT HERE — extracted on August 29th,
// 2026 (batch `manifeste-hub-crossorigin`) for the SAME reason as
// `baliseAmorce` above: staying TYPECHECKED. See its comment for the
// measured defect (`default-src 'self'` blocking the Pomerium redirect) and
// why `crossorigin="use-credentials"` is the retained remedy.
import { baliseManifesteHub } from './src/hub/manifeste-hub-greffon.ts';

/// The CONTENT of the bootstrap, read ONCE through `node:fs` — never `?raw`, see the
/// comment of `amorce-theme-greffon.ts`: such an import does not resolve
/// when Vite bundles its OWN configuration (measured: « No matching export …
/// for import "default" »). `amorce-theme.csp.test.ts`, for its part, reads this same
/// file through `?raw` — which resolves perfectly under Vitest — to compare,
/// byte for byte, the source with what `dist/amorce-theme.js` actually
/// carries.
const AMORCE = readFileSync(
    fileURLToPath(new URL('./src/design/amorce-theme.js', import.meta.url)),
    'utf8',
);

/* ═══════════════════════════════════════════════════════════════════════════
   THE HUB MANIFEST — generated AT BUILD TIME, never hard-written (sub-block G5).

   🔴 WHY IT IS GENERATED. `client/public/` does not exist, and a
   `.webmanifest` file placed anywhere would ESCAPE §7.2, whose scope is
   `client/src/**` in `.css`/`.ts` plus the Vite entries: a literal colour
   would pass there without any check seeing it, and the repository would have
   **two sources of truth for a colour**. The plugin therefore reads `--fond-0`
   and `--accent` through `client/src/design/tokens.ts`, exactly as the
   checks §7.1, §7.4 and §7.6 do — Node imports a `.ts` natively, which
   three of ⑥'s tools already exploit.

   🔴 WHAT THE HUB DECLARES AND THE PER-APPLICATION MANIFESTS CANNOT
   DECLARE: the `file_handlers` of the installer types, in line with
   the amendment of 28/07/2026 to the product framing. **No feature
   depends on them** — it is the amendment's clause, and G5's criterion ② exists
   to guard it: drag-and-drop works ALONE.

   ⚠️ THE MIME TYPES ARE A CHOICE, NOT A STANDARD — neither `.msi` nor `.bat`
   have an unambiguous IANA registration. What counts for criterion ③ is
   not their correctness but WHAT CHROMIUM ANSWERS ABOUT THEM, and that is what is recorded.

   ⚠️ THIS FILE IS NOT TYPECHECKED (see above): an error here is a
   BUILD FAILURE, never a `tsc` error.
   ═══════════════════════════════════════════════════════════════════════════ */
// 🔴 `tokens/couleurs.css` ALONE, NEVER `tokens.css`: since the extraction of
// task 6 (August 25th, 2026), `tokenRacine` below is only called with
// COLOUR names (`--fond-0`, `--accent`, for the web manifest) —
// `tokens/echelles.css`, its neighbour, has none to offer.
const TOKENS_CSS = readFileSync(
    fileURLToPath(new URL('./src/design/tokens/couleurs.css', import.meta.url)),
    'utf8',
);

/** The value of a token of the ROOT block (dark theme), or a build error. */
function tokenRacine(nom: string): string {
    const blocs = lireBlocsDeTheme(TOKENS_CSS);
    const racine = blocs.find((b) => b.nom === 'racine');
    if (racine === undefined) throw new Error('tokens/couleurs.css ne porte plus de bloc racine');
    const valeur = valeurDePropriete(racine, nom);
    // 🔴 WE THROW RATHER THAN FALL BACK TO A DEFAULT COLOUR: a fallback
    //    would set a colour that belongs to no token, that is, the
    //    second source of truth this plugin exists to avoid — and it would
    //    do so SILENTLY.
    if (valeur === null) throw new Error(`tokens/couleurs.css ne déclare plus ${nom}`);
    return valeur;
}

const MANIFESTE_HUB = {
    name: 'Applications',
    short_name: 'Applications',
    id: '/hub.html',
    start_url: '/hub.html',
    scope: '/',
    display: 'standalone',
    display_override: ['window-controls-overlay', 'standalone'],
    background_color: tokenRacine('--fond-0'),
    theme_color: tokenRacine('--accent'),
    file_handlers: [
        {
            action: '/hub.html',
            accept: {
                'application/x-msi': ['.msi'],
                'application/vnd.microsoft.portable-executable': ['.exe'],
                'application/x-bat': ['.bat'],
            },
        },
    ],
};

const greffonManifesteHub = {
    name: 'guac-manifeste-hub',
    generateBundle(_options: unknown, _bundle: unknown) {
        // @ts-expect-error — `this.emitFile` is Rollup's API, and this
        // file is not typechecked: the annotation states the intent.
        this.emitFile({
            type: 'asset',
            fileName: 'hub.webmanifest',
            source: JSON.stringify(MANIFESTE_HUB, null, 2),
        });
    },
    transformIndexHtml: {
        order: 'post' as const,
        handler(_html: string, ctx: { path: string }) {
            // 🔴 THE FILTER IS DELIBERATE HERE, UNLIKE THE BOOTSTRAP PLUGIN:
            //    the hub's manifest only concerns the hub. Setting it on the
            //    five other pages would make them PWAs nobody wanted, and
            //    §7.3 would not say so — it only judges stylesheets.
            if (!ctx.path.endsWith('/hub.html')) return [];
            // 🔴 `baliseManifesteHub()`, NEVER REWRITTEN HERE: it is the SAME
            //    function `manifeste-hub-greffon.test.ts` tests for
            //    `crossorigin="use-credentials"` — a plugin with its
            //    own copy of the attributes would validate its copy, not the
            //    product. See its comment for the measured defect.
            return [baliseManifesteHub()];
        },
    },
};

export const greffonAmorce = {
    name: 'guac-amorce-theme',
    // Emits the bootstrap's text as a build ASSET, just like
    // `hub.webmanifest` above — never copied, always the same read.
    generateBundle(_options: unknown, _bundle: unknown) {
        // @ts-expect-error — `this.emitFile` is Rollup's API, and this
        // file is not typechecked: the annotation states the intent.
        this.emitFile({
            type: 'asset',
            fileName: NOM_FICHIER_AMORCE,
            source: AMORCE,
        });
    },
    transformIndexHtml: {
        order: 'pre' as const,
        // 🔴 `baliseAmorce()` COMES FROM `src/design/amorce-theme-greffon.ts`,
        // NEVER COPIED HERE: it is the SAME function
        // `amorce-theme.csp.test.ts` calls to check the shape of the
        // tag — a plugin with its own copy would validate its copy.
        handler: () => [baliseAmorce()],
    },
};

export default defineConfig({
    plugins: [greffonAmorce, greffonManifesteHub],
    server: {
        host: '0.0.0.0',
        port: 5173,
    },
    build: {
        // ⚠️ A PAGE ABSENT FROM THIS LIST DOES NOT COME OUT OF THE BUILD, AND NOTHING
        // SAYS SO: `npm run build` returns 0 and the page is simply missing from
        // `dist/`. Noted on August 19th, 2026 by playing the case — `connexion.html`
        // already existed at the root, the build succeeded, and `dist/` only carried
        // `index.html` and `shell.html`. Every new page is added here.
        rollupOptions: {
            input: {
                main: 'index.html',
                shell: 'shell.html',
                connexion: 'connexion.html',
                // The token gallery: a surface built like the others,
                // hence subject to checks §7.2 and §7.3 — but EXCLUDED from the
                // "used" half of §7.6, which it would make unable
                // to fail. See `client/outils/tokens-orphelins.mjs`.
                design: 'design.html',
                // The PRIMITIVES gallery (S2). It is born separately rather than
                // in `design.html`, which was at 231 lines for a gate of
                // 300: the split is decided BEFORE the addition, never after.
                // It too is EXCLUDED from the "used" half of §7.6, and
                // for the same reason — see `client/outils/tokens-orphelins.mjs`.
                primitives: 'primitives.html',
                // The HUB (sub-block G5 of ④). It enters AUTOMATICALLY into
                // §7.2, §7.3, §7.6 and §7.9 ① merely by being here — their
                // scopes are DERIVED from this list, never copied.
                // ⚠️ It does NOT enter `SURFACES_PRODUIT` of §7.9 ② A, which
                // is the ONLY hard-coded list of the base
                // (`client/outils/classes-employees.mjs`). Adding it there would
                // change no verdict — ② A is a FLOOR, already
                // satisfied by three surfaces — and would make ④ cross the
                // boundary of a tool of ⑥, which is closed. A legacy item, rather
                // than a risky cosmetic change (decision D6).
                hub: 'hub.html',
            },
        },
    },
    test: {
        // ⚠️ WITHOUT THIS, AN `import css from './x.css?raw'` RETURNS THE EMPTY STRING
        // UNDER VITEST, silently. Vitest short-circuits CSS files
        // by default (`css: false`), and the short-circuit also catches the
        // `?raw` query. Measured on August 19th, 2026: `typeof` does return
        // `string`, but the length returns `0` — so a test parsing this
        // text would see NO token and would go green while measuring
        // nothing, which is the worst of both worlds.
        //
        // `client/src/design/reprise.test.ts` depends on it: it is what proves
        // that `tokens.css` takes over character for character the values
        // from before the base, and it must read the REAL file — a mirror of the
        // values in TypeScript would validate its own copy.
        //
        // ⚠️ There is deliberately NO `client/vitest.config.ts`: such a
        // file would take precedence over this one and Vitest would stop reading the
        // Vite configuration, without saying anything.
        css: true,
    },
});
