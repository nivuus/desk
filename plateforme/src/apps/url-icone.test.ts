// The rule of the signed icon URL, tested WITHOUT a server or database: the module
// is pure, its clock is a parameter, and that is what lets it be besieged
// on both sides of its expiry bound.
//
// 🔴 THE THREE SECURITY REDS OF THIS BATCH ARE HERE, AND THEY WERE SEEN
// RED: a forged signature is refused, an expired URL is refused,
// a URL signed for one application is not valid for another. A check
// never seen red is not a check.

import { createHmac } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { DUREE_JETON_ACCES_MS } from '../identite/jeton';
import {
    DUREE_URL_ICONE_MS,
    PAS_URL_ICONE_MS,
    signature,
    signerUrlIcone,
    iconSubkey,
    verifyIconUrl,
} from './url-icone';

/// The SAME public fixture as `http/routes-harnais.ts`, and for the same
/// reason: it protects nothing, it only satisfies the minimal
/// length of a platform secret.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const MS = 1_787_136_773_742;
const APP = '11111111-2222-3333-4444-555555555555';
const VM = 'vm-1';
const EMPREINTE = 'a'.repeat(64);

function paramsOf(url: string): URLSearchParams {
    return new URL(url, 'http://interne').searchParams;
}

describe("the signed icon URL", () => {
    it('returns a relative path, carrying the four parameters', () => {
        const url = signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS);
        expect(url.startsWith(`/application/${APP}/icone?`)).toBe(true);
        const p = paramsOf(url);
        expect(p.get('e')).toBe(EMPREINTE);
        expect(p.get('v')).toBe(VM);
        // The expiry is ROUNDED up to the next step — see `PAS_URL_ICONE_MS`.
        const x = Number(p.get('x'));
        expect(x % PAS_URL_ICONE_MS).toBe(0);
        expect(x).toBeGreaterThanOrEqual(MS + DUREE_URL_ICONE_MS);
        expect(x).toBeLessThan(MS + DUREE_URL_ICONE_MS + PAS_URL_ICONE_MS);
        expect(p.get('s')).toMatch(/^[A-Za-z0-9_-]{43}$/);
    });

    it('verifies, and returns the VM it carries', () => {
        const v = verifyIconUrl(APP, paramsOf(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS)), SECRET, MS);
        expect(v).toEqual({ ok: true, vm: VM });
    });

    /* ── RED ①: THE FORGED SIGNATURE ─────────────────────────────── */

    it('🔴 REFUSES a forged signature — one byte is enough', () => {
        const p = paramsOf(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        const vraie = p.get('s')!;
        // A SINGLE character changes, and it really changes: `a` -> `b`.
        const faux = (vraie[0] === 'a' ? 'b' : 'a') + vraie.slice(1);
        expect(faux).not.toBe(vraie);
        p.set('s', faux);
        expect(verifyIconUrl(APP, p, SECRET, MS)).toEqual({
            ok: false,
            motif: 'signature-invalide',
        });
    });

    it('🔴 REFUSES a signature from ANOTHER key — the subkey is not the secret', () => {
        // 🔴 IT IS THE CHECK THAT SAYS THE DERIVATION IS GOOD FOR SOMETHING.
        // A signature computed with the RAW token secret — that is,
        // what one would have written without deriving — must be REFUSED. Without it,
        // replacing `iconSubkey(secret)` with `secret` would leave everything green.
        const expiration = String(MS + DUREE_URL_ICONE_MS);
        const message = ['v1', `${String(APP.length)}:${APP}`, `${String(VM.length)}:${VM}`,
            `${String(expiration.length)}:${expiration}`].join('\n');
        const brute = createHmac('sha256', SECRET).update(message, 'utf8').digest('base64url');
        const p = new URLSearchParams({ e: EMPREINTE, v: VM, x: expiration, s: brute });
        expect(verifyIconUrl(APP, p, SECRET, MS)).toEqual({
            ok: false,
            motif: 'signature-invalide',
        });
        // And the POSITIVE witness, without which the refusal above would prove
        // nothing: the SAME URL, signed by the subkey, is accepted.
        p.set('s', signature({ application: APP, vm: VM, expiration }, SECRET));
        expect(verifyIconUrl(APP, p, SECRET, MS).ok).toBe(true);
    });

    it('the subkey is NOT the secret, and it is stable', () => {
        const a = iconSubkey(SECRET);
        expect(a.length).toBe(32);
        expect(a.toString('utf8')).not.toBe(SECRET);
        expect(a.equals(iconSubkey(SECRET))).toBe(true);
        expect(a.equals(iconSubkey(`${SECRET}-autre`))).toBe(false);
    });

    /* ── RED ②: THE EXPIRED URL ──────────────────────────────────────── */

    it('🔴 REFUSES an EXPIRED URL, and the bound is besieged from BOTH sides', () => {
        const p = paramsOf(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        const x = Number(p.get('x'));
        // One millisecond before: still good.
        expect(verifyIconUrl(APP, p, SECRET, x - 1).ok).toBe(true);
        // At the EXACT instant: the bound is strict, the URL is dead.
        expect(verifyIconUrl(APP, p, SECRET, x)).toEqual({ ok: false, motif: 'url-expiree' });
        // Well after: same.
        expect(verifyIconUrl(APP, p, SECRET, x + 3_600_000)).toEqual({
            ok: false,
            motif: 'url-expiree',
        });
    });

    it('🔴 a forged AND stale URL is told « signature », never « expired »', () => {
        // 🔴 THE ORDER OF THE CHECKS IS A SECURITY PROPERTY: saying
        // "expired" to a forger would tell them their signature was
        // good. The signature check therefore comes BEFORE the time check.
        const p = paramsOf(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        p.set('s', 'z'.repeat(43));
        expect(verifyIconUrl(APP, p, SECRET, MS + 10_000_000)).toEqual({
            ok: false,
            motif: 'signature-invalide',
        });
    });

    it('🔴 a NON-INTEGER expiry, although signed, is refused', () => {
        // 🔴 `Number('x')` returns `NaN`, and `maintenant >= NaN` is FALSE: without
        // this guard, an unreadable expiry would be ETERNAL. The only path
        // that reaches it is a signature computed with the REAL key — so
        // this test computes it, rather than leaving the guard green by
        // construction.
        for (const x of ['not-a-number', '1.5', 'Infinity']) {
            const p = new URLSearchParams({
                e: EMPREINTE,
                v: VM,
                x,
                s: signature({ application: APP, vm: VM, expiration: x }, SECRET),
            });
            expect(verifyIconUrl(APP, p, SECRET, MS), x).toEqual({
                ok: false,
                motif: 'signature-invalide',
            });
        }
    });

    /* ── RED ③: A URL IS ONLY VALID FOR ITS SCOPE ─────────────────── */

    it('🔴 a URL signed for one application IS NOT VALID for another', () => {
        const p = paramsOf(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        const autre = '99999999-8888-7777-6666-555555555555';
        expect(verifyIconUrl(autre, p, SECRET, MS)).toEqual({
            ok: false,
            motif: 'signature-invalide',
        });
        // Witness: the SAME URL, on ITS application, is accepted.
        expect(verifyIconUrl(APP, p, SECRET, MS).ok).toBe(true);
    });

    it('🔴 a URL signed for one VM IS NOT VALID for another', () => {
        const p = paramsOf(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        p.set('v', 'vm-2');
        expect(verifyIconUrl(APP, p, SECRET, MS)).toEqual({
            ok: false,
            motif: 'signature-invalide',
        });
    });

    it('🔴 the fields do not SLIDE into one another', () => {
        // 🔴 THE LENGTH PREFIX EXISTS FOR THIS: without it, a simple
        // `join('\n')` would make an application named `x\n3:vm` and a VM
        // `abc` produce the same message as another pair. We test
        // that the two pairs yield DIFFERENT signatures.
        const a = signature({ application: 'x', vm: 'y|z', expiration: '1' }, SECRET);
        const b = signature({ application: 'x|y', vm: 'z', expiration: '1' }, SECRET);
        expect(a).not.toBe(b);
        const c = signature({ application: 'x\n3:abc', vm: 'q', expiration: '1' }, SECRET);
        const d = signature({ application: 'x', vm: 'abc', expiration: '1' }, SECRET);
        expect(c).not.toBe(d);
    });

    /* ── LA FORME ─────────────────────────────────────────────────────── */

    it('refuses an absent or empty parameter, without confusing it with a wrong signature', () => {
        const base = paramsOf(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        for (const nom of ['v', 'x', 's']) {
            const manque = new URLSearchParams(base);
            manque.delete(nom);
            expect(verifyIconUrl(APP, manque, SECRET, MS), `without ${nom}`).toEqual({
                ok: false,
                motif: 'parametre-absent',
            });
            const vide = new URLSearchParams(base);
            vide.set(nom, '');
            expect(verifyIconUrl(APP, vide, SECRET, MS), `${nom} empty`).toEqual({
                ok: false,
                motif: 'parametre-absent',
            });
        }
    });

    /* ── THE DURATION ─────────────────────────────────────────────────────── */

    it('🔴 two strikes in the SAME minute give the SAME URL — otherwise the cache is dead', () => {
        // 🔴 IT IS THE PROPERTY THAT SAVES `Cache-Control: immutable`. Without
        // the rounding, `x` and `s` would change at every keystroke, the cache key
        // too, and no entry would ever be read again.
        const a = signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS);
        const b = signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS + 1_000);
        expect(b).toBe(a);
        // And the NEGATIVE witness, without which the equality above could
        // come from an ignored clock: one step further, the URL DIFFERS.
        expect(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS + PAS_URL_ICONE_MS)).not.toBe(a);
    });

    it('🔴 the duration FLOOR is guaranteed, over a whole step', () => {
        // The rounding is UPWARDS: at no instant of the step does the lifetime
        // drop below `DUREE_URL_ICONE_MS`.
        for (let d = 0; d < PAS_URL_ICONE_MS; d += 997) {
            const t = MS + d;
            const x = Number(paramsOf(signerUrlIcone(APP, VM, EMPREINTE, SECRET, t)).get('x'));
            expect(x - t, `at +${String(d)} ms`).toBeGreaterThanOrEqual(DUREE_URL_ICONE_MS);
            expect(x - t, `at +${String(d)} ms`).toBeLessThan(
                DUREE_URL_ICONE_MS + PAS_URL_ICONE_MS,
            );
        }
    });

    it('🔴 NEVER outlives the bearer token that gave birth to it', () => {
        // 🔴 IT IS THE WRITTEN REASON FOR THE VALUE, PINNED RATHER THAN LEFT
        // IN A COMMENT. The two constants are recalibrated TOGETHER:
        // lowering the access token below five minutes would make the bound wrong,
        // and without this test nobody would see it.
        expect(DUREE_URL_ICONE_MS).toBeLessThanOrEqual(DUREE_JETON_ACCES_MS);
        expect(DUREE_URL_ICONE_MS).toBe(300_000);
    });
});
