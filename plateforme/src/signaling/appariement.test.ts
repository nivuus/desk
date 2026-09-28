import { describe, expect, it } from 'vitest';
import { Appariement } from './appariement';

describe('Appariement', () => {
    it("refuses a second occupant of the same role, with today's reason", () => {
        const a = new Appariement<string>();
        expect(a.declarer('s', 'agent', 'sock-1')).toBeUndefined();
        // The reason is taken WORD FOR WORD from the former `server.ts:119-125`:
        // changing it would break a peer that reads it. `agent/src/signaling.rs:139`
        // logs `reason`; the browser raises it in an Error
        // (`webrtc.ts:130-131`).
        //
        // ❌ These two numbers were `:130` and `:109`, and BOTH were
        // wrong — the second since P2, the first since commit `5fbc89b`
        // of P3. Fixed at the cross-cutting review at the end of branch P3, HERE
        // **and** in `appariement.ts`, where the same pair lived: "fixed in
        // its place" is a claim of COMPLETENESS, and the places were
        // enumerated by `grep -n` before writing.
        expect(a.declarer('s', 'agent', 'sock-2'))
            .toBe('an agent is already connected to session s');
    });

    it('isolates the sessions from one another', () => {
        const a = new Appariement<string>();
        a.declarer('s1', 'agent', 'a1');
        expect(a.declarer('s2', 'agent', 'a2')).toBeUndefined();
        expect(a.pair('s1', 'client')).toBe('a1');
        expect(a.pair('s2', 'client')).toBe('a2');
        // A client of s1 does not reach the agent of s2.
        a.declarer('s1', 'client', 'c1');
        expect(a.pair('s2', 'agent')).toBeUndefined();
    });

    it("keeps the last offer and forgets it when it is taken", () => {
        const a = new Appariement<string>();
        a.retenirOffre('s', 'v=0 premiere');
        expect(a.prendreOffre('s')).toBe('v=0 premiere');
        expect(a.prendreOffre('s')).toBeUndefined();
    });

    it('the last offer overwrites the previous ones', () => {
        const a = new Appariement<string>();
        a.retenirOffre('s', 'v=0 old');
        a.retenirOffre('s', 'v=0 fresh');
        expect(a.prendreOffre('s')).toBe('v=0 fresh');
    });

    it('forgets the session when both its peers have left', () => {
        const a = new Appariement<string>();
        a.declarer('s', 'agent', 'a');
        a.declarer('s', 'client', 'c');
        expect(a.retirer('s', 'agent')).toEqual({ vide: false });
        expect(a.retirer('s', 'client')).toEqual({ vide: true });
        // The session having been forgotten, the agent role becomes free again.
        expect(a.declarer('s', 'agent', 'a2')).toBeUndefined();
    });
});
