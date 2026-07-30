// Tests du bandeau de statut : priorité d'un message terminal sur les
// messages ordinaires qui le suivraient.
//
// Comme `audio.test.ts`, testé par injection, sans DOM.

import { describe, expect, it } from 'vitest';

import { creerStatut } from './status';

function faireCible() {
    return { textContent: '', dataset: {} as { hidden?: string } };
}

describe('creerStatut', () => {
    it('affiche un message ordinaire', () => {
        const element = faireCible();
        const statut = creerStatut(element);

        statut.afficher('offre envoyée, attente de l’agent…');

        expect(element.textContent).toBe('offre envoyée, attente de l’agent…');
        expect(element.dataset.hidden).toBe('false');
    });

    it('affiche un message terminal', () => {
        const element = faireCible();
        const statut = creerStatut(element);

        statut.afficher('session terminée : fermeture demandée', { terminal: true });

        expect(element.textContent).toBe('session terminée : fermeture demandée');
        expect(element.dataset.hidden).toBe('false');
    });

    it('un message ordinaire arrivant après un terminal ne l’écrase pas', () => {
        // Le cas réel qui a motivé ce module : `connectionstatechange` sur
        // `disconnected` se déclenche juste après `session-end`, et écrivait
        // par-dessus « session terminée : … » avant ce correctif.
        const element = faireCible();
        const statut = creerStatut(element);

        statut.afficher('session terminée : fermeture demandée', { terminal: true });
        statut.afficher('connexion : disconnected');

        expect(element.textContent).toBe('session terminée : fermeture demandée');
    });

    it('deux messages terminaux successifs se remplacent l’un l’autre', () => {
        const element = faireCible();
        const statut = creerStatut(element);

        statut.afficher('session terminée : fermeture demandée', { terminal: true });
        statut.afficher('session terminée : erreur agent', { terminal: true });

        expect(element.textContent).toBe('session terminée : erreur agent');
    });

    it('masquer() cache le bandeau tant qu’aucun message terminal n’est affiché', () => {
        const element = faireCible();
        const statut = creerStatut(element);

        statut.afficher('prêt — 1920×1080');
        statut.masquer();

        expect(element.dataset.hidden).toBe('true');
    });

    it('masquer() n’efface pas un message terminal affiché', () => {
        const element = faireCible();
        const statut = creerStatut(element);

        statut.afficher('session terminée : fermeture demandée', { terminal: true });
        statut.masquer();

        expect(element.dataset.hidden).toBe('false');
        expect(element.textContent).toBe('session terminée : fermeture demandée');
    });

    it('masquer() n’efface pas un message persistant affiché', () => {
        // Cas réel : une alerte réseau (`link`, `alerte: true`) affichée
        // pendant la fenêtre de tir du minuteur anonyme d'un bandeau voisin
        // (« prêt », « manette détectée »…) ne doit pas disparaître quand ce
        // minuteur se déclenche — ce module ne sait rien de son existence.
        const element = faireCible();
        const statut = creerStatut(element);

        statut.afficher('Réseau insuffisant pour le jeu nerveux — 1920×1080, 2.0 Mb/s', {
            persistant: true,
        });
        statut.masquer();

        expect(element.dataset.hidden).toBe('false');
        expect(element.textContent).toBe('Réseau insuffisant pour le jeu nerveux — 1920×1080, 2.0 Mb/s');
    });

    it('un message ordinaire suivant un persistant lève la persistance : masquer() s’applique de nouveau', () => {
        // Symétrique du cas ci-dessus : un retour à un état normal (par
        // exemple `link` avec `alerte: false` après une amélioration du
        // réseau) doit pouvoir se masquer normalement, sans que l'ancienne
        // alerte ne le bloque indéfiniment.
        const element = faireCible();
        const statut = creerStatut(element);

        statut.afficher('Réseau insuffisant pour le jeu nerveux — 1920×1080, 2.0 Mb/s', {
            persistant: true,
        });
        statut.afficher('1920×1080, 8.0 Mb/s');
        statut.masquer();

        expect(element.dataset.hidden).toBe('true');
    });

    it('un message terminal reste prioritaire sur un persistant : la garde terminale n’est pas affaiblie', () => {
        // Ce test compte particulièrement : `persistant` est un drapeau ajouté
        // à côté de `terminal`, exactement le genre d'endroit où l'on
        // affaiblit une garde existante sans le voir.
        const element = faireCible();
        const statut = creerStatut(element);

        statut.afficher('session terminée : fermeture demandée', { terminal: true });
        statut.afficher('Réseau insuffisant pour le jeu nerveux — 1920×1080, 2.0 Mb/s', {
            persistant: true,
        });

        expect(element.textContent).toBe('session terminée : fermeture demandée');

        statut.masquer();
        expect(element.dataset.hidden).toBe('false');
        expect(element.textContent).toBe('session terminée : fermeture demandée');
    });
});
