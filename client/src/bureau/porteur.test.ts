import { describe, expect, it } from 'vitest';
import { batirEtat, elire, estPlacePrise, fenetresAPeindre, lireEtat } from './porteur';

describe('elire', () => {
    it('sans API de verrou, l onglet devient porteur : le repli est OPTIMISTE', () => {
        // ⚠️ Optimiste et non pessimiste : sans verrou, se declarer suiveur
        // ferait qu AUCUN onglet n ouvrirait jamais la session. La plateforme
        // tranchera, et `estPlacePrise` rattrapera le perdant.
        let role = '';
        elire('v', {
            devenirPorteur: () => { role = 'porteur'; },
            devenirSuiveur: () => { role = 'suiveur'; },
        });
        expect(role).toBe('porteur');
    });

    it('avec l API, le porteur n est proclame QUE lorsque le verrou est obtenu', () => {
        let role = '';
        let relacher: (() => void) | undefined;
        elire('v', {
            // Un verrou qui n appelle JAMAIS `pendant` : le verrou n est pas
            // obtenu, donc cet onglet n est pas porteur.
            verrou: (_nom, pendant) => { relacher = () => void pendant(); },
            devenirPorteur: () => { role = 'porteur'; },
            devenirSuiveur: () => { role = 'suiveur'; },
        });
        expect(role).toBe('suiveur');
        // Puis le verrou se libere : l onglet en attente est promu.
        relacher!();
        expect(role).toBe('porteur');
    });
});

describe('estPlacePrise', () => {
    it('reconnait le motif TYPE', () => {
        expect(estPlacePrise({ type: 'error', reason: 'peu importe', motif: 'role-occupe' })).toBe(true);
    });

    it('ne reconnait PAS un refus d une autre cause', () => {
        // 🔴 CE CAS EST LE POINT : un frein de volume doit rester VISIBLE.
        // L avaler ferait de ce lot la panne muette qu il pretend eviter.
        expect(estPlacePrise({ type: 'error', reason: 'trop de requetes', motif: 'trop-de-requetes' })).toBe(false);
    });

    it('ne reconnait PAS un refus sans motif, meme si sa phrase le dit', () => {
        // ⚠️ Le piege de F1 : une phrase francaise se reformule. On ne
        // devine pas, on lit le motif -- ou on affiche.
        expect(estPlacePrise({ type: 'error', reason: 'un client est deja connecte a la session s' })).toBe(false);
    });

    it('ne leve pas sur une entree qui n est pas un objet', () => {
        expect(estPlacePrise(undefined)).toBe(false);
        expect(estPlacePrise('role-occupe')).toBe(false);
    });
});

describe('lireEtat', () => {
    it('rend la liste d un etat bien forme', () => {
        const etat = batirEtat([{ session: 's', titre: 'Bloc-notes', ouverte: true }]);
        expect(lireEtat(etat)?.[0]?.titre).toBe('Bloc-notes');
    });

    it('rend undefined sur un message d un AUTRE emetteur', () => {
        // Un `BroadcastChannel` est partage par origine : tout ce qui y passe
        // n est pas forcement de nous.
        expect(lireEtat({ type: 'autre-chose', fenetres: [] })).toBeUndefined();
    });

    it('rend undefined quand `fenetres` n est pas un tableau', () => {
        expect(lireEtat({ type: 'etat-bureau', fenetres: 'trois' })).toBeUndefined();
    });

    it('ECARTE une entree mal formee au lieu de la laisser passer', () => {
        const lu = lireEtat({
            type: 'etat-bureau',
            fenetres: [{ session: 's', titre: 'bon', ouverte: false }, { session: 42 }],
        });
        expect(lu?.length).toBe(1);
    });
});

describe('fenetresAPeindre', () => {
    it('un suiveur qui n a encore rien recu ne peint rien', () => {
        expect(fenetresAPeindre('suiveur', [], undefined)).toEqual([]);
    });

    it(
        'un suiveur qui a recu N fenetres les garde au tour de minuterie suivant, ' +
            'MEME QUAND SA PROPRE LISTE EST VIDE',
        () => {
            // 🔴 C EST CE CAS QUI ATTRAPE LE DEFAUT DU ROUND 1 (critique ①) :
            // la minuterie repeint a 1 Hz depuis `bureau.liste()`, qui est
            // STRUCTURELLEMENT VIDE chez un suiveur -- aucun socket, donc
            // aucun `fenetreOuverte` ne l alimente jamais. Une regle qui
            // peindrait `listePropre` chez un suiveur effacerait donc, au
            // tour SUIVANT une diffusion, ce qu elle venait de montrer.
            const recues = [{ session: 's', titre: 'Bloc-notes', ouverte: true }];
            expect(fenetresAPeindre('suiveur', [], recues)).toEqual(recues);
        },
    );

    it('le porteur peint TOUJOURS sa propre liste, jamais un etat recu perime', () => {
        const propre = [{ session: 's', titre: 'Bloc-notes', ouverte: false }];
        const recuPerime = [{ session: 's', titre: 'Bloc-notes', ouverte: true }];
        expect(fenetresAPeindre('porteur', propre, recuPerime)).toEqual(propre);
    });
});
