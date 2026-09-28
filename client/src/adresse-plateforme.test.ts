// 🔴 THIS FILE EXISTS FOR A FAILURE NO NODE TEST CAN SEE.
//
// A page served over `https://` by the proxy that opened `ws://…:8080` is
// MIXED CONTENT: the browser refuses the connection, silently for anything
// that is not the console. The P5 deployment would therefore be shipped
// broken AND GREEN. This module makes the thing decidable in Node by making
// `location` a PARAMETER — the pattern of `resize.ts` and `prefixe.ts`.

import { describe, expect, it } from 'vitest';
import { adressePlateforme, adresseSignaling, type Emplacement } from './adresse-plateforme';

/// A page served by the proxy, over TLS, on the default port.
const TLS: Emplacement = { protocol: 'https:', host: 'plateforme.exemple.fr' };
/// The same in clear text — the bootstrap of a deployment, or a local trial.
const CLAIR: Emplacement = { protocol: 'http:', host: 'plateforme.exemple.fr' };
/// The vite development server.
const SAMPLE: Emplacement = { protocol: 'http:', host: 'localhost:5173' };

describe('adressePlateforme and adresseSignaling', () => {
    it("(a) a page on https: gives https: and wss:/signal", () => {
        expect(adressePlateforme(TLS)).toBe('https://plateforme.exemple.fr');
        expect(adresseSignaling(TLS)).toBe('wss://plateforme.exemple.fr/signal');
    });

    it("(b) a page on http: gives http: and ws:/signal", () => {
        expect(adressePlateforme(CLAIR)).toBe('http://plateforme.exemple.fr');
        expect(adresseSignaling(CLAIR)).toBe('ws://plateforme.exemple.fr/signal');
    });

    it("🔴 (c) the EXPLICIT parameter wins, over both", () => {
        // 🔴 THE RED: always deriving from the page. The local trial mode
        // would disappear, and P3 already paid for the disappearance of a trial mode.
        expect(adressePlateforme(TLS, 'http://192.168.3.2:8080'))
            .toBe('http://192.168.3.2:8080');
        expect(adresseSignaling(TLS, 'ws://192.168.3.2:8080'))
            .toBe('ws://192.168.3.2:8080');
    });

    it("🔴 (d) NO :8080 appears when the page is on the default port", () => {
        // 🔴 THIS IS THE WHOLE POINT OF THIS MODULE, and the red is the former code:
        // `ws://${location.hostname}:8080`. Under TLS it is mixed content, and
        // the browser refuses — without any Node test being able to see it.
        expect(adressePlateforme(TLS)).not.toContain(':8080');
        expect(adresseSignaling(TLS)).not.toContain(':8080');
        expect(adressePlateforme(CLAIR)).not.toContain(':8080');
        expect(adresseSignaling(CLAIR)).not.toContain(':8080');
    });

    it("🔴 (d bis) under TLS, NEVER ws: nor http: — that is mixed content", () => {
        // 🔴 THE most direct RED: keeping `ws://` hardcoded. This assertion
        // falls, and it is SEPARATE from (a) because `expect` interrupts a
        // test at the first false assertion — the lesson P2 paid for.
        expect(adresseSignaling(TLS).startsWith('wss://')).toBe(true);
        expect(adressePlateforme(TLS).startsWith('https://')).toBe(true);
    });

    it("(e) the page PORT is kept, not replaced by 8080", () => {
        // The vite development server serves on 5173: the address returned
        // is that of the page, port included.
        expect(adressePlateforme(SAMPLE)).toBe('http://localhost:5173');
        expect(adresseSignaling(SAMPLE)).toBe('ws://localhost:5173/signal');
    });

    it("(f) a NON-standard port under TLS is kept too", () => {
        const p: Emplacement = { protocol: 'https:', host: 'exemple.fr:8443' };
        expect(adressePlateforme(p)).toBe('https://exemple.fr:8443');
        expect(adresseSignaling(p)).toBe('wss://exemple.fr:8443/signal');
    });

    it("(g) an EMPTY or absent parameter does not win", () => {
        // An empty string is what `URLSearchParams.get` returns on `?x=`: treating
        // it as explicit would produce an empty address, hence a failure
        // without a message. `null` is what it returns on an absent parameter.
        expect(adresseSignaling(TLS, '')).toBe('wss://plateforme.exemple.fr/signal');
        expect(adresseSignaling(TLS, null)).toBe('wss://plateforme.exemple.fr/signal');
        expect(adressePlateforme(TLS, '')).toBe('https://plateforme.exemple.fr');
        expect(adressePlateforme(TLS, null)).toBe('https://plateforme.exemple.fr');
    });

    it("(h) an unknown protocol does not build an absurd address", () => {
        // `file:` happens when the HTML is opened from disk. Nothing can
        // be deduced from it: the module falls back to clear text rather than returning
        // `filews://`, which no browser would understand and whose error message
        // would not point at the cause.
        const f: Emplacement = { protocol: 'file:', host: '' };
        expect(adresseSignaling(f)).toBe('ws:///signal');
        expect(adressePlateforme(f)).toBe('http://');
    });
});
