// The PURE part of command-line account creation.
//
// 🔴 The red of this file is `--mot-de-passe`: accepting it "for
// convenience" would expose the password to EVERY user of the machine,
// `ps` giving the argv of every process. The refusal is explicit and carries its
// reason, so that the administrator knows what to do instead.

import { describe, expect, it } from 'vitest';
import { analyserArguments } from './creer-utilisateur';

describe('analyserArguments', () => {
    it('reads --email', () => {
        expect(analyserArguments(['--email', 'ada@exemple.test']))
            .toEqual({ email: 'ada@exemple.test' });
    });

    it('REFUSES --mot-de-passe on the command line, with its reason', () => {
        // 🔴 The red: accepting it. `ps` exposes the argv of every process to every
        // user of the machine; the password is read from standard
        // input, and there only.
        const r = analyserArguments(['--email', 'ada@exemple.test', '--mot-de-passe', 'secret']);
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/standard input/i);
        // The reason does NOT COPY the secret just refused: that would be
        // rewriting it into a log after refusing it in an argv.
        expect(r.refus).not.toContain('secret');
    });

    it('also refuses the variants of the same flag', () => {
        for (const drapeau of ['--motdepasse', '--password', '-p']) {
            const r = analyserArguments(['--email', 'ada@exemple.test', drapeau, 'secret']);
            expect('refus' in r).toBe(true);
        }
    });

    it('refuses a missing --email, rather than returning undefined', () => {
        // Returning `undefined` would let the caller crash further on, with an
        // unrelated diagnostic.
        const r = analyserArguments([]);
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/--email/);
    });

    it('refuses an empty email', () => {
        expect('refus' in analyserArguments(['--email', ''])).toBe(true);
        expect('refus' in analyserArguments(['--email'])).toBe(true);
    });
});
