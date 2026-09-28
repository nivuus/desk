import { describe, expect, it } from 'vitest';
import { plan, verdict, type Tranche } from './tranches';

/**
 * ⚠️ CE QUI JUGE CE FICHIER EST UNE MUTATION NOMMÉE : remplacer `Math.ceil` par
 * `Math.floor` dans `plan`. La dernière tranche disparaît, et `verdict`
 * déclare alors `complet` un fichier TRONQUÉ. Les tests qui la tuent portent le
 * marqueur 🔴 dans leur titre — ce sont ceux qui rattachent le découpage à la
 * `size` par un chemin INDÉPENDANT de `plan` : compter des tranches ou
 * sommer leurs octets contre le nombre attendu, jamais contre ce que `plan`
 * vient de rendre. Un test qui comparerait `plan` à lui-même resterait vert
 * sous la mutation.
 */

/** Somme des octets d'un découpage — l'invariant central, calculé à part. */
function somme(tranches: Tranche[]): number {
    return tranches.reduce((total, t) => total + t.octets, 0);
}

describe('plan', () => {
    it('returns ZERO chunks for an empty file, never an empty chunk', () => {
        // Un fichier vide est légitime : il n'a rien à déposer. Fabriquer une
        // tranche de zéro octet obligerait le déposant à envoyer une trame sans
        // contenu pour sceller un fichier sans contenu.
        expect(plan(0, 4)).toEqual([]);
    });

    it.each([
        [0, 4],
        [1, 4],
        [4, 4],
        [8, 4],
        [10, 4],
        [10, 1],
        [7, 3],
    ])(
        '🔴 the sum of the bytes is EXACTLY the size (size=%i, step=%i)',
        (size, pas) => {
            // La comparaison porte sur `size`, PAS sur `plan` : c'est ce qui
            // rend l'assertion capable de voir une queue de fichier perdue.
            expect(somme(plan(size, pas))).toBe(size);
        },
    );

    it.each([
        [0, 4, 0],
        [1, 4, 1],
        [4, 4, 1],
        [8, 4, 2],
        [10, 4, 3],
        [9, 3, 3],
    ])(
        '🔴 the number of chunks is the ceiling of the quotient (size=%i, step=%i → %i)',
        (size, pas, combien) => {
            // Le nombre attendu est écrit à la main, jamais recalculé : un
            // `Math.ceil` dans le test reproduirait le défaut qu'il cherche.
            expect(plan(size, pas)).toHaveLength(combien);
        },
    );

    it("does NOT make an empty final chunk when the size is an exact multiple of the step", () => {
        const tranches = plan(8, 4);
        expect(tranches).toEqual([
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ]);
        expect(tranches.some((t) => t.octets === 0)).toBe(false);
    });

    it("returns ONE single chunk, of the file size, when the file is smaller than the step", () => {
        expect(plan(3, 4)).toEqual([{ n: 0, octets: 3 }]);
    });

    it('numbers the ranks from ZERO and contiguously, so that the position can be computed', () => {
        const pas = 4;
        const tranches = plan(10, pas);
        expect(tranches.map((t) => t.n)).toEqual([0, 1, 2]);
        // La propriété que la base zéro achète : `n * pas` EST la position, sans
        // table ni décalage à retenir de chaque côté du pont.
        const positions = tranches.map((t) => t.n * pas);
        expect(positions).toEqual([0, 4, 8]);
    });

    it('shortens ONLY the last chunk', () => {
        const tranches = plan(10, 4);
        expect(tranches.slice(0, -1).every((t) => t.octets === 4)).toBe(true);
        expect(tranches[tranches.length - 1].octets).toBe(2);
    });

    it.each<[string, number, number]>([
        ['a zero step — which would also make the plan loop forever', 10, 0],
        ['a negative step', 10, -1],
        ['a non-integer step', 10, 2.5],
        ['a NaN step', 10, Number.NaN],
        ['a negative size', -1, 4],
        ['a non-integer size', 2.5, 4],
        ['a NaN size', Number.NaN, 4],
    ])('THROWS on an invalid contract: %s', (_titre, size, pas) => {
        // Le contrat est détenu par l'appelant, pas reçu du fil : un contrat
        // absurde est un défaut de programme, et le rendre sous forme de
        // verdict le déguiserait en anomalie de transfert.
        expect(() => plan(size, pas)).toThrow(/chunks:/);
    });
});

