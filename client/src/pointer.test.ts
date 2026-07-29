import { describe, expect, it } from 'vitest';
import { creerClampReport, sommerDeltas } from './pointer';

describe('sommation des deltas coalescés', () => {
    it('somme les échantillons intermédiaires', () => {
        expect(sommerDeltas([
            { movementX: 3, movementY: -1 },
            { movementX: 4, movementY: -2 },
        ])).toEqual({ dx: 7, dy: -3 });
    });

    it('rend zéro sur une liste vide', () => {
        expect(sommerDeltas([])).toEqual({ dx: 0, dy: 0 });
    });
});

describe('clamp avec report', () => {
    it('laisse passer les valeurs dans la plage', () => {
        const clamp = creerClampReport();
        expect(clamp(10, -10)).toEqual({ dx: 10, dy: -10 });
    });

    it('borne le débordement et le reporte sur l\'appel suivant', () => {
        // La somme transmise doit rester exacte : c'est elle qui détermine la
        // visée. Écrêter en perdant le reste ferait dériver le tir.
        const clamp = creerClampReport();
        expect(clamp(40000, 0)).toEqual({ dx: 32767, dy: 0 });
        expect(clamp(0, 0)).toEqual({ dx: 7233, dy: 0 });
        expect(clamp(0, 0)).toEqual({ dx: 0, dy: 0 });
    });

    it('reporte aussi les débordements négatifs', () => {
        const clamp = creerClampReport();
        expect(clamp(0, -40000)).toEqual({ dx: 0, dy: -32768 });
        expect(clamp(0, 0)).toEqual({ dx: 0, dy: -7232 });
    });

    it('ne reporte rien quand rien ne déborde', () => {
        const clamp = creerClampReport();
        clamp(5, 5);
        expect(clamp(0, 0)).toEqual({ dx: 0, dy: 0 });
    });
});
