import { describe, expect, it } from 'vitest';
import { baliseManifesteHub } from './manifeste-hub-greffon';

/// 🔴 THIS IS THE REGRESSION ITSELF, REPLAYED. Measured on August 29th, 2026
/// (`https://app.allanic.me/hub.html`, Chrome, owner's console): a
/// `<link rel="manifest">` WITHOUT `crossorigin` goes out without cookies, Pomerium
/// answers with a redirect to `authenticate.allanic.me`, and the CSP
/// blocks loading from another origin — visible under
/// `default-src 'self'`. This test fails if anyone removes the attribute.
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
