// Tests de l'écran plein cadre des états terminaux — sous-bloc S4, tâche 9.
//
// ⚠️ ILS EXERCENT LE ROUTAGE AUTANT QUE L'ÉCRAN, et c'est le point : ce qui
// peut casser n'est pas « l'écran sait s'afficher » mais « il s'affiche pour
// les BONS messages ». Les trois premiers passent donc par `creerStatut`, avec
// l'écran injecté, comme le produit le fait.
//
// 🔴 LE SENS QUI COMPTE EST L'INVERSE. Un écran qui se lèverait à TOUT message
// passerait le premier test ; ce sont le deuxième et le troisième qui
// l'attrapent. C'est la même dissymétrie que `status.test.ts` garde sur la
// protection terminale.
//
// Sans DOM, par injection, comme `status.test.ts` et `audio.test.ts`.

import { describe, expect, it } from 'vitest';

import { creerStatut } from './status';
import { creerEcranTerminal } from './ecran-terminal';

function faireEcran() {
    const cible = {
        racine: { hidden: true },
        titre: { textContent: '' },
        raison: { textContent: '', className: '' },
    };
    return { cible, ecran: creerEcranTerminal(cible) };
}

function faireBandeau() {
    return { textContent: '', dataset: {} as { hidden?: string } };
}

describe('écran terminal', () => {
    it('① un message TERMINAL lève l’écran et y écrit le texte', () => {
        const { cible, ecran } = faireEcran();
        const statut = creerStatut(faireBandeau(), ecran);

        statut.afficher('session terminée : fermeture demandée', { terminal: true });

        expect(cible.racine.hidden).toBe(false);
        // Le TEXTE, pas seulement la visibilité : un écran levé et vide passerait
        // une assertion qui ne regarderait que `hidden`.
        expect(cible.raison.textContent).toBe('session terminée : fermeture demandée');
        expect(cible.titre.textContent).toBe('Session terminée');
    });

    it('② un message ORDINAIRE ne lève pas l’écran', () => {
        const { cible, ecran } = faireEcran();
        const statut = creerStatut(faireBandeau(), ecran);

        statut.afficher('prêt — 1280×720');

        expect(cible.racine.hidden).toBe(true);
        expect(cible.raison.textContent).toBe('');
    });

    it('③ un message PERSISTANT ne lève pas l’écran', () => {
        const { cible, ecran } = faireEcran();
        const statut = creerStatut(faireBandeau(), ecran);

        // Le sommeil et le lien dégradé passent par ici : ils DURENT, mais ils
        // ne terminent rien, et l'écran plein cadre masquerait une session
        // parfaitement vivante.
        statut.afficher('image figée : fenêtre masquée', { persistant: true });

        expect(cible.racine.hidden).toBe(true);
        expect(cible.raison.textContent).toBe('');
    });

    it('④ le ton danger pose message--danger, le ton neutre ne pose rien', () => {
        const danger = faireEcran();
        danger.ecran.montrer('échec : signaling injoignable', 'danger');
        expect(danger.cible.raison.className).toBe('message message--danger');
        expect(danger.cible.titre.textContent).toBe('Échec de la session');

        const neutre = faireEcran();
        neutre.ecran.montrer('session terminée : fermeture demandée', 'neutre');
        expect(neutre.cible.raison.className).toBe('message');
    });

    it('⑤ un second état terminal REMPLACE le premier, ton compris', () => {
        const { cible, ecran } = faireEcran();
        const statut = creerStatut(faireBandeau(), ecran);

        statut.afficher('échec : signaling injoignable', { terminal: true, ton: 'danger' });
        statut.afficher('session terminée : fermeture demandée', {
            terminal: true,
            ton: 'neutre',
        });

        expect(cible.raison.textContent).toBe('session terminée : fermeture demandée');
        // La classe est RÉÉCRITE, pas cumulée : sans quoi l'écran garderait le
        // rouge d'un échec sous le libellé d'une fin normale.
        expect(cible.raison.className).toBe('message');
        expect(cible.titre.textContent).toBe('Session terminée');
    });

    it('⑥ sans écran injecté, un message terminal ne casse rien', () => {
        // La cible est OPTIONNELLE : c'est ce qui laisse les onze tests de
        // `status.test.ts` passer inchangés, et c'est le comportement d'avant S4.
        const bandeau = faireBandeau();
        const statut = creerStatut(bandeau);

        statut.afficher('session terminée : fermeture demandée', { terminal: true });

        expect(bandeau.textContent).toBe('session terminée : fermeture demandée');
    });
});
