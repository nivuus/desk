import { describe, expect, it } from 'vitest';
import { resoudre } from './resolution';

describe('resoudre', () => {
    // 🔴 THE RED OF "SERVE THE HUB AT THE ROOT" (owner's decision,
    // 30 August 2026). Before this batch, `/` returned `index.html` — the
    // SESSION page — measured in PRODUCTION as the failure: without a
    // `?session=` parameter, this page invents a session, fails, and shows "Session
    // failed". This test turns red if `PAGE` ever goes back to
    // `index.html`.
    it('returns hub.html for the root, and marks it DOCUMENT', () => {
        expect(resoudre('/')).toEqual({
            ok: true,
            file: 'hub.html',
            mime: 'text/html; charset=utf-8',
            document: true,
            empreinte: false,
        });
    });

    it('returns a named file', () => {
        expect(resoudre('/hub.html')).toEqual({
            ok: true,
            file: 'hub.html',
            mime: 'text/html; charset=utf-8',
            document: true,
            empreinte: false,
        });
    });

    // A RESOURCE, not a document: it is this boolean that later decides
    // between `no-store` and `immutable`, and getting it wrong here would kill the
    // browser cache on every file fingerprinted by Vite.
    it("marks an asset as a RESOURCE, never as a document", () => {
        expect(resoudre('/assets/index-a1b2c3.js')).toEqual({
            ok: true,
            file: 'assets/index-a1b2c3.js',
            mime: 'text/javascript; charset=utf-8',
            document: false,
            empreinte: true,
        });
    });

    // The SPA fallback, reproduced: a path without extension falls back to the
    // page, never to a 404 — and since 30 August 2026, that page is the
    // HUB, not the session (see the root test above).
    it('folds a path without extension onto hub.html', () => {
        expect(resoudre('/quelconque')).toMatchObject({ ok: true, file: 'hub.html' });
    });

    // 🔴 TRAVERSAL IS JUDGED ON THE RESOLVED PATH. A substring filter on
    // `..` would let the encoded form below through.
    it('refuses an ENCODED traversal', () => {
        expect(resoudre('/%2e%2e%2fetc%2fpasswd')).toEqual({ ok: false, motif: 'traversee' });
    });

    it('refuses a plaintext traversal', () => {
        expect(resoudre('/../etc/passwd')).toEqual({ ok: false, motif: 'traversee' });
    });

    it('refuses a traversal that goes back up AFTER going down', () => {
        expect(resoudre('/assets/../../etc/passwd')).toEqual({ ok: false, motif: 'traversee' });
    });

    it('refuses a NUL byte', () => {
        expect(resoudre('/index.html%00.txt')).toEqual({ ok: false, motif: 'octet-nul' });
    });

    it('refuses a malformed encoding, rather than throwing', () => {
        expect(resoudre('/%zz')).toEqual({ ok: false, motif: 'chemin-invalide' });
    });

    // 🔴 THE MIME LIST IS CLOSED. Falling back to `application/octet-stream`
    // would publish any file left in the root — a `.env`, a key, a
    // `.map` — with a download prompt.
    it("refuses an extension outside the list, never falls back to octet-stream", () => {
        expect(resoudre('/.env')).toEqual({ ok: false, motif: 'extension-inconnue' });
        expect(resoudre('/index.js.map')).toEqual({ ok: false, motif: 'extension-inconnue' });
    });

    // 🔴 THE HOLE THE MIME LIST LET THROUGH: `.json` IS a KNOWN
    // extension, so a name reduced to that bare extension alone went through the
    // list above. `/.env` and `/.htaccess`, for their part, stay refused by
    // `extension-inconnue` (their "extension" is not listed): this guard
    // changes NOTHING in their verdict, it closes the case the list, on its
    // own, could not close.
    it('refuses a name reduced to a bare extension, even one known to the list', () => {
        expect(resoudre('/.json')).toEqual({ ok: false, motif: 'nom-vide' });
    });

    it('serves the manifest and the icons that the hub names', () => {
        expect(resoudre('/hub.webmanifest')).toMatchObject({
            ok: true,
            mime: 'application/manifest+json',
            document: false,
        });
        expect(resoudre('/favicon.ico')).toMatchObject({ ok: true, document: false });
    });
});

// 🔴 THIS BLOCK EXISTS BECAUSE THE EXPECTATIONS ABOVE PINNED THE FAULTY
// BEHAVIOUR. `/hub.webmanifest` and `/favicon.ico` were classified there like the
// assets — same `document: false`, hence same headers —, and this equality
// gave a year of `immutable` to two names Vite NEVER fingerprints. MEASURED
// on the real `client/dist`: `/hub.webmanifest` returned
// `public, max-age=31536000, immutable`, hence **the hub's PWA manifest
// unrevisable for a year** in every browser that had seen it.
//
// 🔴 WHAT DECIDES IS THE LOCATION, NOT THE EXTENSION NOR THE SHAPE OF THE NAME. A
// `.js` placed at the root is not fingerprinted; a `.png` under the assets
// directory is. A regular expression on "a dash followed by eight
// characters" would have been fooled by a hand-written name.
describe("the fingerprint, which decides the cache", () => {
    it('an asset of the Vite directory IS fingerprinted', () => {
        expect(resoudre('/assets/main-DOC38JmJ.css')).toMatchObject({ empreinte: true });
    });

    // 🔴 THE MEASURED CASE. It is THIS test that would turn red if the manifest
    // became `immutable` again.
    it("the PWA manifest of the hub is NOT fingerprinted", () => {
        expect(resoudre('/hub.webmanifest')).toMatchObject({ empreinte: false });
    });

    it("a root icon is NOT fingerprinted", () => {
        expect(resoudre('/favicon.ico')).toMatchObject({ empreinte: false });
    });

    // ⚠️ THE WITNESS THAT SEPARATES "LOCATION" FROM "EXTENSION": same
    // extension as the asset of the first test, other location, other verdict.
    it("a `.js` placed at the ROOT is not fingerprinted", () => {
        expect(resoudre('/prefixe-iK9obSCN.js')).toMatchObject({ empreinte: false });
    });

    // ⚠️ THE PREFIX CARRIES ITS SEPARATOR: without it, this name would pass for an
    // asset.
    it("a root file whose name STARTS with `assets` is not fingerprinted", () => {
        expect(resoudre('/assetsX.js')).toMatchObject({ empreinte: false });
    });

    // ⚠️ THE SPA FALLBACK RETURNS `hub.html` (since 30 August 2026), NEVER AN
    // ASSET: a path without extension UNDER the assets directory must
    // not inherit its cache.
    it("a path without extension under `assets/` falls back to the page, not fingerprinted", () => {
        expect(resoudre('/assets/quelque-chose')).toMatchObject({
            file: 'hub.html',
            empreinte: false,
        });
    });
});
