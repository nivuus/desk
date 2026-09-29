import { describe, expect, it } from 'vitest';
// 🔴 THIS TEST IMPORTS `plateforme/src/http/page/entetes-page.ts` — ANOTHER
// PACKAGE — AND THAT IS THE POINT. The defect measured on August 29th, 2026
// (`Executing inline script violates … script-src 'self'`) existed
// precisely because no test looked at both sides at once: the
// CSP lives in `plateforme/`, the anti-FOUC bootstrap lives in `client/`, and
// each package has its own `vitest` suite, which only sees half of the
// problem. `docs/claude/pitfalls-shell-tests-acceptance.md` names the class: "WHAT A BROWSER REQUIRES, NO
// NODE TEST SEES". This file is the guard that links the two —
// should it one day be deleted to "lighten" this package, the defect
// would become invisible to `npm test` again, exactly as before this batch.
import { CSP } from '../../../plateforme/src/http/page/entetes-page';
import { BOOTSTRAP_FILE_NAME, baliseAmorce } from './amorce-theme-greffon';
// `?raw`, NEVER `node:fs`: see `amorce-theme-greffon.ts`, which explains
// why reading CONTENT does not live in this shared module.
import AMORCE from './amorce-theme.js?raw';

/** The tag the `guac-amorce-theme` plugin really injects into
 * `<head>` — THE SAME FUNCTION `vite.config.ts` calls, never a copy:
 * see `amorce-theme-greffon.ts` for why it lives under `src/` and not
 * in `vite.config.ts`, which is never typechecked. */
function baliseInjectee() {
    return baliseAmorce();
}

describe("the anti-FOUC bootstrap cannot be blocked by the served CSP", () => {
    // 🔴 THIS IS THE REGRESSION ITSELF, REPLAYED BACKWARDS. Before this batch, the
    // plugin returned `{ tag: 'script', children: AMORCE, … }`: an INLINE
    // script. The CSP below NEVER admitted `'unsafe-inline'` nor any
    // hash for `script-src` — so Chrome blocked it, exactly as
    // measured. This test fails if anyone reintroduces `children` without changing
    // the CSP, AND if anyone hardens the CSP without checking that the bootstrap stays
    // servable.
    it('the plugin injects a `src` tag, NEVER `children` (an inline script)', () => {
        const balise = baliseInjectee();
        expect(balise.attrs).toHaveProperty('src');
        expect(balise).not.toHaveProperty('children');
    });

    it("the tag is neither `async`, nor `defer`, nor `type=\"module\"` — the anti-FOUC requires it", () => {
        // An async, deferred, or module script would run
        // AFTER the `<body>` is parsed, so after the first paint: that
        // would be the FOUC this bootstrap exists to avoid. A CLASSIC `<script
        // src>`, on the other hand, blocks document parsing until it
        // runs — exactly like the inline script it replaces.
        //
        // ⚠️ `as Record<string, unknown>`: the REAL type of `attrs`
        // (`{ src: string }`) carries NONE of these three keys — that is
        // precisely what this test wants to prove, so accessing them requires
        // stepping away from it HERE, in the test, without widening the production type.
        const attrs = baliseInjectee().attrs as Record<string, unknown>;
        expect(attrs.async).toBeUndefined();
        expect(attrs.defer).toBeUndefined();
        expect(attrs.type).not.toBe('module');
    });

    it("the served URL is SAME ORIGIN (an absolute path, never a third-party host)", () => {
        // 🔴 `script-src 'self'` admits ONLY the same origin. An absolute URL
        // (`https://…`) would silently escape this test while being
        // blocked by the CSP in practice — the same class of defect, a
        // second time.
        const balise = baliseInjectee();
        expect(balise.attrs?.src).toBe(`/${BOOTSTRAP_FILE_NAME}`);
        expect(balise.attrs?.src).toMatch(/^\//);
    });

    it("the CSP needs NO `'unsafe-inline'` nor hash to admit this file", () => {
        // 🔴 THIS ASSERTION IS WHAT MAKES REMEDY (b) DECISIVE: `'self'`
        // alone already covers a same-origin file. A `sha256-…` hash
        // would have had to be KEPT IDENTICAL, by hand, in
        // `deploiement/nginx.conf` — which carries its own STATIC copy of
        // this CSP (see `entetes-page.test.ts`, "the CSP does not drift from
        // nginx's") and can NOT compute a hash on the fly. That is
        // exactly the "wreck of 487": a copied hash outlives the
        // content it described. `'self'` is not copied, so it
        // never drifts.
        const scriptSrc = CSP.split(';')
            .map((d) => d.trim())
            .find((d) => d.startsWith('script-src'));
        expect(scriptSrc).toBe("script-src 'self'");
    });
});

describe("the built asset does not diverge from its source", () => {
    // 🔴 THIS BATCH'S RED DEMONSTRATION GOES THROUGH THIS TEST. `client/dist`
    // is a BUILD ARTEFACT (gitignored): this test requires `npm run build`
    // BEFOREHAND, and throws LOUDLY (no fallback `||`, no silent
    // `try/catch`) if `client/dist/amorce-theme.js` is absent — "a fallback `||`
    // turns *absent file* into *green check*" (`CLAUDE.md`).
    //
    // What this test establishes: the file ACTUALLY served by the platform
    // (the one `PLATEFORME_PAGE` exposes, copied from `client/dist` to
    // `/opt/nivuus/desk/client/dist` in production) is BYTE FOR BYTE
    // the one `src/design/amorce-theme.js` defines today. Turned red and
    // reverted on August 29th, 2026: `cp src/design/amorce-theme.js
    // <scratchpad>/amorce-theme.backup.js`, one byte added to the source WITHOUT
    // a rebuild, this test goes red (the source changed, `dist/` did not), restored
    // from the named copy — never `git checkout --`, which would also have
    // erased any uncommitted work (`docs/claude/pitfalls-shell-tests-acceptance.md`, shell traps).
    it('`dist/amorce-theme.js` is identical to `src/design/amorce-theme.js`', async () => {
        // 🔴 DYNAMIC IMPORT, NOT STATIC: an `import … from '../../dist/…'`
        // at the top of the file would make MODULE RESOLUTION fail as early as
        // collection, and would bring down the FOUR OTHER tests of this file
        // with it — a diagnosis that would point at the wrong culprit.
        // Here, only THIS test fails, with the message that says what to do.
        let servi: string;
        try {
            const module = await import('../../dist/amorce-theme.js?raw');
            servi = module.default;
        } catch (cause) {
            throw new Error(
                `client/dist/amorce-theme.js is missing: run 'npm run build' in ` +
                    `client/ before this test — a missing build file is not a ` +
                    `green check. Cause: ${String(cause)}`,
            );
        }
        expect(servi).toBe(AMORCE);
    });
});
