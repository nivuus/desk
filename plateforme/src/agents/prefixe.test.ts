// The opaque session prefix: what makes the namespace GLOBAL.
//
// 🔴 The central red of this file is "the prefix never contains the
// separator". All of the plan's D2 rests on it: the TURN identifier is
// `<expiration>:<session>`, hence `<expiration>:<prefix>:<name>` once the
// session is prefixed, and coturn cuts on the FIRST `:`. If the prefix
// alphabet could carry a `:`, the first bound would become ambiguous.

import { describe, expect, it } from 'vitest';
import { deriverIdentifiants } from '../signaling/ice';
import { SEPARATEUR, composer, decouper, newPrefix } from './prefixe';

describe('the opaque prefix', () => {
    it('returns 22 characters of the base64url alphabet', () => {
        // 16 bytes in base64url make 22 characters without padding. The alphabet
        // excludes `+` and `/`: `/` would one day break a URL split, and `+`
        // turns into a space in a badly decoded query string.
        const p = newPrefix();
        expect(p).toHaveLength(22);
        expect(p).toMatch(/^[A-Za-z0-9_-]{22}$/);
    });

    it('returns two DIFFERENT values on two calls', () => {
        // A fixed seed would make the prefix guessable, hence the namespace
        // global but not opaque.
        expect(newPrefix()).not.toBe(newPrefix());
    });

    it('🔴 NEVER contains the separator, over a large number of draws', () => {
        // It is the assertion the non-ambiguity of the TURN format
        // rests on. 500 draws, that is 11,000 characters: an alphabet that
        // carried `:` would show it here.
        for (let i = 0; i < 500; i += 1) {
            expect(newPrefix()).not.toContain(SEPARATEUR);
        }
    });

    it('composes `<prefix>:<name>`', () => {
        expect(composer('PPP', 'bureau')).toBe('PPP:bureau');
        expect(composer('PPP', 'w-1')).toBe('PPP:w-1');
    });

    it('composes WITHOUT a separator when the prefix is empty', () => {
        // 🔴 It is what restores EXACTLY the behaviour from before P3 —
        // `bureau`, `w-1`. Setting the separator unconditionally would give
        // `:bureau`, which is the name of no existing session, and nothing would
        // flag it.
        expect(composer('', 'bureau')).toBe('bureau');
        expect(composer('', 'w-1')).toBe('w-1');
    });

    it('splits on the FIRST separator, and returns an empty prefix otherwise', () => {
        expect(decouper('PPP:bureau')).toEqual({ prefixe: 'PPP', nom: 'bureau' });
        // Without a prefix: the local trial mode, which must stay EXPLICIT and
        // not throw.
        expect(decouper('bureau')).toEqual({ prefixe: '', nom: 'bureau' });
        // Splitting on the LAST separator would let `PPP:w-1` through but
        // would break a name that itself carried a `:`.
        expect(decouper('PPP:a:b')).toEqual({ prefixe: 'PPP', nom: 'a:b' });
    });

    it('🔴 keeps the FIRST segment of the TURN identifier unambiguous', () => {
        // E6's check, pinned on the exact string like `ice.test.ts:13`.
        // `maintenant` in milliseconds, the duration in seconds: 1,000 + 3,600.
        const p = 'AAAAAAAAAAAAAAAAAAAAAA';
        const { username } = deriverIdentifiants('secret', composer(p, 'bureau'), 3600, 1_000_000);
        expect(username).toBe('4600:AAAAAAAAAAAAAAAAAAAAAA:bureau');
        // And it is indeed the expiry that coturn will read by cutting on the first
        // `:` — putting the expiry AFTER the session would break this.
        expect(username.split(SEPARATEUR)[0]).toBe('4600');
    });
});
