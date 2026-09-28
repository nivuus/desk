// Tests of the full-frame screen of terminal states — sub-block S4, task 9.
//
// ⚠️ THEY EXERCISE THE ROUTING AS MUCH AS THE SCREEN, and that is the point: what
// can break is not "the screen can display itself" but "it displays for
// the RIGHT messages". The first three therefore go through `createStatus`, with
// the screen injected, as the product does.
//
// 🔴 THE DIRECTION THAT MATTERS IS THE REVERSE. A screen that rose on ANY message
// would pass the first test; it is the second and the third that
// catch it. It is the same asymmetry `status.test.ts` guards on the
// terminal protection.
//
// Without a DOM, by injection, like `status.test.ts` and `audio.test.ts`.

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
        // The TEXT, not only the visibility: a raised and empty screen would pass
        // an assertion that only looked at `hidden`.
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

        // Sleep and a degraded link go through here: they LAST, but they
        // end nothing, and the full-frame screen would hide a perfectly
        // alive session.
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
        // The class is REWRITTEN, not accumulated: otherwise the screen would keep the
        // red of a failure under the label of a normal end.
        expect(cible.raison.className).toBe('message');
        expect(cible.titre.textContent).toBe('Session ended');
    });

    it('⑥ without an injected screen, a terminal message breaks nothing', () => {
        // The target is OPTIONAL: that is what lets the eleven tests of
        // `status.test.ts` pass unchanged, and it is the behaviour from before S4.
        const bandeau = faireBandeau();
        const statut = createStatus(bandeau);

        statut.show('session ended: close requested', { terminal: true });

        expect(bandeau.textContent).toBe('session ended: close requested');
    });
});