describe('verdict', () => {
    it('returns complete when all the expected chunks are there, at the right size', () => {
        expect(verdict(10, 4, plan(10, 4))).toEqual({ etat: 'complet' });
    });

    it("returns complete whatever the ORDER of arrival of the chunks", () => {
        // Rien ne garantit que le déposant émette dans l'ordre, ni que le
        // réseau les rende dans l'ordre.
        const desordre = [...plan(10, 4)].reverse();
        expect(verdict(10, 4, desordre)).toEqual({ etat: 'complet' });
    });

    it('names a hole in the middle', () => {
        expect(verdict(12, 4, [
            { n: 0, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'manquantes', n: [1] });
    });

    it("🔴 refuses to declare complete a TRUNCATED file: the last chunk is missing", () => {
        // C'est le cas exact que la mutation `ceil → floor` rend invisible.
        // Avec `floor`, le plan de 10 octets par 4 ne compterait que deux
        // tranches, les deux ci-dessous suffiraient, et deux octets seraient
        // perdus SANS QU'AUCUNE TRACE NE LE DISE.
        expect(verdict(10, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'manquantes', n: [2] });
    });

    it.each([
        [10, 4],
        [7, 3],
        [1, 4],
        [9, 2],
    ])(
        "🔴 an upload cut off from its last chunk is NEVER complete (size=%i, step=%i)",
        (size, pas) => {
            const ampute = plan(size, pas).slice(0, -1);
            // La somme déposée est strictement inférieure à la taille : c'est
            // la formulation la plus directe de « le fichier est tronqué ».
            expect(somme(ampute)).toBeLessThan(size);
            expect(verdict(size, pas, ampute).etat).toBe('manquantes');
        },
    );

    it('names all the chunks when nothing has been dropped', () => {
        expect(verdict(10, 4, [])).toEqual({ etat: 'manquantes', n: [0, 1, 2] });
    });

    it('declares INCONSISTENT a chunk SHORTER than planned, not missing', () => {
        // Un transfert amputé : la redemander rendrait la même chose.
        expect(verdict(12, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 3 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [1] });
    });

    it('declares INCONSISTENT a chunk LONGER than planned', () => {
        // Elle déborderait sur sa voisine.
        expect(verdict(12, 4, [
            { n: 0, octets: 5 },
            { n: 1, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('declares INCONSISTENT the last chunk sent at the size of the step', () => {
        // Le piège naturel du déposant : remplir la queue jusqu'au pas.
        expect(verdict(10, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [2] });
    });

    it('declares INCONSISTENT a rank beyond the plan', () => {
        expect(verdict(8, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [2] });
    });

    it('declares INCONSISTENT a negative rank', () => {
        expect(verdict(8, 4, [
            { n: -1, octets: 4 },
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [-1] });
    });

    it('declares INCONSISTENT a non-integer rank, and returns it AS IS', () => {
        // La liste doit montrer ce qui a réellement été envoyé, pas une valeur
        // nettoyée : c'est ce qu'un journal a besoin de lire.
        expect(verdict(8, 4, [{ n: 1.5, octets: 4 }])).toEqual({
            etat: 'incoherentes',
            n: [1.5],
        });
    });

    it('declares INCONSISTENT a non-integer byte count', () => {
        expect(verdict(8, 4, [
            { n: 0, octets: 4.5 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('declares INCONSISTENT a DUPLICATE whose two occurrences agree on the size', () => {
        // 🔴 Deux dépôts pour le même rang : l'un a écrasé l'autre, et rien ici
        // ne peut savoir lequel a gagné. Deux trames de même longueur ne
        // portent pas forcément le même contenu.
        expect(verdict(8, 4, [
            { n: 0, octets: 4 },
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('declares INCONSISTENT a duplicate whose occurrences diverge', () => {
        expect(verdict(8, 4, [
            { n: 1, octets: 4 },
            { n: 1, octets: 2 },
            { n: 0, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [1] });
    });

    it('🔴 makes incoherentes PREVAIL over manquantes when both are present', () => {
        // Annoncer d'abord le trou ferait recompléter la tranche absente, puis
        // re-vérifier, puis retomber sur la même incohérence — la boucle exacte
        // que la distinction existe pour empêcher.
        const rendu = verdict(12, 4, [{ n: 0, octets: 9 }]);
        expect(rendu).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('returns complete for an empty file of which nothing has been dropped', () => {
        expect(verdict(0, 4, [])).toEqual({ etat: 'complet' });
    });

    it('declares INCONSISTENT the slightest chunk dropped for an empty file', () => {
        expect(verdict(0, 4, [{ n: 0, octets: 0 }])).toEqual({
            etat: 'incoherentes',
            n: [0],
        });
    });

    it('sorts the ranks NUMERICALLY and without duplicates', () => {
        // ⚠️ Onze tranches : c'est le seuil à partir duquel le tri par défaut
        // de JavaScript, qui compare des CHAÎNES, rendrait `[0, 10, 2, …]`.
        const rendu = verdict(44, 4, [{ n: 1, octets: 4 }]);
        expect(rendu).toEqual({
            etat: 'manquantes',
            n: [0, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        });
    });

    it('does not repeat an inconsistent rank reported several times', () => {
        const rendu = verdict(12, 4, [
            { n: 2, octets: 1 },
            { n: 2, octets: 1 },
            { n: 2, octets: 1 },
        ]);
        expect(rendu.etat).toBe('incoherentes');
        expect((rendu as { n: number[] }).n).toEqual([2]);
    });

    it.each<[string, number, number]>([
        ['a zero step', 10, 0],
        ['a negative size', -1, 4],
    ])('THROWS on an invalid contract, like plan: %s', (_titre, size, pas) => {
        // La garde est la MÊME des deux côtés : un contrat qui ne tient pas ne
        // doit pas produire un verdict d'apparence normale.
        expect(() => verdict(size, pas, [])).toThrow(/chunks:/);
    });

    it('NEVER throws on what comes from the wire, however absurd', () => {
        // Un pair déréglé ou malveillant ne doit pas pouvoir faire tomber le
        // vérificateur : tout ce qui vient de `presentes` devient un verdict.
        expect(() =>
            verdict(8, 4, [
                { n: -7, octets: Number.NaN },
                { n: 1e9, octets: -3 },
                { n: 0.5, octets: Infinity },
            ]),
        ).not.toThrow();
    });
});
