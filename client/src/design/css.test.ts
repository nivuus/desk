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
describe('css.ts — le lecteur de feuille', () => {
    it('① un commentaire ne déclare RIEN : le blanchiment le retire avant analyse', () => {
        // 🔴 LE COMMENTAIRE EST DANS LE BLOC, ET C'EST TOUT CE QUI FAIT LA
        // VALEUR DE CE TEST. Une première rédaction le posait AU-DESSUS de la
        // règle : `declarationsDe` ne lit que l'intérieur des `{ … }`, si bien
        // que la déclaration fantôme n'était de toute façon jamais lue — le
        // test passait VERT sur un blanchiment neutralisé, mesuré. C'est le
        // patron du contrôle vacueux, attrapé ici sur le test lui-même, et le
        // dépôt le paie assez souvent pour qu'il soit écrit à sa place.
        const css = `
            .a {
                /* padding: 6px; — une valeur citée dans une PROSE, pas une règle */
                padding: var(--e-2);
            }
        `;
        expect(
            declarationsDe(sansCommentaires(css)),
            'une déclaration FANTÔME, lue dans un commentaire, est comptée comme réelle',
        ).toEqual([{ propriete: 'padding', valeur: 'var(--e-2)' }]);
    });

    it('② une déclaration s’extrait avec sa propriété et sa valeur', () => {
        const declarations = declarationsDe('.a { padding: 6px var(--e-3); border: 0 }');
        expect(declarations).toEqual([
            { propriete: 'padding', valeur: '6px var(--e-3)' },
            { propriete: 'border', valeur: '0' },
        ]);
    });

    it('② bis — le corps d’une at-rule n’est jamais pris pour une déclaration', () => {
        // `[^{}]*` ne franchit ni `{` ni `}` : seuls les blocs les plus
        // intérieurs rendent des déclarations.
        expect(declarationsDe('@media (min-width: 30rem) { .a { padding: var(--e-2) } }')).toEqual([
            { propriete: 'padding', valeur: 'var(--e-2)' },
        ]);
    });

    it('③ un sélecteur composé se découpe sur « , » « > » « + » « ~ » et l’espace', () => {
        expect(compounds('.carte > .carte__titre')).toEqual(['.carte', '.carte__titre']);
        expect(compounds('.a+.b~.c d')).toEqual(['.a', '.b', '.c', 'd']);
    });

    it('les préludes rendent tout ce qui précède un « { », at-rules comprises', () => {
        expect(preludes('@media print { .a, .b { padding: 0 } }')).toEqual([
            '@media print',
            '.a, .b',
        ]);
    });

    it('blocApres rend le corps du bloc qui suit, accolades APPARIÉES', () => {
        const css = '@media print { .a { padding: 0 } } .z { border: 0 }';
        expect(blocApres(css, css.indexOf('@media')).trim()).toBe('.a { padding: 0 }');
    });

    it('atteignabilité — le lecteur rend du vide sur du vide, et le dit ici', () => {
        // 🔴 SANS CETTE LIGNE, LES TESTS CI-DESSUS NE DISENT RIEN DU CAS VIDE, et
        // c'est ce cas-là qui rend VERT un garde d'absence en ne mesurant rien
        // (G5 de `primitives.test.ts`). Le lecteur n'a pas à s'en défendre : ce
        // sont ses APPELANTS qui portent leur assertion d'atteignabilité, et ce
        // test existe pour que cette répartition soit écrite quelque part.
        expect(declarationsDe('')).toEqual([]);
        expect(preludes('')).toEqual([]);
    });
});
