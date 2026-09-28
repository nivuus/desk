// ⚠️ **`?raw` et non `node:fs`** : `client/` n'a pas `@types/node`, et un test
// écrit avec `readFileSync` passerait sous vitest — qui transpile par esbuild,
// sans vérifier les types — tout en CASSANT `npm run typecheck`. Piège relevé
// par le sous-bloc P2 de la plateforme, et payé ici une seconde fois.
//
// `?raw` exige `test.css: true` dans `client/vite.config.ts` pour les feuilles
// de style ; sur un fichier `.rs` il n'y a pas de court-circuit à lever, et le
// test ci-dessous vérifie de toute façon que le texte lu n'est pas vide.
import rustPressePapier from '../../agent/src/presse_papier.rs?raw';
import { describe, it, expect } from 'vitest';
import {
    PressePapierLocal,
    MESSAGE_ECHEC,
    FAILURES_BEFORE_MESSAGE,
    PRESSE_PAPIER_MAX,
} from './presse-papier';

describe('PressePapierLocal', () => {
    it('returns the received text when the window is focused', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'bonjour', octets: 7 });
        expect(pp.toWrite(true)).toBe('bonjour');
    });

    // 🔴 Le dépôt différé : écrire sans focus ferait rendre le texte au
    // premier appel, et ce test tombe.
    it('returns nothing without focus, then returns the text when focus comes back', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'bonjour', octets: 7 });
        expect(pp.toWrite(false)).toBeUndefined();
        expect(pp.toWrite(true)).toBe('bonjour');
    });

    // 🔴 « Une écriture obsolète est impossible » : empiler dans un tableau
    // ferait sortir le PREMIER, et ce test le voit.
    it('two receptions without focus: the LAST text comes out, never a queue', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'ancien', octets: 6 });
        pp.recevoir({ texte: 'recent', octets: 6 });
        expect(pp.toWrite(true)).toBe('recent');
        pp.confirmer('recent');
        expect(pp.toWrite(true)).toBeUndefined();
    });

    it('does not rewrite what was already written', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'bonjour', octets: 7 });
        pp.confirmer('bonjour');
        expect(pp.toWrite(true)).toBeUndefined();
    });

    // 🔴 Crier au PREMIER échec ferait un bandeau permanent sur un produit
    // qui marche : le premier échec est le cas ordinaire d'une fenêtre sans
    // focus.
    it('says nothing on the first failure, and speaks on the second', () => {
        const pp = new PressePapierLocal();
        expect(pp.echouer()).toBeUndefined();
        expect(pp.echouer()).toBe(MESSAGE_ECHEC);
        expect(FAILURES_BEFORE_MESSAGE).toBe(2);
    });

    it('a success resets the failure counter to zero', () => {
        const pp = new PressePapierLocal();
        expect(pp.echouer()).toBeUndefined();
        pp.confirmer('something');
        expect(pp.echouer()).toBeUndefined();
    });

    // 🔴 Le message dit COMMENT rétablir, pas seulement que quelque chose
    // manque — même règle que le micro. L'assertion porte sur une
    // sous-chaîne d'INSTRUCTION, jamais sur la seule présence d'un message.
    it('the failure message says how to recover', () => {
        expect(MESSAGE_ECHEC).toContain('click');
        expect(MESSAGE_ECHEC).toContain('focus');
    });

    // 🔴 Le critère ③ : écrire quand même, ou taire le refus, fait tomber ce
    // test.
    it('a refusal writes nothing and says so, naming the size', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: null, octets: 102400 });
        expect(pp.toWrite(true)).toBeUndefined();
        const message = pp.refusADire();
        expect(message).toBeDefined();
        expect(message).toContain('100');
    });

    it('a refusal does not overwrite the last remembered text', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'valide', octets: 6 });
        pp.recevoir({ texte: null, octets: 102400 });
        expect(pp.toWrite(true)).toBe('valide');
    });

    // 🔴 Sans consommation, le bandeau se réafficherait à chaque tour.
    it('the refusal is consumed: two calls return only one message', () => {
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

describe('PressePapierLocal.aEmettre — guard no. 3', () => {
    // 🔴 C'EST LE GARDE N°3. ROUGE si `aEmettre` rend toujours son argument :
    // l'agent écrit T dans le presse-papier Windows, le Sondeur le relit, le
    // pousse à la page, la page l'écrit localement — et si l'utilisateur colle
    // alors, la page le renvoie à l'agent. Un aller-retour par collage.
    it('silences a text that was just received', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.aEmettre('x')).toBeUndefined();
    });

    // ROUGE si le garde bloquait TOUT après une réception. Sans ce test, un
    // `aEmettre` qui rendrait toujours `undefined` passerait le précédent — et
    // le collage ne marcherait plus du tout.
    it('lets a different text through', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.aEmettre('y')).toBe('y');
    });

    // ROUGE si l'état initial comparait à la chaîne vide : un collage de chaîne
    // vide serait alors muet dès le premier geste.
    it('lets through before any reception', () => {
        const etat = new PressePapierLocal();
        expect(etat.aEmettre('x')).toBe('x');
        expect(etat.aEmettre('')).toBe('');
    });

    // 🔴 Le garde ne vaut que pour le PREMIER renvoi. Un utilisateur qui colle
    // deux fois le même texte le veut deux fois — et l'agent, lui, ne réécrira
    // pas pour rien : c'est son garde n°2 qui absorbe le doublon, côté VM.
    // ROUGE si le témoin est permanent au lieu d'être consommable.
    it('silences only the FIRST echo', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.aEmettre('x')).toBeUndefined();
        expect(etat.aEmettre('x')).toBe('x');
    });

    // ROUGE si le garde prenait un REFUS pour un contenu reçu : rien n'a été
    // écrit localement, donc rien ne peut être un écho.
    it('a refusal does not arm the guard', () => {
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
    it('arms the guard even when the local write has not happened yet', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.toWrite(false)).toBeUndefined();
        expect(etat.aEmettre('x')).toBeUndefined();
    });
});

/// 🔴 **LA BORNE EST ÉCRITE DANS DEUX LANGAGES QU'AUCUN `import` NE RELIE**, et
/// ce test est la seule chose qui les empêche de diverger en silence.
///
/// Le dépôt a payé cette classe au sous-bloc P2 de la plateforme, et le remède
/// employé est le même : **relire le fichier source de l'autre langage** plutôt
/// que d'espérer qu'on pensera aux deux.
///
/// ROUGE si l'une des deux valeurs bouge sans l'autre.
describe('PRESSE_PAPIER_MAX', () => {
    it("is worth what the Rust agent declares", () => {
        const trouve = /pub const PRESSE_PAPIER_MAX: usize = ([^;]+);/.exec(rustPressePapier);
        // ⚠️ Sans cette assertion, un renommage côté Rust rendrait `trouve`
        // nul et le test passerait en ne mesurant RIEN — le contrôle vacueux
        // que ce dépôt paie depuis D7.
        expect(trouve, "the Rust constant was not found").not.toBeNull();
        // eslint-disable-next-line no-eval
        const rustValue = Number(new Function(`return ${trouve![1].replace(/_/g, '')}`)());
        expect(PRESSE_PAPIER_MAX).toBe(rustValue);
    });
});
