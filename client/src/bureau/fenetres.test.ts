import { describe, expect, it } from 'vitest';
import { lignes, sectionVisible } from './fenetres';

describe('lignes', () => {
    it('une fenetre OUVERTE n est pas rouvrable : il n y a rien a suggerer', () => {
        expect(lignes([{ session: 's', titre: 'Bloc-notes', ouverte: true }])[0].rouvrable).toBe(false);
    });

    it('une fenetre FERMEE est rouvrable', () => {
        expect(lignes([{ session: 's', titre: 'Paint', ouverte: false }])[0].rouvrable).toBe(true);
    });

    it('le booleen ouverte distingue les deux etats : c est lui qui pilote la classe cote cablage', () => {
        // 🔴 LA REGLE PURE NE DECIDE PLUS DU NOM DE CLASSE : un nom qui
        // transiterait par une variable serait invisible au controle §7.9,
        // qui ne voit que les litteraux passes a `classList.add('…')`. Ce que
        // la regle pure decide, c est l ETAT ; le nom litteral de la classe
        // vit dans fenetres-dom.ts.
        const ouverte = lignes([{ session: 'a', titre: 'x', ouverte: true }])[0];
        const fermee = lignes([{ session: 'b', titre: 'x', ouverte: false }])[0];
        expect([ouverte.ouverte, fermee.ouverte]).toEqual([true, false]);
    });

    it('le mot de la pastille est accentue du cote FERME', () => {
        // `fermée`, pas `fermee` : c est du texte montre a un humain.
        expect(lignes([{ session: 's', titre: 'x', ouverte: false }])[0].etat).toBe('fermée');
    });

    it('la session est reconduite telle quelle : c est elle qui rouvre', () => {
        expect(lignes([{ session: 's-7', titre: 'x', ouverte: false }])[0].session).toBe('s-7');
    });

    it('l ordre des fenetres est PRESERVE', () => {
        const rendu = lignes([
            { session: 'a', titre: 'Un', ouverte: true },
            { session: 'b', titre: 'Deux', ouverte: false },
        ]);
        expect(rendu.map((l) => l.titre)).toEqual(['Un', 'Deux']);
    });
});

describe('sectionVisible', () => {
    it('aucune fenetre : la section est ABSENTE', () => {
        // 🔴 ABSENTE, PAS VIDE. Une section montrant en permanence « aucune
        // fenetre ouverte » serait du bruit sur l etat NOMINAL d un hub qu on
        // vient d ouvrir (spec 4.1).
        expect(sectionVisible([])).toBe(false);
    });

    it('une fenetre, meme FERMEE : la section est visible', () => {
        // Une fenetre fermee a quelque chose a offrir -- son bouton Rouvrir --
        // donc la cacher priverait l utilisateur du seul geste qui la ramene.
        expect(sectionVisible([{ session: 's', titre: 'x', ouverte: false }])).toBe(true);
    });
});
