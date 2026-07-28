import { describe, expect, it } from 'vitest';
import { classer, SEUIL_ACTIVATION_MS } from '../public/lib/classify.js';

const HORS_ACTIVATION = SEUIL_ACTIVATION_MS + 1000;

describe('classer', () => {
    it('classe une poignée nulle en bloqué', () => {
        expect(classer({ poigneeNulle: true, vivante: false, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('bloque');
    });

    it('classe une poignée rendue avec signal de vie en succès', () => {
        expect(classer({ poigneeNulle: false, vivante: true, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('succes');
    });

    it('classe une poignée rendue sans signal de vie en ouverte-mais-perdue', () => {
        expect(classer({ poigneeNulle: false, vivante: false, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('ouverte-mais-perdue');
    });

    // La garde qui empêche de conclure à tort : un « succès » obtenu dans les 5 s
    // suivant un clic ne prouve rien, l'activation transitoire pouvait encore courir.
    it('refuse de conclure quand le geste utilisateur est trop récent', () => {
        expect(classer({ poigneeNulle: false, vivante: true, msDepuisGeste: 1200 }))
            .toBe('non-concluant');
    });

    it('conclut malgré un geste récent quand le geste EST le mécanisme testé', () => {
        expect(classer({
            poigneeNulle: false,
            vivante: true,
            msDepuisGeste: 12,
            gesteAttendu: true,
        })).toBe('succes');
    });

    it('classe la variante 4 sur le seul signal de vie', () => {
        expect(classer({ poigneeNulle: 'sans-objet', vivante: true, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('succes');
        expect(classer({ poigneeNulle: 'sans-objet', vivante: false, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('ouverte-mais-perdue');
    });

    it('expose le seuil d\'activation à 5000 ms', () => {
        expect(SEUIL_ACTIVATION_MS).toBe(5000);
    });
});
