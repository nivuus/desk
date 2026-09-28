import { describe, expect, it } from 'vitest';
import { RejeuResize } from './resize';

describe('RejeuResize', () => {
    it("returns the observed size when nothing was emitted yet", () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        expect(r.aEmettre()).toEqual({ largeur: 1280, hauteur: 720 });
    });

    it('returns nothing when the emitted size is already the right one', () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        r.confirmer({ largeur: 1280, hauteur: 720 });
        expect(r.aEmettre()).toBeUndefined();
    });

    it("returns the observed size as long as no emission was confirmed", () => {
        // ⚠️ Ce test n'exerce AUCUN état de canal : `RejeuResize` est pur et
        // n'en connaît aucun (leg n°11 de D9 — son titre annonçait « quand le
        // canal était fermé au moment du geste », état qu'il ne pouvait pas
        // atteindre, et il rendait le même verdict pour n'importe quelle autre
        // raison de non-confirmation). L'état de canal vit dans `main.ts`,
        // chez `emettreSiPossible`, et c'est là qu'il se teste — pas ici.
        //
        // MOTIVATION du mécanisme, et non ce que ce test éprouve : le cas du
        // leg 10 est celui où le ResizeObserver a vu la taille mais
        // `readyState !== 'open'` a fait abandonner l'envoi. À l'ouverture du
        // canal, la taille doit repartir — sans quoi elle est perdue à jamais,
        // l'observateur ne se redéclenchant que sur un NOUVEAU changement.
        const r = new RejeuResize();
        r.observer({ largeur: 1920, hauteur: 1080 });
        // aucun `confirmer` : l'envoi n'a pas eu lieu
        expect(r.aEmettre()).toEqual({ largeur: 1920, hauteur: 1080 });
    });

    it('returns the LAST observed size, not the first', () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        r.observer({ largeur: 1920, hauteur: 1080 });
        expect(r.aEmettre()).toEqual({ largeur: 1920, hauteur: 1080 });
    });

    it("returns nothing as long as nothing was observed", () => {
        expect(new RejeuResize().aEmettre()).toBeUndefined();
    });
});
