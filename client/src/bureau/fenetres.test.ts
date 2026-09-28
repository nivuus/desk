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
        // 🔴 LA REGLE PURE NE DECIDE PLUS DU NOM DE CLASSE : un nom qui
        // transiterait par une variable serait invisible au controle §7.9,
        // qui ne voit que les litteraux passes a `classList.add('…')`. Ce que
        // la regle pure decide, c est l ETAT ; le nom litteral de la classe
        // vit dans fenetres-dom.ts.
        const ouverte = lignes([{ session: 'a', titre: 'x', ouverte: true }])[0];
        const fermee = lignes([{ session: 'b', titre: 'x', ouverte: false }])[0];
        expect([ouverte.ouverte, fermee.ouverte]).toEqual([true, false]);
    });

    it('the badge word is emphasised on the CLOSED side', () => {
        // `fermée`, pas `fermee` : c est du texte montre a un humain.
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
        // 🔴 ABSENTE, PAS VIDE. Une section montrant en permanence « aucune
        // fenetre ouverte » serait du bruit sur l etat NOMINAL d un hub qu on
        // vient d ouvrir (spec 4.1).
        expect(sectionVisible([])).toBe(false);
    });

    it('a window, even CLOSED: the section is visible', () => {
        // Une fenetre fermee a quelque chose a offrir -- son bouton Rouvrir --
        // donc la cacher priverait l utilisateur du seul geste qui la ramene.
        expect(sectionVisible([{ session: 's', titre: 'x', ouverte: false }])).toBe(true);
    });
});
