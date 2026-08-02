import { describe, expect, it } from 'vitest';
import { viewportPair } from './viewport';

describe('viewportPair', () => {
    it('laisse des dimensions déjà paires intactes', () => {
        expect(viewportPair(1280, 720)).toEqual({ largeur: 1280, hauteur: 720 });
    });

    // Le cas banal : un pop-up de navigateur annonce couramment une hauteur
    // impaire. 1280×713 a été mesuré en recette D1, et rendait l'ouverture de
    // la fenêtre impossible — l'agent aligne ses régions en pair pour NV12,
    // la taille demandée ne pouvait donc jamais égaler celle obtenue.
    it('arrondit vers le bas des dimensions impaires', () => {
        expect(viewportPair(1281, 713)).toEqual({ largeur: 1280, hauteur: 712 });
    });

    it('arrondit les fractions avant de rendre pair', () => {
        expect(viewportPair(1280.6, 713.4)).toEqual({ largeur: 1280, hauteur: 712 });
    });

    // Une sortie virtuelle de dimension nulle n'est pas capturable : mieux vaut
    // un plancher qu'un moniteur 0×0 dont le défaut se manifesterait bien plus
    // loin, sur la chaîne d'échange d'une mire ou sur le facteur d'échelle.
    it('ne descend jamais sous un plancher de deux pixels', () => {
        expect(viewportPair(1, 0)).toEqual({ largeur: 2, hauteur: 2 });
    });
});
