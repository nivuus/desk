import { describe, expect, it } from 'vitest';
import { plan, verdict, type Tranche } from './tranches';

/**
 * ⚠️ CE QUI JUGE CE FICHIER EST UNE MUTATION NOMMÉE : remplacer `Math.ceil` par
 * `Math.floor` dans `plan`. La dernière tranche disparaît, et `verdict`
 * déclare alors `complet` un fichier TRONQUÉ. Les tests qui la tuent portent le
 * marqueur 🔴 dans leur titre — ce sont ceux qui rattachent le découpage à la
 * `taille` par un chemin INDÉPENDANT de `plan` : compter des tranches ou
 * sommer leurs octets contre le nombre attendu, jamais contre ce que `plan`
 * vient de rendre. Un test qui comparerait `plan` à lui-même resterait vert
 * sous la mutation.
 */

/** Somme des octets d'un découpage — l'invariant central, calculé à part. */
function somme(tranches: Tranche[]): number {
    return tranches.reduce((total, t) => total + t.octets, 0);
}

describe('plan', () => {
    it('rend ZÉRO tranche pour un fichier vide, jamais une tranche vide', () => {
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
        '🔴 la somme des octets vaut EXACTEMENT la taille (taille=%i, pas=%i)',
        (taille, pas) => {
            // La comparaison porte sur `taille`, PAS sur `plan` : c'est ce qui
            // rend l'assertion capable de voir une queue de fichier perdue.
            expect(somme(plan(taille, pas))).toBe(taille);
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
        '🔴 le nombre de tranches est le plafond du quotient (taille=%i, pas=%i → %i)',
        (taille, pas, combien) => {
            // Le nombre attendu est écrit à la main, jamais recalculé : un
            // `Math.ceil` dans le test reproduirait le défaut qu'il cherche.
            expect(plan(taille, pas)).toHaveLength(combien);
        },
    );

    it("ne fabrique PAS de tranche finale vide quand la taille est un multiple exact du pas", () => {
        const tranches = plan(8, 4);
        expect(tranches).toEqual([
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ]);
        expect(tranches.some((t) => t.octets === 0)).toBe(false);
    });

    it("rend UNE seule tranche, de la taille du fichier, quand celui-ci est plus petit que le pas", () => {
        expect(plan(3, 4)).toEqual([{ n: 0, octets: 3 }]);
    });

    it('numérote les rangs à BASE ZÉRO et de façon contiguë, si bien que la position se calcule', () => {
        const pas = 4;
        const tranches = plan(10, pas);
        expect(tranches.map((t) => t.n)).toEqual([0, 1, 2]);
        // La propriété que la base zéro achète : `n * pas` EST la position, sans
        // table ni décalage à retenir de chaque côté du pont.
        const positions = tranches.map((t) => t.n * pas);
        expect(positions).toEqual([0, 4, 8]);
    });

    it('ne raccourcit QUE la dernière tranche', () => {
        const tranches = plan(10, 4);
        expect(tranches.slice(0, -1).every((t) => t.octets === 4)).toBe(true);
        expect(tranches[tranches.length - 1].octets).toBe(2);
    });

    it.each<[string, number, number]>([
        ['un pas nul — qui ferait aussi boucler le plan sans fin', 10, 0],
        ['un pas négatif', 10, -1],
        ['un pas non entier', 10, 2.5],
        ['un pas NaN', 10, Number.NaN],
        ['une taille négative', -1, 4],
        ['une taille non entière', 2.5, 4],
        ['une taille NaN', Number.NaN, 4],
    ])('LÈVE sur un contrat invalide : %s', (_titre, taille, pas) => {
        // Le contrat est détenu par l'appelant, pas reçu du fil : un contrat
        // absurde est un défaut de programme, et le rendre sous forme de
        // verdict le déguiserait en anomalie de transfert.
        expect(() => plan(taille, pas)).toThrow(/tranches :/);
    });
});

describe('verdict', () => {
    it('rend complet quand toutes les tranches attendues sont là, à la bonne taille', () => {
        expect(verdict(10, 4, plan(10, 4))).toEqual({ etat: 'complet' });
    });

    it("rend complet quel que soit l'ORDRE d'arrivée des tranches", () => {
        // Rien ne garantit que le déposant émette dans l'ordre, ni que le
        // réseau les rende dans l'ordre.
        const desordre = [...plan(10, 4)].reverse();
        expect(verdict(10, 4, desordre)).toEqual({ etat: 'complet' });
    });

    it('nomme un trou au milieu', () => {
        expect(verdict(12, 4, [
            { n: 0, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'manquantes', n: [1] });
    });

    it("🔴 refuse de déclarer complet un fichier TRONQUÉ : la dernière tranche manque", () => {
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
        "🔴 un dépôt amputé de sa dernière tranche n'est JAMAIS complet (taille=%i, pas=%i)",
        (taille, pas) => {
            const ampute = plan(taille, pas).slice(0, -1);
            // La somme déposée est strictement inférieure à la taille : c'est
            // la formulation la plus directe de « le fichier est tronqué ».
            expect(somme(ampute)).toBeLessThan(taille);
            expect(verdict(taille, pas, ampute).etat).toBe('manquantes');
        },
    );

    it('nomme toutes les tranches quand rien n’a été déposé', () => {
        expect(verdict(10, 4, [])).toEqual({ etat: 'manquantes', n: [0, 1, 2] });
    });

    it('déclare INCOHÉRENTE une tranche plus COURTE que prévue, pas manquante', () => {
        // Un transfert amputé : la redemander rendrait la même chose.
        expect(verdict(12, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 3 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [1] });
    });

    it('déclare INCOHÉRENTE une tranche plus LONGUE que prévue', () => {
        // Elle déborderait sur sa voisine.
        expect(verdict(12, 4, [
            { n: 0, octets: 5 },
            { n: 1, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('déclare INCOHÉRENTE la dernière tranche envoyée à la taille du pas', () => {
        // Le piège naturel du déposant : remplir la queue jusqu'au pas.
        expect(verdict(10, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [2] });
    });

    it('déclare INCOHÉRENT un rang au-delà du plan', () => {
        expect(verdict(8, 4, [
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
            { n: 2, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [2] });
    });

    it('déclare INCOHÉRENT un rang négatif', () => {
        expect(verdict(8, 4, [
            { n: -1, octets: 4 },
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [-1] });
    });

    it('déclare INCOHÉRENT un rang non entier, et le rend TEL QUEL', () => {
        // La liste doit montrer ce qui a réellement été envoyé, pas une valeur
        // nettoyée : c'est ce qu'un journal a besoin de lire.
        expect(verdict(8, 4, [{ n: 1.5, octets: 4 }])).toEqual({
            etat: 'incoherentes',
            n: [1.5],
        });
    });

    it('déclare INCOHÉRENT un nombre d’octets non entier', () => {
        expect(verdict(8, 4, [
            { n: 0, octets: 4.5 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('déclare INCOHÉRENT un DOUBLON dont les deux occurrences s’accordent sur la taille', () => {
        // 🔴 Deux dépôts pour le même rang : l'un a écrasé l'autre, et rien ici
        // ne peut savoir lequel a gagné. Deux trames de même longueur ne
        // portent pas forcément le même contenu.
        expect(verdict(8, 4, [
            { n: 0, octets: 4 },
            { n: 0, octets: 4 },
            { n: 1, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('déclare INCOHÉRENT un doublon dont les occurrences divergent', () => {
        expect(verdict(8, 4, [
            { n: 1, octets: 4 },
            { n: 1, octets: 2 },
            { n: 0, octets: 4 },
        ])).toEqual({ etat: 'incoherentes', n: [1] });
    });

    it('🔴 fait PRIMER incoherentes sur manquantes quand les deux sont présents', () => {
        // Annoncer d'abord le trou ferait recompléter la tranche absente, puis
        // re-vérifier, puis retomber sur la même incohérence — la boucle exacte
        // que la distinction existe pour empêcher.
        const rendu = verdict(12, 4, [{ n: 0, octets: 9 }]);
        expect(rendu).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('rend complet pour un fichier vide dont rien n’a été déposé', () => {
        expect(verdict(0, 4, [])).toEqual({ etat: 'complet' });
    });

    it('déclare INCOHÉRENTE la moindre tranche déposée pour un fichier vide', () => {
        expect(verdict(0, 4, [{ n: 0, octets: 0 }])).toEqual({
            etat: 'incoherentes',
            n: [0],
        });
    });

    it('trie les rangs NUMÉRIQUEMENT et sans doublon', () => {
        // ⚠️ Onze tranches : c'est le seuil à partir duquel le tri par défaut
        // de JavaScript, qui compare des CHAÎNES, rendrait `[0, 10, 2, …]`.
        const rendu = verdict(44, 4, [{ n: 1, octets: 4 }]);
        expect(rendu).toEqual({
            etat: 'manquantes',
            n: [0, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        });
    });

    it('ne répète pas un rang incohérent signalé plusieurs fois', () => {
        const rendu = verdict(12, 4, [
            { n: 2, octets: 1 },
            { n: 2, octets: 1 },
            { n: 2, octets: 1 },
        ]);
        expect(rendu.etat).toBe('incoherentes');
        expect((rendu as { n: number[] }).n).toEqual([2]);
    });

    it.each<[string, number, number]>([
        ['un pas nul', 10, 0],
        ['une taille négative', -1, 4],
    ])('LÈVE sur un contrat invalide, comme plan : %s', (_titre, taille, pas) => {
        // La garde est la MÊME des deux côtés : un contrat qui ne tient pas ne
        // doit pas produire un verdict d'apparence normale.
        expect(() => verdict(taille, pas, [])).toThrow(/tranches :/);
    });

    it('ne lève JAMAIS sur ce qui vient du fil, si absurde soit-il', () => {
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
