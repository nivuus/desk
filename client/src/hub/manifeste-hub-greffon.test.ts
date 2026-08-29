import { describe, expect, it } from 'vitest';
import { baliseManifesteHub } from './manifeste-hub-greffon';

/// 🔴 C'EST LA RÉGRESSION ELLE-MÊME, REJOUÉE. Mesuré le 29 août 2026
/// (`https://app.allanic.me/hub.html`, Chrome, console du propriétaire) : un
/// `<link rel="manifest">` SANS `crossorigin` part sans cookies, Pomerium
/// répond par une redirection vers `authenticate.allanic.me`, et la CSP
/// bloque le chargement d'une autre origine — visible sous
/// `default-src 'self'`. Ce test échoue si quiconque retire l'attribut.
describe('la balise <link rel="manifest"> du hub porte crossorigin="use-credentials"', () => {
    it('sans quoi le manifeste part sans cookies et Pomerium redirige vers une autre origine', () => {
        const balise = baliseManifesteHub();
        expect(balise.attrs.crossorigin).toBe('use-credentials');
    });

    it('pointe toujours /hub.webmanifest', () => {
        expect(baliseManifesteHub().attrs.href).toBe('/hub.webmanifest');
    });

    it('reste rel="manifest"', () => {
        expect(baliseManifesteHub().attrs.rel).toBe('manifest');
    });

    it("s'injecte dans <head>, comme le greffon d'amorce", () => {
        expect(baliseManifesteHub().injectTo).toBe('head');
    });
});
