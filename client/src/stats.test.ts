import { describe, expect, it } from 'vitest';

import { suivreMontant } from './stats';

describe('suivreMontant — les mesures du micro (chantier E)', () => {
    it("distingue « aucune piste montante » d'une piste montante à zéro", () => {
        // ⚠️ LA DISTINCTION EST LE FOND DU TEST, pas une coquetterie
        // d'affichage. `stats.ts` la fait déjà pour l'audio DESCENDANTE, et le
        // commentaire qui l'accompagne dit pourquoi : sans elle, une session
        // sans micro négocié se lit exactement comme un micro dont le débit est
        // simplement nul, et un défaut de négociation devient indiagnosticable.
        const absente = suivreMontant(undefined, undefined);
        expect(absente.ligne).toBe('micro absent');

        const posee = { octets: 0, paquets: 0, horodatage: 1000 };
        const zero = suivreMontant(posee, { octets: 0, paquets: 0, horodatage: 2000 });
        expect(zero.ligne).not.toBe('micro absent');
        expect(zero.ligne).toMatch(/^micro 0 kb\/s/);
    });

    it('calcule le débit sur le delta, pas sur le cumul', () => {
        const avant = { octets: 1_000, paquets: 10, horodatage: 1_000 };
        const apres = { octets: 3_000, paquets: 60, horodatage: 2_000 };

        // 2 000 octets en 1 s = 16 000 bits/s = 16 kb/s. Un affichage du CUMUL
        // rendrait 24 kb/s ici, ce qui passerait inaperçu sur une première
        // lecture et croîtrait sans fin.
        const { ligne } = suivreMontant(avant, apres);
        expect(ligne).toMatch(/^micro 16 kb\/s/);
        expect(ligne).toMatch(/paquets 60/);
    });

    it('un premier relevé ne prétend à aucun débit', () => {
        const { ligne, memoire } = suivreMontant(undefined, {
            octets: 5_000,
            paquets: 40,
            horodatage: 1_000,
        });
        expect(ligne).toMatch(/^micro 0 kb\/s/);
        expect(memoire).toEqual({ octets: 5_000, paquets: 40, horodatage: 1_000 });
    });

    it('la disparition de la piste OUBLIE l’instantané précédent', () => {
        // Même défaut que celui déjà corrigé sur l'audio descendante : sans
        // l'oubli, une piste qui disparaît puis revient (SSRC renégocié,
        // compteurs repartis de zéro) calculerait son premier débit contre les
        // compteurs d'un AUTRE flux — delta absurde, voire négatif.
        const { memoire } = suivreMontant({ octets: 9_000, paquets: 90, horodatage: 1_000 }, undefined);
        expect(memoire).toBeUndefined();
    });

    it('un horodatage qui ne progresse pas ne rend pas un débit infini', () => {
        const meme = { octets: 1_000, paquets: 10, horodatage: 5_000 };
        const { ligne } = suivreMontant(meme, { octets: 4_000, paquets: 40, horodatage: 5_000 });
        expect(ligne).toMatch(/^micro 0 kb\/s/);
    });
});
