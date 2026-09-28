import { describe, expect, it } from 'vitest';
import { blocApres, compounds, declarationsDe, preludes, sansCommentaires } from './css';

/**
 * Les tests du LECTEUR DE FEUILLE — sous-projet ⑥, sous-bloc S4, tâche 1.
 *
 * 🔴 CE MODULE EST L'OUTIL DE TOUS LES GARDES DE FORME DU SOUS-PROJET, et c'est
 * pour cela qu'il est testé à part. `primitives.test.ts` le portait en propre ;
 * les gardes neufs de S4 (§7.10, `style.test.ts`) le réemploient au lieu de le
 * recopier — et une machinerie recopiée diverge sans qu'aucune commande ne le
 * dise.
 *
 * 🔴 LE BLANCHIMENT EST LA PROPRIÉTÉ QUI COMPTE, ET CE DÉPÔT L'A PAYÉE TROIS
 * FOIS : un garde qui cherche une sous-chaîne dans le texte brut est satisfait
 * par le COMMENTAIRE du fichier qu'il analyse — S1 sur `CLE_THEME`, S2 sur G1
 * et G5, S3 sur sa rouge n°16. Le test ① ci-dessous est celui qui la tient.
 */
describe('css.ts — the stylesheet reader', () => {
    it('① a comment declares NOTHING: blanking removes it before analysis', () => {
        // 🔴 LE COMMENTAIRE EST DANS LE BLOC, ET C'EST TOUT CE QUI FAIT LA
        // VALEUR DE CE TEST. Une première rédaction le posait AU-DESSUS de la
        // règle : `declarationsDe` ne lit que l'intérieur des `{ … }`, si bien
        // que la déclaration fantôme n'était de toute façon jamais lue — le
        // test passait VERT sur un blanchiment neutralisé, mesuré. C'est le
        // patron du contrôle vacueux, attrapé ici sur le test lui-même, et le
        // dépôt le paie assez souvent pour qu'il soit écrit à sa place.
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
        // `[^{}]*` ne franchit ni `{` ni `}` : seuls les blocs les plus
        // intérieurs rendent des déclarations.
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
        // 🔴 SANS CETTE LIGNE, LES TESTS CI-DESSUS NE DISENT RIEN DU CAS VIDE, et
        // c'est ce cas-là qui rend VERT un garde d'absence en ne mesurant rien
        // (G5 de `primitives.test.ts`). Le lecteur n'a pas à s'en défendre : ce
        // sont ses APPELANTS qui portent leur assertion d'atteignabilité, et ce
        // test existe pour que cette répartition soit écrite quelque part.
        expect(declarationsDe('')).toEqual([]);
        expect(preludes('')).toEqual([]);
    });
});
