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
 * DEMONSTRATION texts, never the real files. These tests exercise
 * the FUNCTIONS; that the tree complies is exercised by
 * `client/outils/classes-employees.mjs`, which reads it.
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
        // 🔴 WITHOUT SETTING THE BODIES ASIDE, this check would become permissive: a
        // length `.5rem` or a `content: ".x"` would read as a declared
        // class, and any typo would end up being
        // "declared" somewhere.
        const css = `.a { margin: .5rem; content: ".fantome"; background: url(x.y); }`;
        expect([...classesDeclarees(css)]).toEqual(['a']);
    });

    it('BLANKS the comments before searching', () => {
        // The pattern this repository has paid for three times: a guard satisfied by the
        // comment of the file it analyses.
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
        // 🔴 MANDATORY: `client/design.html` declares inline six classes that
        // `galerie.ts` uses. Omitting them would make §7.9 RED on
        // correct code, that is, pressure to loosen it.
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
        // 🔴 FOUND BY RUNNING THE CHECK ON `classes.ts` ITSELF. A regex
        // that carries quotes broke the parity of the string tracking: all
        // the rest of the file was read as a string, so NO
        // comment was blanked any more, and the check returned `UNDECLARED`
        // on the prose of a comment. It is the pattern "a guard satisfied
        // by the comment of the file it analyses", in reverse.
        //
        // ⚠️ THE VECTOR CARRIES AN ODD NUMBER OF QUOTES, AND THAT IS THE
        // CONDITION FOR IT TO DISCRIMINATE. A first draft used
        // `/['"]([^'"]*)['"]/g` — SIX quotes, so a parity that closes
        // by itself: the mutation that removes regex recognition
        // then left this test GREEN. A perturbation that perturbs
        // nothing reads exactly like a check that does not bite.
        const ts = `
            const morceaux = texte.split(/[',]/);
            // el.className = 'fantome-apres-regex';
            el.className = 'reelle';
        `;
        expect([...classesEmployeesTs(ts)]).toEqual(['reelle']);
    });

    it("does NOT blank what lives in a string — the silent false negative", () => {
        // ⚠️ A naively blanked `'https://x'` would lose the end of its line, and
        // the class written after it would become invisible: a false NEGATIVE,
        // so exactly the typo §7.9 exists to catch,
        // but silent.
        const ts = `const u = 'https://exemple.test'; el.className = 'apres-url';`;
        expect([...classesEmployeesTs(ts)]).toEqual(['apres-url']);
    });
});

describe('the blankers, separately', () => {
    it('sansCommentairesHtml keeps the line breaks', () => {
        // `<!--x` is FIVE characters and `y-->` is FOUR: the count
        // is that of the blanked characters, not an approximation. This
        // assertion was written wrong a first time, and it was
        // the implementation that was right.
        expect(sansCommentairesHtml('a<!--x\ny-->b')).toBe('a     \n    b');
    });

    it('sansCommentairesTs keeps the line breaks', () => {
        expect(sansCommentairesTs('a//x\nb')).toBe('a   \nb');
    });
});
