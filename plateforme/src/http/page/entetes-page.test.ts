import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
    CSP,
    ENTETES_DOCUMENT,
    ENTETES_RESSOURCE_EMPREINTEE,
    ENTETES_RESSOURCE_REVALIDABLE,
} from './entetes-page';

describe('the document headers', () => {
    // 🔴 THREE DISTINCT TESTS, NOT THREE ASSERTIONS IN ONE: `expect`
    // stops at the first failure, and these three security properties are
    // INDEPENDENT — if the CSP is wrong, nobody learns whether
    // `X-Frame-Options` is too.
    it('carries the CSP', () => {
        expect(ENTETES_DOCUMENT['Content-Security-Policy']).toBe(CSP);
    });

    it('carries Referrer-Policy: no-referrer', () => {
        expect(ENTETES_DOCUMENT['Referrer-Policy']).toBe('no-referrer');
    });

    it('carries X-Frame-Options: DENY', () => {
        expect(ENTETES_DOCUMENT['X-Frame-Options']).toBe('DENY');
    });

    // 🔴 HSTS STAYS AT THE TLS TERMINATOR. The platform is reachable in clear text
    // on 192.168.3.1:8080; asserting there that the origin is HTTPS for a year
    // would be a claim it is not in a position to make.
    it("does NOT emit Strict-Transport-Security", () => {
        expect(ENTETES_DOCUMENT).not.toHaveProperty('Strict-Transport-Security');
    });

    it('keeps no-store on the document, which may carry a token', () => {
        expect(ENTETES_DOCUMENT['Cache-Control']).toBe('no-store');
    });
});

describe('the headers of a FINGERPRINTED resource', () => {
    // 🔴 THE HALF THAT COUNTS. `no-store` here would kill the browser cache
    // on names Vite really fingerprints — the typical silent failure.
    it('is immutable, NEVER no-store', () => {
        expect(ENTETES_RESSOURCE_EMPREINTEE['Cache-Control']).toBe(
            'public, max-age=31536000, immutable',
        );
    });

    // 🔴 TWO DISTINCT TESTS, NOT TWO ASSERTIONS IN ONE — same reason
    // as above.
    it("does NOT carry Content-Security-Policy: it is a document header", () => {
        expect(ENTETES_RESSOURCE_EMPREINTEE).not.toHaveProperty('Content-Security-Policy');
    });

    it("does NOT carry X-Frame-Options: it is a document header", () => {
        expect(ENTETES_RESSOURCE_EMPREINTEE).not.toHaveProperty('X-Frame-Options');
    });
});

describe('the headers of a REVALIDATABLE resource', () => {
    // 🔴 THE NEW SET, AND THE REASON IT EXISTS: `hub.webmanifest`,
    // `favicon.ico` and every resource with a STABLE name received a year
    // of `immutable` — hence became unrevisable in every browser that had
    // seen them.
    it("is NEVER immutable", () => {
        expect(ENTETES_RESSOURCE_REVALIDABLE['Cache-Control']).not.toContain('immutable');
    });

    // ⚠️ SEPARATE: `no-store` is the OTHER extreme, just as wrong here, and a
    // regression towards it must turn red for ITS reason.
    it("is NEVER no-store either", () => {
        expect(ENTETES_RESSOURCE_REVALIDABLE['Cache-Control']).not.toContain('no-store');
    });

    it('requires a revalidation', () => {
        expect(ENTETES_RESSOURCE_REVALIDABLE['Cache-Control']).toContain('must-revalidate');
    });

    it('carries nosniff like the two other sets', () => {
        expect(ENTETES_RESSOURCE_REVALIDABLE['X-Content-Type-Options']).toBe('nosniff');
    });
});

// 🔴 TWO COPIES OF THE SAME POLICY DRIFT. This test is the only thing that
// prevents it: a hardening applied on one side only would ship two setups
// with different security without any suite flinching.
describe('the CSP does not drift from the nginx one', () => {
    it('is identical to the one of deploiement/nginx.conf', () => {
        // ⚠️ NO FALLBACK: `readFileSync` THROWS if the file is missing, and that is
        // wanted. "A fallback `||` turns *absent file* into *green
        // check*."
        const nginx = readFileSync(
            new URL('../../../../deploiement/nginx.conf', import.meta.url),
            'utf8',
        );
        const trouvees = [...nginx.matchAll(/add_header Content-Security-Policy "([^"]+)"/g)];
        // If nginx declared two, comparing "the first" would choose
        // silently. We require uniqueness rather than deciding.
        expect(trouvees).toHaveLength(1);
        expect(trouvees[0][1]).toBe(CSP);
    });
});

// 🔴 REGRESSION OF 30 AUGUST 2026, FOUND IN PRODUCTION BY THE OWNER:
// `manifest-src 'self'` (added on 29 August 2026 to no longer depend on the
// implicit fallback to `default-src`) does NOT cover the PER-APPLICATION
// manifest, published as `blob:` by `client/src/hub/page.ts`
// (`publierLeManifeste`, G5's path V1) — `'self'` alone blocks it in a loop
// on `https://app.allanic.me`. This test turns red if `manifest-src` loses `blob:`.
describe("manifest-src admits blob:, otherwise the PER-APPLICATION manifest no longer loads", () => {
    it('admits blob: in addition to self', () => {
        const [, directive] = CSP.match(/manifest-src ([^;]+);/) ?? [];
        expect(directive).toBe("'self' blob:");
    });
});
