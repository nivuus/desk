import { describe, expect, it } from 'vitest';
import { ADRESSE_INCONNUE, adresseSource, pairDeConfiance } from './adresse-source';

/// Documentation addresses (RFC 5737), all DISTINCT and all
/// plausible: a module that normalises addresses cannot be tested
/// on `a`/`b`/`c` labels.
const CLIENT = '203.0.113.7';
const INTERMEDIAIRE = '198.51.100.4';
/// The address of a proxy on an internal docker network — the value an
/// operator will really set in `PLATEFORME_PROXY_DE_CONFIANCE`.
const PROXY = '172.18.0.5';

const NO_TRUST: ReadonlySet<string> = new Set();
const PROXY_DE_CONFIANCE: ReadonlySet<string> = new Set([PROXY]);

describe('adresseSource', () => {
    it("(a) 🔴 a NON-trusted source has its header IGNORED", () => {
        // The requester claims to come from elsewhere; nobody authorised it to
        // say so. Trusting this header would be identity spoofing, and
        // would make the per-address brake bypassable with one header line.
        expect(adresseSource(CLIENT, `${INTERMEDIAIRE}, ${PROXY}`, NO_TRUST)).toBe(CLIENT);
    });

    it("(a bis) EMPTY trust is the default, and it trusts nobody", () => {
        expect(adresseSource(PROXY, CLIENT, NO_TRUST)).toBe(PROXY);
    });

    it('(b) 🔴 a trusted source: we take the LAST element, never the first', () => {
        // `nginx` with `$proxy_add_x_forwarded_for` APPENDS its peer's address
        // to what the client sent. A client that sends
        // `X-Forwarded-For: 203.0.113.7` therefore produces
        // `203.0.113.7, <its real address>`: the FIRST element is the one
        // the client forged, the LAST is the only one the proxy wrote
        // itself.
        expect(adresseSource(PROXY, `${CLIENT}, ${INTERMEDIAIRE}`, PROXY_DE_CONFIANCE))
            .toBe(INTERMEDIAIRE);
    });

    it('(c) header absent ⇒ the address of the peer', () => {
        expect(adresseSource(PROXY, undefined, PROXY_DE_CONFIANCE)).toBe(PROXY);
    });

    it('(d) header present but empty or blank ⇒ the address of the peer', () => {
        expect(adresseSource(PROXY, ' , ', PROXY_DE_CONFIANCE)).toBe(PROXY);
        expect(adresseSource(PROXY, '', PROXY_DE_CONFIANCE)).toBe(PROXY);
    });

    it('(d bis) TRAILING empty elements are skipped, not taken for the last one', () => {
        // `X-Forwarded-For: 203.0.113.7, ` has an EMPTY last element. Taking
        // it would give an empty brake key, which every malformed request
        // would share.
        expect(adresseSource(PROXY, `${CLIENT}, `, PROXY_DE_CONFIANCE)).toBe(CLIENT);
    });

    it('(e) 🔴 an IPv4 wrapped in IPv6 returns the SAME key as its bare form', () => {
        // Without this normalisation, the same client counts TWICE depending on the
        // stack used, and its brake budget doubles.
        expect(adresseSource(`::ffff:${CLIENT}`, undefined, NO_TRUST))
            .toBe(adresseSource(CLIENT, undefined, NO_TRUST));
        expect(adresseSource(`::ffff:${CLIENT}`, undefined, NO_TRUST)).toBe(CLIENT);
    });

    it("(e bis) TRUST is judged on the normalised form, on both sides", () => {
        // Node commonly returns `::ffff:172.18.0.5` for an IPv4 peer on a
        // dual stack. Comparing the raw form to the configured value would make
        // trust fail silently — and the per-address brake
        // would degenerate into a GLOBAL brake without any line saying so.
        expect(adresseSource(`::ffff:${PROXY}`, CLIENT, PROXY_DE_CONFIANCE)).toBe(CLIENT);
        // And the configuration written in mapped form is worth the bare one.
        expect(adresseSource(PROXY, CLIENT, new Set([`::ffff:${PROXY}`]))).toBe(CLIENT);
    });

    it("(e ter) the retained header element is normalised too", () => {
        expect(adresseSource(PROXY, `::ffff:${CLIENT}`, PROXY_DE_CONFIANCE)).toBe(CLIENT);
    });

    it('(f) 🔴 a peer without an address returns a NAMED value, never « undefined »', () => {
        // An already closed socket returns `undefined` for `remoteAddress`. The brake
        // key must not become the string `"undefined"` by an interpolation
        // accident: it is the trap `signaling/turn-harnais.ts`
        // documents for `process.env`, and it replays here.
        const rendu = adresseSource(undefined, undefined, NO_TRUST);
        expect(rendu).toBe(ADRESSE_INCONNUE);
        expect(rendu).not.toBe('undefined');
        expect(rendu.length).toBeGreaterThan(0);
    });

    it("(f bis) a peer without an address NEVER becomes trusted", () => {
        // If `ADRESSE_INCONNUE` were inadvertently in the trust
        // set, every anonymous peer could forge its address.
        expect(adresseSource(undefined, CLIENT, new Set([ADRESSE_INCONNUE]))).toBe(ADRESSE_INCONNUE);
    });
});

describe('pairDeConfiance', () => {
    it('accepts a declared peer', () => {
        expect(pairDeConfiance('10.0.0.1', new Set(['10.0.0.1']))).toBe(true);
    });

    // The prefix of mapped IPv4 addresses, as `adresseSource` already does.
    it('normalises the ::ffff: prefix ON THE PEER SIDE', () => {
        expect(pairDeConfiance('::ffff:10.0.0.1', new Set(['10.0.0.1']))).toBe(true);
    });

    // 🔴 CORRECTION ROUND 1 — THE NORMALISATION OF THE DECLARED SIDE WAS
    // TESTED BY NOTHING: measured, replacing `normaliser(declare)` with
    // `declare` in `pairDeConfiance` left 645/645 tests green. The direction
    // of the failure is closed (an operator declaring a mapped form
    // would see EVERYONE refused, never an opening), but the claim
    // of the header comment — "NORMALISING IS MANDATORY ON BOTH SIDES" —
    // rested on nothing. This test closes the hole: trust is declared
    // in MAPPED form, the peer presents itself in BARE form.
    it('normalises the ::ffff: prefix ON THE DECLARED SIDE', () => {
        expect(pairDeConfiance('10.0.0.1', new Set(['::ffff:10.0.0.1']))).toBe(true);
    });

    it('refuses an undeclared peer', () => {
        expect(pairDeConfiance('10.0.0.2', new Set(['10.0.0.1']))).toBe(false);
    });

    // 🔴 AN EMPTY LIST TRUSTS NOBODY. The opposite would make
    // the absence of configuration an opening — the exact inverse of the safe default.
    it('refuses everybody when the list is empty', () => {
        expect(pairDeConfiance('10.0.0.1', new Set())).toBe(false);
    });

    it('refuses an absent address', () => {
        expect(pairDeConfiance(undefined, new Set(['10.0.0.1']))).toBe(false);
    });
});
