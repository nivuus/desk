import { describe, it, expect } from 'vitest';
import {
    PressePapierLocal,
    MESSAGE_ECHEC,
    ECHECS_AVANT_MESSAGE,
} from './presse-papier';

describe('PressePapierLocal', () => {
    it('rend le texte reçu quand la fenêtre est focalisée', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'bonjour', octets: 7 });
        expect(pp.aEcrire(true)).toBe('bonjour');
    });

    // 🔴 Le dépôt différé : écrire sans focus ferait rendre le texte au
    // premier appel, et ce test tombe.
    it('ne rend rien sans focus, puis rend le texte au retour du focus', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'bonjour', octets: 7 });
        expect(pp.aEcrire(false)).toBeUndefined();
        expect(pp.aEcrire(true)).toBe('bonjour');
    });

    // 🔴 « Une écriture obsolète est impossible » : empiler dans un tableau
    // ferait sortir le PREMIER, et ce test le voit.
    it('deux réceptions sans focus : le DERNIER texte sort, jamais une file', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'ancien', octets: 6 });
        pp.recevoir({ texte: 'recent', octets: 6 });
        expect(pp.aEcrire(true)).toBe('recent');
        pp.confirmer('recent');
        expect(pp.aEcrire(true)).toBeUndefined();
    });

    it('ne réécrit pas ce qui a déjà été écrit', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'bonjour', octets: 7 });
        pp.confirmer('bonjour');
        expect(pp.aEcrire(true)).toBeUndefined();
    });

    // 🔴 Crier au PREMIER échec ferait un bandeau permanent sur un produit
    // qui marche : le premier échec est le cas ordinaire d'une fenêtre sans
    // focus.
    it('ne dit rien au premier échec, et parle au second', () => {
        const pp = new PressePapierLocal();
        expect(pp.echouer()).toBeUndefined();
        expect(pp.echouer()).toBe(MESSAGE_ECHEC);
        expect(ECHECS_AVANT_MESSAGE).toBe(2);
    });

    it('un succès remet le compteur d’échecs à zéro', () => {
        const pp = new PressePapierLocal();
        expect(pp.echouer()).toBeUndefined();
        pp.confirmer('quelque chose');
        expect(pp.echouer()).toBeUndefined();
    });

    // 🔴 Le message dit COMMENT rétablir, pas seulement que quelque chose
    // manque — même règle que le micro. L'assertion porte sur une
    // sous-chaîne d'INSTRUCTION, jamais sur la seule présence d'un message.
    it('le message d’échec dit comment rétablir', () => {
        expect(MESSAGE_ECHEC).toContain('cliquez');
        expect(MESSAGE_ECHEC).toContain('focus');
    });

    // 🔴 Le critère ③ : écrire quand même, ou taire le refus, fait tomber ce
    // test.
    it('un refus n’écrit rien et se dit, en nommant la taille', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: null, octets: 102400 });
        expect(pp.aEcrire(true)).toBeUndefined();
        const message = pp.refusADire();
        expect(message).toBeDefined();
        expect(message).toContain('100');
    });

    it('un refus n’écrase pas le dernier texte mémorisé', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'valide', octets: 6 });
        pp.recevoir({ texte: null, octets: 102400 });
        expect(pp.aEcrire(true)).toBe('valide');
    });

    // 🔴 Sans consommation, le bandeau se réafficherait à chaque tour.
    it('le refus se consomme : deux appels ne rendent qu’un message', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: null, octets: 102400 });
        expect(pp.refusADire()).toBeDefined();
        expect(pp.refusADire()).toBeUndefined();
    });
});
