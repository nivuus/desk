import { describe, expect, it } from 'vitest';
import { resoudre } from './resolution';

describe('resoudre', () => {
    // 🔴 LA ROUGE DE « SERS LE HUB À LA RACINE » (décision du propriétaire,
    // 30 août 2026). Avant ce lot, `/` rendait `index.html` — la page de
    // SESSION — mesurée en PRODUCTION comme la panne : sans paramètre
    // `?session=`, cette page invente une session, échoue, et rend « Échec
    // de la session ». Ce test rougit si `PAGE` revient un jour à
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

    // Une RESSOURCE, pas un document : c'est ce booléen qui décide plus tard
    // entre `no-store` et `immutable`, et se tromper ici tuerait le cache du
    // navigateur sur tous les fichiers empreintés par Vite.
    it("marks an asset as a RESOURCE, never as a document", () => {
        expect(resoudre('/assets/index-a1b2c3.js')).toEqual({
            ok: true,
            file: 'assets/index-a1b2c3.js',
            mime: 'text/javascript; charset=utf-8',
            document: false,
            empreinte: true,
        });
    });

    // Le repli SPA, reproduit : un chemin sans extension retombe sur la
    // page, jamais sur un 404 — et depuis le 30 août 2026, cette page est le
    // HUB, pas la session (voir le test de racine ci-dessus).
    it('folds a path without extension onto hub.html', () => {
        expect(resoudre('/quelconque')).toMatchObject({ ok: true, file: 'hub.html' });
    });

    // 🔴 LA TRAVERSÉE SE JUGE SUR LE CHEMIN RÉSOLU. Un filtre par sous-chaîne
    // `..` laisserait passer la forme encodée ci-dessous.
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

    // 🔴 LA LISTE MIME EST CLOSE. Retomber sur `application/octet-stream`
    // publierait tout fichier laissé dans la racine — un `.env`, une clé, un
    // `.map` — avec une invite de téléchargement.
    it("refuses an extension outside the list, never falls back to octet-stream", () => {
        expect(resoudre('/.env')).toEqual({ ok: false, motif: 'extension-inconnue' });
        expect(resoudre('/index.js.map')).toEqual({ ok: false, motif: 'extension-inconnue' });
    });

    // 🔴 LE TROU QUE LA LISTE MIME LAISSAIT PASSER : `.json` EST une extension
    // CONNUE, donc un nom réduit à cette seule extension nue franchissait la
    // liste ci-dessus. `/.env` et `/.htaccess`, eux, restent refusés par
    // `extension-inconnue` (leur « extension » n'y figure pas) : cette garde
    // ne change RIEN à leur verdict, elle ferme le cas que la liste, à elle
    // seule, ne pouvait pas fermer.
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

// 🔴 CE BLOC EXISTE PARCE QUE LES ATTENTES CI-DESSUS FIGEAIENT LE COMPORTEMENT
// FAUTIF. `/hub.webmanifest` et `/favicon.ico` y étaient classés comme les
// actifs — même `document: false`, donc mêmes en-têtes —, et cette égalité
// donnait un an d'`immutable` à deux noms que Vite n'empreinte JAMAIS. MESURÉ
// sur le vrai `client/dist` : `/hub.webmanifest` rendait
// `public, max-age=31536000, immutable`, donc **le manifeste PWA du hub non
// révisable pendant un an** chez tout navigateur l'ayant vu.
//
// 🔴 CE QUI TRANCHE EST L'EMPLACEMENT, PAS L'EXTENSION NI LA FORME DU NOM. Un
// `.js` posé à la racine n'est pas empreinté ; un `.png` sous le répertoire
// d'actifs l'est. Une expression régulière sur « un tiret suivi de huit
// caractères » se serait laissé tromper par un nom écrit à la main.
describe("the fingerprint, which decides the cache", () => {
    it('an asset of the Vite directory IS fingerprinted', () => {
        expect(resoudre('/assets/main-DOC38JmJ.css')).toMatchObject({ empreinte: true });
    });

    // 🔴 LE CAS MESURÉ. C'est CE test qui rougirait si le manifeste
    // redevenait `immutable`.
    it("the PWA manifest of the hub is NOT fingerprinted", () => {
        expect(resoudre('/hub.webmanifest')).toMatchObject({ empreinte: false });
    });

    it("a root icon is NOT fingerprinted", () => {
        expect(resoudre('/favicon.ico')).toMatchObject({ empreinte: false });
    });

    // ⚠️ LE TÉMOIN QUI SÉPARE « EMPLACEMENT » DE « EXTENSION » : même
    // extension que l'actif du premier test, autre emplacement, autre verdict.
    it("a `.js` placed at the ROOT is not fingerprinted", () => {
        expect(resoudre('/prefixe-iK9obSCN.js')).toMatchObject({ empreinte: false });
    });

    // ⚠️ LE PRÉFIXE PORTE SON SÉPARATEUR : sans lui, ce nom passerait pour un
    // actif.
    it("a root file whose name STARTS with `assets` is not fingerprinted", () => {
        expect(resoudre('/assetsX.js')).toMatchObject({ empreinte: false });
    });

    // ⚠️ LE REPLI SPA REND `hub.html` (depuis le 30 août 2026), JAMAIS UN
    // ACTIF : un chemin sans extension SOUS le répertoire d'actifs ne doit
    // pas hériter de son cache.
    it("a path without extension under `assets/` falls back to the page, not fingerprinted", () => {
        expect(resoudre('/assets/quelque-chose')).toMatchObject({
            file: 'hub.html',
            empreinte: false,
        });
    });
});
