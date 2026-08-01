import { describe, it, expect, vi } from 'vitest';
import { creerBureau } from './shell';

function bureauDeTest() {
    const ouvertes = new Map<string, { closed: boolean; close: () => void }>();
    const envoyes: unknown[] = [];
    const bureau = creerBureau({
        ouvrirFenetre: (session) => {
            const f = { closed: false, close: () => { f.closed = true; } };
            ouvertes.set(session, f);
            return f as unknown as Window;
        },
        envoyer: (message) => { envoyes.push(message); },
        afficher: () => {},
    });
    return { bureau, ouvertes, envoyes };
}

describe('page-shell', () => {
    it('ouvre une fenêtre navigateur quand le superviseur annonce une fenêtre', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        expect(ouvertes.has('w-1')).toBe(true);
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: true }]);
    });

    it('transmet au superviseur le viewport que la page annonce', () => {
        const { bureau, envoyes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        bureau.viewportRecu('w-1', 1600, 900);
        expect(envoyes).toContainEqual({
            type: 'viewport', session: 'w-1', largeur: 1600, hauteur: 900,
        });
    });

    it("ignore un viewport pour une session qu'elle n'a pas ouverte", () => {
        // Le message vient de `postMessage` : n'importe quelle page de même
        // origine peut en émettre un. Ne relayer que ce qu'on a demandé.
        const { bureau, envoyes } = bureauDeTest();
        bureau.viewportRecu('w-inventee', 800, 600);
        expect(envoyes).toHaveLength(0);
    });

    it('ferme la fenêtre navigateur quand la fenêtre Windows disparaît', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        bureau.fenetreFermee('w-1');
        expect(ouvertes.get('w-1')!.closed).toBe(true);
        expect(bureau.liste()).toEqual([]);
    });

    it('garde la fenêtre dans sa liste quand seule la page a été fermée', () => {
        // Décision de D1 : fermer une page ne ferme pas l'application
        // Windows. La shell doit donc pouvoir la rouvrir.
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        ouvertes.get('w-1')!.closed = true;
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: false }]);
    });

    it('rouvre une fenêtre dont la page a été fermée', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        ouvertes.get('w-1')!.closed = true;
        bureau.rouvrir('w-1');
        expect(ouvertes.get('w-1')!.closed).toBe(false);
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: true }]);
    });

    it('affiche un refus sans rien ouvrir', () => {
        const affiche = vi.fn();
        const bureau = creerBureau({
            ouvrirFenetre: () => { throw new Error('rien ne doit être ouvert'); },
            envoyer: () => {},
            afficher: affiche,
        });
        bureau.refus('F9', 'plus aucune sortie virtuelle disponible');
        expect(affiche).toHaveBeenCalledWith(
            expect.stringContaining('plus aucune sortie virtuelle disponible'),
        );
    });

    it("signale un blocage de pop-up plutôt que de l'ignorer", () => {
        // `window.open` rend `null` quand le navigateur bloque : sans ce
        // traitement, l'utilisateur verrait une fenêtre listée « ouverte »
        // qui n'existe pas.
        const affiche = vi.fn();
        const bureau = creerBureau({
            ouvrirFenetre: () => null,
            envoyer: () => {},
            afficher: affiche,
        });
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        expect(affiche).toHaveBeenCalledWith(expect.stringContaining('pop-up'));
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: false }]);
    });
});
