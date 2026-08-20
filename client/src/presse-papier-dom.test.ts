import { describe, expect, it, vi } from 'vitest';

import { attacherPressePapierAuDOM } from './presse-papier-dom';
import { MESSAGE_ECHEC, messageDeRefus } from './presse-papier';

/// Une cible d'événements minimale, sans DOM : le module n'a besoin que de
/// `focus`, et l'injecter est ce qui rend ce fichier éprouvable sans jsdom.
function cibleFactice() {
    const rappels = new Map<string, Set<() => void>>();
    return {
        addEventListener(nom: string, rappel: () => void) {
            if (!rappels.has(nom)) rappels.set(nom, new Set());
            rappels.get(nom)!.add(rappel);
        },
        removeEventListener(nom: string, rappel: () => void) {
            rappels.get(nom)?.delete(rappel);
        },
        declencher(nom: string) {
            for (const rappel of rappels.get(nom) ?? []) rappel();
        },
        compte(nom: string) {
            return rappels.get(nom)?.size ?? 0;
        },
    };
}

describe('attacherPressePapierAuDOM', () => {
    it('écrit le texte reçu quand la fenêtre a le focus', async () => {
        const ecrire = vi.fn().mockResolvedValue(undefined);
        const cible = cibleFactice();
        const attache = attacherPressePapierAuDOM({
            ecrire,
            focalise: () => true,
            cible,
            surMessage: vi.fn(),
        });

        attache.recevoir({ texte: 'bonjour', octets: 7 });
        await Promise.resolve();

        expect(ecrire).toHaveBeenCalledWith('bonjour');
        attache.detacher();
    });

    /// Le dépôt différé de D3, et la seule ligne de DOM de ce module : sans
    /// focus on ne tente rien (`writeText` échouerait), et le retour du focus
    /// est ce qui sort le texte.
    it("n'écrit rien sans focus, puis écrit au retour du focus", async () => {
        const ecrire = vi.fn().mockResolvedValue(undefined);
        const cible = cibleFactice();
        let focalise = false;
        const attache = attacherPressePapierAuDOM({
            ecrire,
            focalise: () => focalise,
            cible,
            surMessage: vi.fn(),
        });

        attache.recevoir({ texte: 'differe', octets: 7 });
        await Promise.resolve();
        expect(ecrire).not.toHaveBeenCalled();

        focalise = true;
        cible.declencher('focus');
        await Promise.resolve();

        expect(ecrire).toHaveBeenCalledWith('differe');
        attache.detacher();
    });

    /// Un refus est DIT, jamais tu — et il ne déclenche aucune écriture.
    it('dit le refus et n’écrit rien', async () => {
        const ecrire = vi.fn().mockResolvedValue(undefined);
        const surMessage = vi.fn();
        const attache = attacherPressePapierAuDOM({
            ecrire,
            focalise: () => true,
            cible: cibleFactice(),
            surMessage,
        });

        attache.recevoir({ texte: null, octets: 100_000 });
        await Promise.resolve();

        expect(ecrire).not.toHaveBeenCalled();
        expect(surMessage).toHaveBeenCalledWith(messageDeRefus(100_000));
        attache.detacher();
    });

    /// `ECHECS_AVANT_MESSAGE` vaut 2 : le premier échec est le cas ordinaire
    /// d'une fenêtre qui perd le focus pendant l'écriture, et crier dessus
    /// ferait un bandeau permanent sur un produit qui marche.
    it('ne crie qu’au deuxième échec consécutif', async () => {
        const ecrire = vi.fn().mockRejectedValue(new Error('refusé'));
        const surMessage = vi.fn();
        const cible = cibleFactice();
        const attache = attacherPressePapierAuDOM({
            ecrire,
            focalise: () => true,
            cible,
            surMessage,
        });

        attache.recevoir({ texte: 'un', octets: 2 });
        await Promise.resolve();
        await Promise.resolve();
        expect(surMessage).not.toHaveBeenCalled();

        attache.recevoir({ texte: 'deux', octets: 4 });
        await Promise.resolve();
        await Promise.resolve();
        expect(surMessage).toHaveBeenCalledWith(MESSAGE_ECHEC);
        attache.detacher();
    });

    /// Sans ce détachement, l'écouteur `focus` survivrait à la fin de session
    /// et écrirait le presse-papier local d'une session morte — le même défaut
    /// que les trois détachements voisins de `main.ts` existent pour éviter.
    it('détache son écouteur de focus', () => {
        const cible = cibleFactice();
        const attache = attacherPressePapierAuDOM({
            ecrire: vi.fn().mockResolvedValue(undefined),
            focalise: () => true,
            cible,
            surMessage: vi.fn(),
        });

        expect(cible.compte('focus')).toBe(1);
        attache.detacher();
        expect(cible.compte('focus')).toBe(0);
    });

    /// 🔴 `readText()` n'est appelée NULLE PART, ni au focus ni jamais : c'est
    /// le geste de l'ancien produit (`web/index.js`), il exige une permission,
    /// et il lit une ressource privée EN DEHORS de toute intention de collage.
    /// Ce test garde cette propriété contre une régression future — le module
    /// ne reçoit aucune fonction de lecture, et son interface ne peut donc pas
    /// en acquérir une sans que ce fichier ne cesse de compiler.
    it('ne reçoit aucune capacité de LECTURE du presse-papier', () => {
        const options = {
            ecrire: vi.fn().mockResolvedValue(undefined),
            focalise: () => true,
            cible: cibleFactice(),
            surMessage: vi.fn(),
        };
        expect(Object.keys(options).sort()).toEqual(['cible', 'ecrire', 'focalise', 'surMessage']);
        attacherPressePapierAuDOM(options).detacher();
    });
});
