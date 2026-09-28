// The complete handshake rule: who passes, who is refused, with
// which reason.

import { describe, expect, it } from 'vitest';
import { signer, DUREE_JETON_ACCES_MS } from './jeton';
import { ProprieteDeSession } from '../signaling/propriete';
import { garde } from './garde';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const T0 = 1_787_000_000_000;

/// Two prefixes of the REAL length `agents/prefixe.ts` produces — 22
/// base64url characters. A short string would not measure the same thing.
const P = 'RhH1x2QmTz9kLpVbNc7dAw';
const Q = 'Zk4pQw8sXt2vBn6mLr0eYu';

function neuve(maintenant: () => number = () => T0) {
    const proprietes = new ProprieteDeSession();
    return { proprietes, g: garde(SECRET, maintenant, proprietes) };
}

describe('handshake guard', () => {
    it('🔴 REFUSES an `agent` peer WITHOUT a token — the P2 window is CLOSED', () => {
        // 🔴 THIS ASSERTION IS THE EXACT INVERSE OF THE ONE P2 SHIPPED, and
        // it is P3's central gesture. P2 wrote here "accepts an `agent`
        // peer WITHOUT a token", announcing in its own comment that
        // "the day P3 inverts it, this test will have to be rewritten ON PURPOSE,
        // not by surprise". It is done, and on purpose.
        //
        // 🔴 THE RED IS FREE: P2's binary carries it. `garde.ts`
        // opened with `if (role === 'agent') return { ok: true };`, and a peer
        // that declared itself `{"role":"agent"}` got TURN credentials
        // valid for 86,400 s without presenting the slightest identity.
        const { g } = neuve();
        const v = g.verify({ role: 'agent', session: 'bureau' });
        expect(v.ok).toBe(false);
        if (v.ok) return;
        expect(v.motif).toBe('jeton-absent');
    });

    it('accepts an `agent` whose token PREFIXES the requested session', () => {
        const { g } = neuve();
        const jeton = signer(P, SECRET, T0, DUREE_JETON_ACCES_MS, 'agent');
        expect(g.verify({ role: 'agent', session: `${P}:bureau`, jeton }))
            .toEqual({ ok: true });
        // The same VM on one of ITS windows.
        expect(g.verify({ role: 'agent', session: `${P}:w-1`, jeton }).ok).toBe(true);
    });

    it('🔴 REFUSES the SAME agent token on the session of ANOTHER VM', () => {
        // 🔴 The red: omitting the prefix comparison. An enrolled agent
        // would then occupy the session of ANY other VM — it is the `agent`
        // counterpart of P2's criterion ③, and without it enrolment only authenticates
        // the existence of a VM, never WHICH one.
        const { g } = neuve();
        const jeton = signer(P, SECRET, T0, DUREE_JETON_ACCES_MS, 'agent');
        const refus = g.verify({ role: 'agent', session: `${Q}:bureau`, jeton });
        expect(refus).toMatchObject({ ok: false, motif: 'session-refusee' });
        if (refus.ok) return;
        // The message ON THE WIRE does not distinguish causes: it is the same
        // as that of a client refused for ownership. The LOG, for its part,
        // carries what is needed to diagnose.
        expect(refus.message).toBe('access refused to the requested session');
        expect(refus.journal).toContain(`${Q}:bureau`);
    });

    it('🔴 REFUSES a prefix that is only a START of the subject, without the separator', () => {
        // 🔴 The red: comparing by `session.startsWith(sujet)` WITHOUT the
        // separator. An agent with prefix `AB` would then occupy the sessions
        // of VM `ABC`, whose prefix extends it — a collision that would
        // only happen between two specific VMs, hence never in testing and
        // always in production.
        const { g } = neuve();
        const jeton = signer('AB', SECRET, T0, DUREE_JETON_ACCES_MS, 'agent');
        expect(g.verify({ role: 'agent', session: 'ABC:bureau', jeton }))
            .toMatchObject({ ok: false, motif: 'session-refusee' });
    });

    it('🔴 REFUSES a HUMAN token presented as `role:agent` — confusion, direction 1', () => {
        // 🔴 The red: omitting `type === 'agent'`. Both tokens are signed
        // by the SAME secret: a stolen human token would open an `agent` role,
        // hence an `ice-config` on any session whose name it prefixed.
        //
        // ⚠️ TWO DIRECTIONS OF CONFUSION, TWO TESTS, never a single one with two
        // assertions: `expect` interrupts at the first, and the second would
        // be tested by nothing. It is P2's lesson ①A-bis, applied
        // in advance.
        const { g } = neuve();
        const humain = signer(P, SECRET, T0);
        expect(g.verify({ role: 'agent', session: `${P}:bureau`, jeton: humain }))
            .toMatchObject({ ok: false, motif: 'session-refusee' });
    });

    it('🔴 REFUSES an AGENT token presented as `role:client` — confusion, direction 2', () => {
        // 🔴 The red: omitting `type !== 'agent'`. An agent token would open
        // a `client` role, bypassing the session ownership P2
        // set (`signaling/propriete.ts`): the agent would become a
        // user, on anyone's session.
        const { g } = neuve();
        const agent = signer(P, SECRET, T0, DUREE_JETON_ACCES_MS, 'agent');
        expect(g.verify({ role: 'client', session: `${P}:bureau`, jeton: agent }))
            .toMatchObject({ ok: false, motif: 'session-refusee' });
    });

    it('REFUSES a `client` without a token', () => {
        const { g } = neuve();
        const v = g.verify({ role: 'client', session: 's-1' });
        expect(v.ok).toBe(false);
        if (v.ok) return;
        expect(v.motif).toBe('jeton-absent');
    });

    it('REFUSES a malformed or badly signed token', () => {
        const { g } = neuve();
        expect(g.verify({ role: 'client', session: 's-1', jeton: 'not.a.token' }))
            .toMatchObject({ ok: false, motif: 'jeton-invalide' });
        const autre = signer('u1', 'ANOTHER-secret-of-forty-characters-or-more', T0);
        expect(g.verify({ role: 'client', session: 's-1', jeton: autre }))
            .toMatchObject({ ok: false, motif: 'jeton-invalide' });
    });

    it('REFUSES a token of an unexpected type, without ever THROWING', () => {
        // A `String(jeton)` without a check would pass an object off as a
        // string, or would throw on `null`.
        const { g } = neuve();
        for (const jeton of [42, { sub: 'u1' }, null, [], true]) {
            expect(() => g.verify({ role: 'client', session: 's-1', jeton }))
                .not.toThrow();
            expect(g.verify({ role: 'client', session: 's-1', jeton }).ok).toBe(false);
        }
    });

    it('accepts a `client` with a valid token on a FREE session', () => {
        const { g } = neuve();
        const jeton = signer('u1', SECRET, T0);
        expect(g.verify({ role: 'client', session: 's-1', jeton }))
            .toEqual({ ok: true, userId: 'u1' });
    });

    it('accepts the SAME user on THEIR session, and REFUSES another', () => {
        const { g, proprietes } = neuve();
        proprietes.revendiquer('s-1', 'u1');
        expect(g.verify({ role: 'client', session: 's-1', jeton: signer('u1', SECRET, T0) }))
            .toEqual({ ok: true, userId: 'u1' });

        const refus = g.verify({
            role: 'client',
            session: 's-1',
            jeton: signer('u2', SECRET, T0),
        });
        expect(refus).toMatchObject({ ok: false, motif: 'session-refusee' });
        if (refus.ok) return;
        // 🔴 The message ON THE WIRE names neither the owner nor the existence
        // of the session: that would be an oracle. The LOG, for its part, carries the session
        // name and the requester's identifier — it is what separates a
        // diagnosis from an oracle.
        expect(refus.message).not.toContain('u1');
        expect(refus.message).not.toContain('s-1');
        expect(refus.journal).toContain('s-1');
        expect(refus.journal).toContain('u2');
    });

    it('REFUSES an EXPIRED token, on a clock that VARIES', () => {
        // 🔴 Criterion ② lived end to end: the SAME token, two instants.
        // Freezing the clock would make the second call green.
        let maintenant = T0;
        const { g } = neuve(() => maintenant);
        const jeton = signer('u1', SECRET, T0);
        expect(g.verify({ role: 'client', session: 's-1', jeton }))
            .toEqual({ ok: true, userId: 'u1' });
        maintenant = T0 + DUREE_JETON_ACCES_MS;
        expect(g.verify({ role: 'client', session: 's-1', jeton }))
            .toMatchObject({ ok: false, motif: 'jeton-expire' });
    });

    it('verify HAS NO SIDE EFFECT: two calls claim nothing', () => {
        // 🔴 Putting the claim there would leave a GHOST ownership
        // behind a peer that `Appariement::declarer` then refuses
        // because the role is already taken.
        const { g, proprietes } = neuve();
        const jeton = signer('u1', SECRET, T0);
        g.verify({ role: 'client', session: 's-1', jeton });
        g.verify({ role: 'client', session: 's-1', jeton });
        expect(proprietes.proprietaire('s-1')).toBeUndefined();
        // And ANOTHER user still passes, proof that nothing was set.
        expect(g.verify({ role: 'client', session: 's-1', jeton: signer('u2', SECRET, T0) }).ok)
            .toBe(true);
    });

    it('claim then release: the session changes hands', () => {
        const { g, proprietes } = neuve();
        g.revendiquer('s-1', 'u1');
        expect(proprietes.proprietaire('s-1')).toBe('u1');
        // An `agent` has no identifier: it claims nothing.
        g.revendiquer('s-2', undefined);
        expect(proprietes.proprietaire('s-2')).toBeUndefined();
        g.liberer('s-1');
        expect(proprietes.proprietaire('s-1')).toBeUndefined();
    });
});
