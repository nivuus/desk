import { describe, expect, it } from 'vitest';
import { resoudre } from './resolution';

describe('resoudre', () => {
    it('rend index.html pour la racine, et le marque DOCUMENT', () => {
        expect(resoudre('/')).toEqual({
            ok: true,
            fichier: 'index.html',
            mime: 'text/html; charset=utf-8',
            document: true,
        });
    });

    it('rend un fichier nommé', () => {
        expect(resoudre('/hub.html')).toEqual({
            ok: true,
            fichier: 'hub.html',
            mime: 'text/html; charset=utf-8',
            document: true,
        });
    });

    // Une RESSOURCE, pas un document : c'est ce booléen qui décide plus tard
    // entre `no-store` et `immutable`, et se tromper ici tuerait le cache du
    // navigateur sur tous les fichiers empreintés par Vite.
    it("marque un actif comme RESSOURCE, jamais comme document", () => {
        expect(resoudre('/assets/index-a1b2c3.js')).toEqual({
            ok: true,
            fichier: 'assets/index-a1b2c3.js',
            mime: 'text/javascript; charset=utf-8',
            document: false,
        });
    });

    // Le `try_files $uri $uri/ /index.html` de nginx, reproduit : un chemin
    // sans extension retombe sur la page, jamais sur un 404.
    it('replie un chemin sans extension sur index.html', () => {
        expect(resoudre('/hub')).toMatchObject({ ok: true, fichier: 'index.html' });
    });

    // 🔴 LA TRAVERSÉE SE JUGE SUR LE CHEMIN RÉSOLU. Un filtre par sous-chaîne
    // `..` laisserait passer la forme encodée ci-dessous.
    it('refuse une traversée ENCODÉE', () => {
        expect(resoudre('/%2e%2e%2fetc%2fpasswd')).toEqual({ ok: false, motif: 'traversee' });
    });

    it('refuse une traversée en clair', () => {
        expect(resoudre('/../etc/passwd')).toEqual({ ok: false, motif: 'traversee' });
    });

    it('refuse une traversée qui remonte APRÈS être descendue', () => {
        expect(resoudre('/assets/../../etc/passwd')).toEqual({ ok: false, motif: 'traversee' });
    });

    it('refuse un octet NUL', () => {
        expect(resoudre('/index.html%00.txt')).toEqual({ ok: false, motif: 'octet-nul' });
    });

    it('refuse un encodage malformé, plutôt que de lever', () => {
        expect(resoudre('/%zz')).toEqual({ ok: false, motif: 'chemin-invalide' });
    });

    // 🔴 LA LISTE MIME EST CLOSE. Retomber sur `application/octet-stream`
    // publierait tout fichier laissé dans la racine — un `.env`, une clé, un
    // `.map` — avec une invite de téléchargement.
    it("refuse une extension hors de la liste, jamais ne retombe sur octet-stream", () => {
        expect(resoudre('/.env')).toEqual({ ok: false, motif: 'extension-inconnue' });
        expect(resoudre('/index.js.map')).toEqual({ ok: false, motif: 'extension-inconnue' });
    });

    // 🔴 LE TROU QUE LA LISTE MIME LAISSAIT PASSER : `.json` EST une extension
    // CONNUE, donc un nom réduit à cette seule extension nue franchissait la
    // liste ci-dessus. `/.env` et `/.htaccess`, eux, restent refusés par
    // `extension-inconnue` (leur « extension » n'y figure pas) : cette garde
    // ne change RIEN à leur verdict, elle ferme le cas que la liste, à elle
    // seule, ne pouvait pas fermer.
    it('refuse un nom réduit à une extension nue, même connue de la liste', () => {
        expect(resoudre('/.json')).toEqual({ ok: false, motif: 'nom-vide' });
    });

    it('sert le manifeste et les icônes que le hub nomme', () => {
        expect(resoudre('/hub.webmanifest')).toMatchObject({
            ok: true,
            mime: 'application/manifest+json',
            document: false,
        });
        expect(resoudre('/favicon.ico')).toMatchObject({ ok: true, document: false });
    });
});
