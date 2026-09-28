import { describe, expect, it } from 'vitest';
import {
    classesDeclarees,
    classesDeclareesEnLigne,
    classesEmployeesHtml,
    classesEmployeesTs,
    sansCommentairesHtml,
    sansCommentairesTs,
} from './classes';

/**
 * Des textes de DÉMONSTRATION, jamais les vrais fichiers. Ces tests éprouvent
 * les FONCTIONS ; que l'arbre soit conforme est éprouvé par
 * `client/outils/classes-employees.mjs`, qui le lit.
 */

describe('classesDeclarees', () => {
    it('reads the classes of simple and compound selectors', () => {
        const css = `
            .bouton { color: red; }
            .carte .carte__titre { font-weight: 600; }
            .champ__saisie:focus-visible { outline: 1px solid; }
            .message::before { content: ""; }
        `;
        expect([...classesDeclarees(css)].sort()).toEqual([
            'bouton',
            'carte',
            'carte__titre',
            'champ__saisie',
            'message',
        ]);
    });

    it('goes down into @media and reads the classes there', () => {
        const css = `
            @media (min-width: 40rem) {
                .grille { display: grid; }
            }
        `;
        expect([...classesDeclarees(css)]).toEqual(['grille']);
    });

    it("invents NO class from a declaration body", () => {
        // 🔴 SANS L'ÉCART DES CORPS, ce contrôle deviendrait permissif : une
        // longueur `.5rem` ou un `content: ".x"` se lirait comme une classe
        // déclarée, et n'importe quelle faute de frappe finirait par se
        // trouver « déclarée » quelque part.
        const css = `.a { margin: .5rem; content: ".fantome"; background: url(x.y); }`;
        expect([...classesDeclarees(css)]).toEqual(['a']);
    });

    it('BLANKS the comments before searching', () => {
        // Le patron que ce dépôt a payé trois fois : un garde satisfait par le
        // commentaire du fichier qu'il analyse.
        const css = `/* .bouton--principale is a typo */ .bouton { color: red; }`;
        expect([...classesDeclarees(css)]).toEqual(['bouton']);
    });
});

describe('classesEmployeesHtml', () => {
    it('reads a class attribute with several values, double or single quotes', () => {
        const html = `<div class="carte carte--large"></div><p class='message message--danger'></p>`;
        expect([...classesEmployeesHtml(html)].sort()).toEqual([
            'carte',
            'carte--large',
            'message',
            'message--danger',
        ]);
    });

    it('BLANKS the HTML comments before searching', () => {
        const html = `<!-- <div class="fantome"></div> --><div class="reelle"></div>`;
        expect([...classesEmployeesHtml(html)]).toEqual(['reelle']);
    });

    it("does not confuse an attribute whose NAME ends with « class »", () => {
        const html = `<div data-class="fantome" class="reelle"></div>`;
        expect([...classesEmployeesHtml(html)]).toEqual(['reelle']);
    });
});

describe('classesDeclareesEnLigne', () => {
    it('reads the classes of inline <style> blocks', () => {
        // 🔴 OBLIGATOIRE : `client/design.html` déclare en ligne six classes que
        // `galerie.ts` emploie. Les omettre ferait naître §7.9 ROUGE sur du
        // code correct, c'est-à-dire la pression à l'assouplissement.
        const html = `<style>.pastille { color: red; } .barre { width: 0; }</style>`;
        expect([...classesDeclareesEnLigne(html)].sort()).toEqual(['barre', 'pastille']);
    });

    it("returns an EMPTY set when the page has no <style>", () => {
        expect([...classesDeclareesEnLigne('<div class="x"></div>')]).toEqual([]);
    });
});

describe('classesEmployeesTs', () => {
    it('reads classList.add and className, with one or several arguments', () => {
        const ts = `
            el.classList.add('bouton', 'bouton--discret');
            autre.className = 'pastille pastille--ouverte';
        `;
        expect([...classesEmployeesTs(ts)].sort()).toEqual([
            'bouton',
            'bouton--discret',
            'pastille',
            'pastille--ouverte',
        ]);
    });

    it('BLANKS the TypeScript comments before searching', () => {
        const ts = `
            // el.classList.add('fantome-ligne');
            /* el.className = 'fantome-bloc'; */
            el.classList.add('reelle');
        `;
        expect([...classesEmployeesTs(ts)]).toEqual(['reelle']);
    });

    it("is not unbalanced by a REGEX LITERAL", () => {
        // 🔴 TROUVÉ EN LANÇANT LE CONTRÔLE SUR `classes.ts` LUI-MÊME. Une regex
        // qui porte des guillemets rompait la parité du suivi de chaîne : tout
        // le reste du fichier était lu comme une chaîne, donc plus AUCUN
        // commentaire n'y était blanchi, et le contrôle rendait `NON DÉCLARÉE`
        // sur la prose d'un commentaire. C'est le patron « un garde satisfait
        // par le commentaire du fichier qu'il analyse », en creux.
        //
        // ⚠️ LE VECTEUR PORTE UN NOMBRE IMPAIR DE GUILLEMETS, ET C'EST LA
        // CONDITION POUR QU'IL DISCRIMINE. Une première rédaction employait
        // `/['"]([^'"]*)['"]/g` — SIX guillemets, donc une parité qui se
        // referme toute seule : la mutation qui retire la reconnaissance des
        // regex laissait alors ce test VERT. Une perturbation qui ne perturbe
        // rien se lit exactement comme un contrôle qui ne mord pas.
        const ts = `
            const morceaux = texte.split(/[',]/);
            // el.className = 'fantome-apres-regex';
            el.className = 'reelle';
        `;
        expect([...classesEmployeesTs(ts)]).toEqual(['reelle']);
    });

    it("does NOT blank what lives in a string — the silent false negative", () => {
        // ⚠️ Un `'https://x'` blanchi naïvement perdrait la fin de sa ligne, et
        // la classe écrite après lui deviendrait invisible : un faux NÉGATIF,
        // donc exactement la faute de frappe que §7.9 existe pour attraper,
        // mais silencieuse.
        const ts = `const u = 'https://exemple.test'; el.className = 'apres-url';`;
        expect([...classesEmployeesTs(ts)]).toEqual(['apres-url']);
    });
});

describe('the blankers, separately', () => {
    it('sansCommentairesHtml keeps the line breaks', () => {
        // `<!--x` vaut CINQ caractères et `y-->` en vaut QUATRE : le compte
        // est celui des caractères blanchis, pas une approximation. Cette
        // assertion a été écrite fausse une première fois, et c'est
        // l'implémentation qui avait raison.
        expect(sansCommentairesHtml('a<!--x\ny-->b')).toBe('a     \n    b');
    });

    it('sansCommentairesTs keeps the line breaks', () => {
        expect(sansCommentairesTs('a//x\nb')).toBe('a   \nb');
    });
});
