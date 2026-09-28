// Tests de l'écran plein cadre des états terminaux — sous-bloc S4, tâche 9.
//
// ⚠️ ILS EXERCENT LE ROUTAGE AUTANT QUE L'ÉCRAN, et c'est le point : ce qui
// peut casser n'est pas « l'écran sait s'afficher » mais « il s'affiche pour
// les BONS messages ». Les trois premiers passent donc par `createStatus`, avec
// l'écran injecté, comme le produit le fait.
//
// 🔴 LE SENS QUI COMPTE EST L'INVERSE. Un écran qui se lèverait à TOUT message
// passerait le premier test ; ce sont le deuxième et le troisième qui
// l'attrapent. C'est la même dissymétrie que `status.test.ts` garde sur la
// protection terminale.
//
// Sans DOM, par injection, comme `status.test.ts` et `audio.test.ts`.

import { describe, expect, it } from 'vitest';

import { createStatus } from './status';
import { createTerminalScreen } from './ecran-terminal';

function faireEcran() {
    const cible = {
        racine: { hidden: true },
        titre: { textContent: '' },
        raison: { textContent: '', className: '' },
    };
    return { cible, ecran: createTerminalScreen(cible) };
}

function faireBandeau() {
    return { textContent: '', dataset: {} as { hidden?: string } };
}

describe('terminal screen', () => {
    it('① a TERMINAL message raises the screen and writes the text into it', () => {
        const { cible, ecran } = faireEcran();
        const statut = createStatus(faireBandeau(), ecran);

        statut.show('session ended: close requested', { terminal: true });

        expect(cible.racine.hidden).toBe(false);
        // Le TEXTE, pas seulement la visibilité : un écran levé et vide passerait
        // une assertion qui ne regarderait que `hidden`.
        expect(cible.raison.textContent).toBe('session ended: close requested');
        expect(cible.titre.textContent).toBe('Session ended');
    });

    it('② an ORDINARY message does not raise the screen', () => {
        const { cible, ecran } = faireEcran();
        const statut = createStatus(faireBandeau(), ecran);

        statut.show('ready — 1280×720');

        expect(cible.racine.hidden).toBe(true);
        expect(cible.raison.textContent).toBe('');
    });

    it('③ a PERSISTENT message does not raise the screen', () => {
        const { cible, ecran } = faireEcran();
        const statut = createStatus(faireBandeau(), ecran);

        // Le sommeil et le lien dégradé passent par ici : ils DURENT, mais ils
        // ne terminent rien, et l'écran plein cadre masquerait une session
        // parfaitement vivante.
        statut.show('image frozen: window hidden', { persistant: true });

        expect(cible.racine.hidden).toBe(true);
        expect(cible.raison.textContent).toBe('');
    });

    it('④ the danger tone sets message--danger, the neutral tone sets nothing', () => {
        const danger = faireEcran();
        danger.ecran.montrer('failure: signaling unreachable', 'danger');
        expect(danger.cible.raison.className).toBe('message message--danger');
        expect(danger.cible.titre.textContent).toBe('Session failed');

        const neutre = faireEcran();
        neutre.ecran.montrer('session ended: close requested', 'neutre');
        expect(neutre.cible.raison.className).toBe('message');
    });

    it('⑤ a second terminal state REPLACES the first, tone included', () => {
        const { cible, ecran } = faireEcran();
        const statut = createStatus(faireBandeau(), ecran);

        statut.show('failure: signaling unreachable', { terminal: true, ton: 'danger' });
        statut.show('session ended: close requested', {
            terminal: true,
            ton: 'neutre',
        });

        expect(cible.raison.textContent).toBe('session ended: close requested');
        // La classe est RÉÉCRITE, pas cumulée : sans quoi l'écran garderait le
        // rouge d'un échec sous le libellé d'une fin normale.
        expect(cible.raison.className).toBe('message');
        expect(cible.titre.textContent).toBe('Session ended');
    });

    it('⑥ without an injected screen, a terminal message breaks nothing', () => {
        // La cible est OPTIONNELLE : c'est ce qui laisse les onze tests de
        // `status.test.ts` passer inchangés, et c'est le comportement d'avant S4.
        const bandeau = faireBandeau();
        const statut = createStatus(bandeau);

        statut.show('session ended: close requested', { terminal: true });

        expect(bandeau.textContent).toBe('session ended: close requested');
    });
});
