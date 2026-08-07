import { describe, expect, it } from 'vitest';
import { RejeuResize } from './resize';

describe('RejeuResize', () => {
    it("rend la taille observée quand rien n'a encore été émis", () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        expect(r.aEmettre()).toEqual({ largeur: 1280, hauteur: 720 });
    });

    it('ne rend rien quand la taille émise est déjà la bonne', () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        r.confirmer({ largeur: 1280, hauteur: 720 });
        expect(r.aEmettre()).toBeUndefined();
    });

    it('REJOUE la dernière taille quand le canal était fermé au moment du geste', () => {
        // Le cas du leg 10 : le ResizeObserver a vu la taille, mais
        // `readyState !== 'open'` a fait abandonner l'envoi. À l'ouverture du
        // canal, la taille doit repartir — sans quoi elle est perdue à jamais,
        // l'observateur ne se redéclenchant que sur un NOUVEAU changement.
        const r = new RejeuResize();
        r.observer({ largeur: 1920, hauteur: 1080 });
        // aucun `confirmer` : l'envoi n'a pas eu lieu
        expect(r.aEmettre()).toEqual({ largeur: 1920, hauteur: 1080 });
    });

    it('rend la DERNIÈRE taille observée, pas la première', () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        r.observer({ largeur: 1920, hauteur: 1080 });
        expect(r.aEmettre()).toEqual({ largeur: 1920, hauteur: 1080 });
    });

    it("ne rend rien tant que rien n'a été observé", () => {
        expect(new RejeuResize().aEmettre()).toBeUndefined();
    });
});
