import { describe, expect, it } from 'vitest';
import { baliseManifesteHub } from './manifeste-hub-greffon';

/// 🔴 C'EST LA RÉGRESSION ELLE-MÊME, REJOUÉE. Mesuré le 29 août 2026
/// (`https://app.allanic.me/hub.html`, Chrome, console du propriétaire) : un
/// `<link rel="manifest">` SANS `crossorigin` part sans cookies, Pomerium
/// répond par une redirection vers `authenticate.allanic.me`, et la CSP
/// bloque le chargement d'une autre origine — visible sous
/// `default-src 'self'`. Ce test échoue si quiconque retire l'attribut.
describe('the hub <link rel="manifest"> tag carries crossorigin="use-credentials"', () => {
    it('otherwise the manifest goes out without cookies and Pomerium redirects to another origin', () => {
        const balise = baliseManifesteHub();
        expect(balise.attrs.crossorigin).toBe('use-credentials');
    });

    it('always points to /hub.webmanifest', () => {
        expect(baliseManifesteHub().attrs.href).toBe('/hub.webmanifest');
    });

    it('stays rel="manifest"', () => {
        expect(baliseManifesteHub().attrs.rel).toBe('manifest');
    });

    it("injects itself into <head>, like the bootstrap plugin", () => {
        expect(baliseManifesteHub().injectTo).toBe('head');
    });
});
