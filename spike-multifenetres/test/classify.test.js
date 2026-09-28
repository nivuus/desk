import { describe, expect, it } from 'vitest';
import { classer, SEUIL_ACTIVATION_MS } from '../public/lib/classify.js';

const HORS_ACTIVATION = SEUIL_ACTIVATION_MS + 1000;

describe('classer', () => {
    it('classifies a null handle as blocked', () => {
        expect(classer({ poigneeNulle: true, vivante: false, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('bloque');
    });

    it('classifies a returned handle with a life signal as a success', () => {
        expect(classer({ poigneeNulle: false, vivante: true, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('succes');
    });

    it('classifies a returned handle without a life signal as ouverte-mais-perdue', () => {
        expect(classer({ poigneeNulle: false, vivante: false, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('ouverte-mais-perdue');
    });

    // La garde qui empêche de conclure à tort : un « succès » obtenu dans les 5 s
    // suivant un clic ne prouve rien, l'activation transitoire pouvait encore courir.
    it('refuses to conclude when the user gesture is too recent', () => {
        expect(classer({ poigneeNulle: false, vivante: true, msDepuisGeste: 1200 }))
            .toBe('non-concluant');
    });

    it('concludes despite a recent gesture when the gesture IS the tested mechanism', () => {
        expect(classer({
            poigneeNulle: false,
            vivante: true,
            msDepuisGeste: 12,
            gesteAttendu: true,
        })).toBe('succes');
    });

    it('classifies variant 4 on the life signal alone', () => {
        expect(classer({ poigneeNulle: 'sans-objet', vivante: true, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('succes');
        expect(classer({ poigneeNulle: 'sans-objet', vivante: false, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('ouverte-mais-perdue');
    });

    it('exposes the activation threshold at 5000 ms', () => {
        expect(SEUIL_ACTIVATION_MS).toBe(5000);
    });
});
