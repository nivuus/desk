// The sign-in page navigates to the destination named in its own address bar:
// only a same-origin path may come out of `destinationApresConnexion`.

import { describe, expect, it } from 'vitest';
import { destinationApresConnexion } from './destination';

const ORIGINE = 'https://plateforme.example.org';

describe('destinationApresConnexion', () => {
    it('an absolute address on another origin falls back to the root', () => {
        expect(destinationApresConnexion('https://other.example.net/hub', ORIGINE)).toBe('/');
    });

    it('a scheme-relative address falls back to the root', () => {
        expect(destinationApresConnexion('//other.example.net/hub', ORIGINE)).toBe('/');
    });

    it('a backslash-led address, which the URL parser reads as scheme-relative, falls back to the root', () => {
        expect(destinationApresConnexion('/\\other.example.net/hub', ORIGINE)).toBe('/');
    });

    it('a non-http scheme falls back to the root', () => {
        expect(destinationApresConnexion('javascript:alert(1)', ORIGINE)).toBe('/');
        expect(destinationApresConnexion('data:text/html,x', ORIGINE)).toBe('/');
    });

    it('the same host on another port or scheme is another origin', () => {
        expect(destinationApresConnexion('https://plateforme.example.org:8443/', ORIGINE)).toBe('/');
        expect(destinationApresConnexion('http://plateforme.example.org/', ORIGINE)).toBe('/');
    });

    it('a same-origin path keeps its query and fragment', () => {
        expect(destinationApresConnexion('/?app=42&x=%C3%A9#fin', ORIGINE)).toBe('/?app=42&x=%C3%A9#fin');
        expect(destinationApresConnexion('/hub.html?app=42', ORIGINE)).toBe('/hub.html?app=42');
    });

    it('an absolute address on the page origin is reduced to its path', () => {
        expect(destinationApresConnexion(`${ORIGINE}/hub.html?app=1`, ORIGINE)).toBe('/hub.html?app=1');
    });

    it('an absent or empty value goes to the root', () => {
        expect(destinationApresConnexion(null, ORIGINE)).toBe('/');
        expect(destinationApresConnexion('', ORIGINE)).toBe('/');
    });
});
