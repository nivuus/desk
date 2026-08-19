import { describe, expect, it } from 'vitest';
import { Appariement } from './appariement';

describe('Appariement', () => {
    it("refuse un second occupant du même rôle, avec le motif d'aujourd'hui", () => {
        const a = new Appariement<string>();
        expect(a.declarer('s', 'agent', 'sock-1')).toBeUndefined();
        // Le motif est repris MOT POUR MOT de l'ex-`server.ts:119-125` : le
        // changer casserait un pair qui le lit. `agent/src/signaling.rs:130`
        // journalise `reason` ; le navigateur le remonte dans une Error
        // (`webrtc.ts:109`).
        expect(a.declarer('s', 'agent', 'sock-2'))
            .toBe('un agent est déjà connecté à la session s');
    });

    it('isole les sessions entre elles', () => {
        const a = new Appariement<string>();
        a.declarer('s1', 'agent', 'a1');
        expect(a.declarer('s2', 'agent', 'a2')).toBeUndefined();
        expect(a.pair('s1', 'client')).toBe('a1');
        expect(a.pair('s2', 'client')).toBe('a2');
        // Un client de s1 n'atteint pas l'agent de s2.
        a.declarer('s1', 'client', 'c1');
        expect(a.pair('s2', 'agent')).toBeUndefined();
    });

    it("retient la dernière offre et l'oublie quand elle est prise", () => {
        const a = new Appariement<string>();
        a.retenirOffre('s', 'v=0 premiere');
        expect(a.prendreOffre('s')).toBe('v=0 premiere');
        expect(a.prendreOffre('s')).toBeUndefined();
    });

    it('la dernière offre écrase les précédentes', () => {
        const a = new Appariement<string>();
        a.retenirOffre('s', 'v=0 vieille');
        a.retenirOffre('s', 'v=0 fraiche');
        expect(a.prendreOffre('s')).toBe('v=0 fraiche');
    });

    it('oublie la session quand ses deux pairs sont partis', () => {
        const a = new Appariement<string>();
        a.declarer('s', 'agent', 'a');
        a.declarer('s', 'client', 'c');
        expect(a.retirer('s', 'agent')).toEqual({ vide: false });
        expect(a.retirer('s', 'client')).toEqual({ vide: true });
        // La session ayant été oubliée, le rôle agent redevient libre.
        expect(a.declarer('s', 'agent', 'a2')).toBeUndefined();
    });
});
