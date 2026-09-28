import { describe, expect, it } from 'vitest';
import { lignes, sectionVisible } from './fenetres';

describe('lignes', () => {
    it('an OPEN window cannot be reopened: there is nothing to suggest', () => {
        expect(lignes([{ session: 's', titre: 'Bloc-notes', ouverte: true }])[0].rouvrable).toBe(false);
    });

    it('a CLOSED window can be reopened', () => {
        expect(lignes([{ session: 's', titre: 'Paint', ouverte: false }])[0].rouvrable).toBe(true);
    });

    it('the open boolean tells the two states apart: it drives the class on the wiring side', () => {
        // 🔴 THE PURE RULE NO LONGER DECIDES THE CLASS NAME: a name that
        // went through a variable would be invisible to check §7.9,
        // which only sees the literals passed to `classList.add('…')`. What
        // the pure rule decides is the STATE; the literal class name
        // lives in fenetres-dom.ts.
        const ouverte = lignes([{ session: 'a', titre: 'x', ouverte: true }])[0];
        const fermee = lignes([{ session: 'b', titre: 'x', ouverte: false }])[0];
        expect([ouverte.ouverte, fermee.ouverte]).toEqual([true, false]);
    });

    it('the badge word is emphasised on the CLOSED side', () => {
        // `closed` is text shown to a human, not an internal code.
        expect(lignes([{ session: 's', titre: 'x', ouverte: false }])[0].etat).toBe('closed');
    });

    it('the session is carried over as is: it is the one that reopens', () => {
        expect(lignes([{ session: 's-7', titre: 'x', ouverte: false }])[0].session).toBe('s-7');
    });

    it('the order of the windows is PRESERVED', () => {
        const rendu = lignes([
            { session: 'a', titre: 'Un', ouverte: true },
            { session: 'b', titre: 'Deux', ouverte: false },
        ]);
        expect(rendu.map((l) => l.titre)).toEqual(['Un', 'Deux']);
    });
});

describe('sectionVisible', () => {
    it('no window: the section is ABSENT', () => {
        // 🔴 ABSENT, NOT EMPTY. A section permanently showing "no
        // open window" would be noise on the NOMINAL state of a hub that was
        // just opened (spec 4.1).
        expect(sectionVisible([])).toBe(false);
    });

    it('a window, even CLOSED: the section is visible', () => {
        // A closed window has something to offer -- its Reopen button --
        // so hiding it would deprive the user of the only gesture that brings it back.
        expect(sectionVisible([{ session: 's', titre: 'x', ouverte: false }])).toBe(true);
    });
});
