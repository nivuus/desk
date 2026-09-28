import { describe, expect, it } from 'vitest';
import { Appariement } from './appariement';

describe('Appariement', () => {
    it("refuses a second occupant of the same role, with today's reason", () => {
        const a = new Appariement<string>();
        expect(a.declarer('s', 'agent', 'sock-1')).toBeUndefined();
        // Le motif est repris MOT POUR MOT de l'ex-`server.ts:119-125` : le
        // changer casserait un pair qui le lit. `agent/src/signaling.rs:139`
        // journalise `reason` ; le navigateur le remonte dans une Error
        // (`webrtc.ts:130-131`).
        //
        // ❌ Ces deux numéros valaient `:130` et `:109`, et les DEUX étaient
        // faux — le second depuis P2, le premier depuis le commit `5fbc89b`
        // de P3. Corrigés à la revue transverse de fin de branche P3, ICI
        // **et** dans `appariement.ts`, où la même paire vivait : « corrigé à
        // sa place » est une affirmation de COMPLÉTUDE, et les places ont été
        // énumérées par `grep -n` avant d'écrire.
        expect(a.declarer('s', 'agent', 'sock-2'))
            .toBe('an agent is already connected to session s');
    });

    it('isolates the sessions from one another', () => {
        const a = new Appariement<string>();
        a.declarer('s1', 'agent', 'a1');
        expect(a.declarer('s2', 'agent', 'a2')).toBeUndefined();
        expect(a.pair('s1', 'client')).toBe('a1');
        expect(a.pair('s2', 'client')).toBe('a2');
        // Un client de s1 n'atteint pas l'agent de s2.
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
        // La session ayant été oubliée, le rôle agent redevient libre.
        expect(a.declarer('s', 'agent', 'a2')).toBeUndefined();
    });
});
