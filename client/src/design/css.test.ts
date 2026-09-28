import { describe, expect, it } from 'vitest';
import { blocApres, compounds, declarationsDe, preludes, sansCommentaires } from './css';

/**
 * The tests of the STYLESHEET READER — sub-project ⑥, sub-block S4, task 1.
 *
 * 🔴 THIS MODULE IS THE TOOL OF ALL THE SUB-PROJECT'S SHAPE GUARDS, and that is
 * why it is tested on its own. `primitives.test.ts` carried it privately;
 * the new guards of S4 (§7.10, `style.test.ts`) reuse it instead of
 * copying it — and copied machinery drifts without any command
 * saying so.
 *
 * 🔴 BLANKING IS THE PROPERTY THAT MATTERS, AND THIS REPOSITORY HAS PAID FOR IT THREE
 * TIMES: a guard that looks for a substring in the raw text is satisfied
 * by the COMMENT of the file it analyses — S1 on `CLE_THEME`, S2 on G1
 * and G5, S3 on its red no. 16. Test ① below is the one that holds it.
 */
describe('css.ts — the stylesheet reader', () => {
    it('① a comment declares NOTHING: blanking removes it before analysis', () => {
        // 🔴 THE COMMENT IS INSIDE THE BLOCK, AND THAT IS ALL THAT GIVES THIS TEST ITS
        // VALUE. A first draft placed it ABOVE the
        // rule: `declarationsDe` only reads the inside of `{ … }`, so
        // the ghost declaration was never read anyway — the
        // test passed GREEN on a neutralised blanking, measured. It is the
        // pattern of the vacuous check, caught here on the test itself, and the
        // repository pays for it often enough for it to be written in its place.
        const css = `
            .a {
                /* padding: 6px; — a value quoted in PROSE, not a rule */
                padding: var(--e-2);
            }
        `;
        expect(
            declarationsDe(sansCommentaires(css)),
            'a GHOST declaration, read in a comment, is counted as real',
        ).toEqual([{ propriete: 'padding', value: 'var(--e-2)' }]);
    });

    it('② a declaration is extracted with its property and its value', () => {
        const declarations = declarationsDe('.a { padding: 6px var(--e-3); border: 0 }');
        expect(declarations).toEqual([
            { propriete: 'padding', value: '6px var(--e-3)' },
            { propriete: 'border', value: '0' },
        ]);
    });

    it('② bis — the body of an at-rule is never taken for a declaration', () => {
        // `[^{}]*` crosses neither `{` nor `}`: only the innermost blocks
        // return declarations.
        expect(declarationsDe('@media (min-width: 30rem) { .a { padding: var(--e-2) } }')).toEqual([
            { propriete: 'padding', value: 'var(--e-2)' },
        ]);
    });

    it('③ a compound selector splits on « , » « > » « + » « ~ » and the space', () => {
        expect(compounds('.carte > .carte__titre')).toEqual(['.carte', '.carte__titre']);
        expect(compounds('.a+.b~.c d')).toEqual(['.a', '.b', '.c', 'd']);
    });

    it('the preludes return everything before a « { », at-rules included', () => {
        expect(preludes('@media print { .a, .b { padding: 0 } }')).toEqual([
            '@media print',
            '.a, .b',
        ]);
    });

    it('blocApres returns the body of the following block, braces MATCHED', () => {
        const css = '@media print { .a { padding: 0 } } .z { border: 0 }';
        expect(blocApres(css, css.indexOf('@media')).trim()).toBe('.a { padding: 0 }');
    });

    it('reachability — the reader returns emptiness on emptiness, and says so here', () => {
        // 🔴 WITHOUT THIS LINE, THE TESTS ABOVE SAY NOTHING ABOUT THE EMPTY CASE, and
        // it is that case that makes an absence guard GREEN by measuring nothing
        // (G5 of `primitives.test.ts`). The reader does not have to defend against it: it is
        // its CALLERS that carry their reachability assertion, and this
        // test exists so that this split is written down somewhere.
        expect(declarationsDe('')).toEqual([]);
        expect(preludes('')).toEqual([]);
    });
});
