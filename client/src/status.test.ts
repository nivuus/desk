// Tests du bandeau de statut : priorité d'un message terminal sur les
// messages ordinaires qui le suivraient.
//
// Comme `audio.test.ts`, testé par injection, sans DOM.

import { describe, expect, it } from 'vitest';

import { createStatus } from './status';

function faireCible() {
    return { textContent: '', dataset: {} as { hidden?: string } };
}

describe('creerStatut', () => {
    it('displays an ordinary message', () => {
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('offer sent, waiting for the agent…');

        expect(element.textContent).toBe('offer sent, waiting for the agent…');
        expect(element.dataset.hidden).toBe('false');
    });

    it('displays a terminal message', () => {
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('session ended: close requested', { terminal: true });

        expect(element.textContent).toBe('session ended: close requested');
        expect(element.dataset.hidden).toBe('false');
    });

    it('an ordinary message arriving after a terminal one does not overwrite it', () => {
        // Le cas réel qui a motivé ce module : `connectionstatechange` sur
        // `disconnected` se déclenche juste après `session-end`, et écrivait
        // par-dessus « session terminée : … » avant ce correctif.
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('session ended: close requested', { terminal: true });
        statut.show('connection: disconnected');

        expect(element.textContent).toBe('session ended: close requested');
    });

    it('two successive terminal messages replace each other', () => {
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('session ended: close requested', { terminal: true });
        statut.show('session ended: agent error', { terminal: true });

        expect(element.textContent).toBe('session ended: agent error');
    });

    it('masquer() hides the banner as long as no terminal message is displayed', () => {
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('ready — 1920×1080');
        statut.masquer();

        expect(element.dataset.hidden).toBe('true');
    });

    it('masquer() does not erase a displayed terminal message', () => {
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('session ended: close requested', { terminal: true });
        statut.masquer();

        expect(element.dataset.hidden).toBe('false');
        expect(element.textContent).toBe('session ended: close requested');
    });

    it('masquer() does not erase a displayed persistent message', () => {
        // Cas réel : une alerte réseau (`link`, `alerte: true`) affichée
        // pendant la fenêtre de tir du minuteur anonyme d'un bandeau voisin
        // (« prêt », « manette détectée »…) ne doit pas disparaître quand ce
        // minuteur se déclenche — ce module ne sait rien de son existence.
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('Network insufficient for twitchy gaming — 1920×1080, 2.0 Mb/s', {
            persistant: true,
        });
        statut.masquer();

        expect(element.dataset.hidden).toBe('false');
        expect(element.textContent).toBe('Network insufficient for twitchy gaming — 1920×1080, 2.0 Mb/s');
    });

    it('an ordinary message following a persistent one lifts the persistence: masquer() applies again', () => {
        // Symétrique du cas ci-dessus : un retour à un état normal (par
        // exemple `link` avec `alerte: false` après une amélioration du
        // réseau) doit pouvoir se masquer normalement, sans que l'ancienne
        // alerte ne le bloque indéfiniment.
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('Network insufficient for twitchy gaming — 1920×1080, 2.0 Mb/s', {
            persistant: true,
        });
        statut.show('1920×1080, 8.0 Mb/s');
        statut.masquer();

        expect(element.dataset.hidden).toBe('true');
    });

    it('expirer() lifts the persistence of a persistent message and hides it', () => {
        // Cas réel qui a motivé cette méthode : le réveil d'une fenêtre
        // endormie doit effacer le bandeau « image figée : … » affiché avec
        // `persistant: true` à l'endormissement — `masquer()` seul ne le
        // peut pas, par construction.
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('image frozen: window hidden', { persistant: true });
        statut.expirer();

        expect(element.dataset.hidden).toBe('true');
    });

    it('expirer() does not erase a terminal message: the terminal guard is not weakened', () => {
        // Même exigence que pour masquer() : `expirer()` ne lève QUE la
        // persistance, jamais la protection terminale.
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('session ended: close requested', { terminal: true });
        statut.expirer();

        expect(element.dataset.hidden).toBe('false');
        expect(element.textContent).toBe('session ended: close requested');
    });

    it('a terminal message keeps priority over a persistent one: the terminal guard is not weakened', () => {
        // Ce test compte particulièrement : `persistant` est un drapeau ajouté
        // à côté de `terminal`, exactement le genre d'endroit où l'on
        // affaiblit une garde existante sans le voir.
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('session ended: close requested', { terminal: true });
        statut.show('Network insufficient for twitchy gaming — 1920×1080, 2.0 Mb/s', {
            persistant: true,
        });

        expect(element.textContent).toBe('session ended: close requested');

        statut.masquer();
        expect(element.dataset.hidden).toBe('false');
        expect(element.textContent).toBe('session ended: close requested');
    });
});
