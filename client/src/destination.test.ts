// The sign-in page navigates to the destination named in its own address bar:
// only a same-origin path may come out of `postSignInDestination`.

import { describe, expect, it } from 'vitest';
import { postSignInDestination } from './destination';

const ORIGINE = 'https://plateforme.example.org';

describe('postSignInDestination', () => {
    it('an absolute address on another origin falls back to the root', () => {
        expect(postSignInDestination('https://other.example.net/hub', ORIGINE)).toBe('/');
    });

    it('a scheme-relative address falls back to the root', () => {
        expect(postSignInDestination('//other.example.net/hub', ORIGINE)).toBe('/');
    });

    it('a backslash-led address, which the URL parser reads as scheme-relative, falls back to the root', () => {
        expect(postSignInDestination('/\\other.example.net/hub', ORIGINE)).toBe('/');
    });

    it('a non-http scheme falls back to the root', () => {
        expect(postSignInDestination('javascript:alert(1)', ORIGINE)).toBe('/');
        expect(postSignInDestination('data:text/html,x', ORIGINE)).toBe('/');
    });

    it('the same host on another port or scheme is another origin', () => {
        expect(postSignInDestination('https://plateforme.example.org:8443/', ORIGINE)).toBe('/');
        expect(postSignInDestination('http://plateforme.example.org/', ORIGINE)).toBe('/');
    });

    it('a same-origin path keeps its query and fragment', () => {
        expect(postSignInDestination('/?app=42&x=%C3%A9#fin', ORIGINE)).toBe('/?app=42&x=%C3%A9#fin');
        expect(postSignInDestination('/hub.html?app=42', ORIGINE)).toBe('/hub.html?app=42');
    });

    it('an absolute address on the page origin is reduced to its path', () => {
        expect(postSignInDestination(`${ORIGINE}/hub.html?app=1`, ORIGINE)).toBe('/hub.html?app=1');
    });

    it('an absent or empty value goes to the root', () => {
        expect(postSignInDestination(null, ORIGINE)).toBe('/');
        expect(postSignInDestination('', ORIGINE)).toBe('/');
    });
});
