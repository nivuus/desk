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

// ---------------------------------------------------------------------------
// Sous-bloc P2 — le garde n°3 de D5 : la page ne réémet JAMAIS vers l'agent un
// contenu qu'elle vient de recevoir de lui.
// ---------------------------------------------------------------------------

describe('PressePapierLocal.aEmettre — le garde n°3', () => {
    // 🔴 C'EST LE GARDE N°3. ROUGE si `aEmettre` rend toujours son argument :
    // l'agent écrit T dans le presse-papier Windows, le Sondeur le relit, le
    // pousse à la page, la page l'écrit localement — et si l'utilisateur colle
    // alors, la page le renvoie à l'agent. Un aller-retour par collage.
    it('tait un texte qu on vient de recevoir', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.aEmettre('x')).toBeUndefined();
    });

    // ROUGE si le garde bloquait TOUT après une réception. Sans ce test, un
    // `aEmettre` qui rendrait toujours `undefined` passerait le précédent — et
    // le collage ne marcherait plus du tout.
    it('laisse passer un texte différent', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.aEmettre('y')).toBe('y');
    });

    // ROUGE si l'état initial comparait à la chaîne vide : un collage de chaîne
    // vide serait alors muet dès le premier geste.
    it('laisse passer avant toute réception', () => {
        const etat = new PressePapierLocal();
        expect(etat.aEmettre('x')).toBe('x');
        expect(etat.aEmettre('')).toBe('');
    });

    // 🔴 Le garde ne vaut que pour le PREMIER renvoi. Un utilisateur qui colle
    // deux fois le même texte le veut deux fois — et l'agent, lui, ne réécrira
    // pas pour rien : c'est son garde n°2 qui absorbe le doublon, côté VM.
    // ROUGE si le témoin est permanent au lieu d'être consommable.
    it('ne tait que le PREMIER renvoi', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.aEmettre('x')).toBeUndefined();
        expect(etat.aEmettre('x')).toBe('x');
    });

    // ROUGE si le garde prenait un REFUS pour un contenu reçu : rien n'a été
    // écrit localement, donc rien ne peut être un écho.
    it('un refus n arme pas le garde', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: null, octets: 100_000 });
        expect(etat.aEmettre('x')).toBe('x');
    });

    // ROUGE si le témoin était posé par `recevoir` d'un texte QUI N'A PAS ÉTÉ
    // ÉCRIT : un texte reçu sans focus reste en attente, et l'utilisateur peut
    // très bien coller entre-temps un texte identique venu d'ailleurs. Le cas
    // est indiscernable et le choix est de se taire — mais alors le témoin
    // doit venir de `recevoir`, et ce test fige ce choix plutôt que de le
    // laisser dépendre du focus.
    it('arme le garde même quand l écriture locale n a pas encore eu lieu', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.aEcrire(false)).toBeUndefined();
        expect(etat.aEmettre('x')).toBeUndefined();
    });
});
