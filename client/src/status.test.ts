// Tests of the status banner: priority of a terminal message over the
// ordinary messages that would follow it.
//
// Like `audio.test.ts`, tested by injection, without a DOM.

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
        // The real case that motivated this module: `connectionstatechange` on
        // `disconnected` fires right after `session-end`, and wrote
        // over "session ended: …" before this fix.
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
        // Real case: a network alert (`link`, `alerte: true`) displayed
        // during the firing window of the anonymous timer of a neighbouring banner
        // ("ready", "gamepad detected"…) must not disappear when that
        // timer fires — this module knows nothing of its existence.
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
        // Symmetric to the case above: a return to a normal state (for
        // example `link` with `alerte: false` after the network
        // improves) must be able to hide normally, without the old
        // alert blocking it indefinitely.
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
        // The real case that motivated this method: waking up an
        // asleep window must clear the "image frozen: …" banner displayed with
        // `persistant: true` when it fell asleep — `masquer()` alone cannot
        // do it, by construction.
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('image frozen: window hidden', { persistant: true });
        statut.expirer();

        expect(element.dataset.hidden).toBe('true');
    });

    it('expirer() does not erase a terminal message: the terminal guard is not weakened', () => {
        // Same requirement as for masquer(): `expirer()` ONLY lifts
        // persistence, never the terminal protection.
        const element = faireCible();
        const statut = createStatus(element);

        statut.show('session ended: close requested', { terminal: true });
        statut.expirer();

        expect(element.dataset.hidden).toBe('false');
        expect(element.textContent).toBe('session ended: close requested');
    });

    it('a terminal message keeps priority over a persistent one: the terminal guard is not weakened', () => {
        // This test matters particularly: `persistant` is a flag added
        // next to `terminal`, exactly the kind of place where one
        // weakens an existing guard without seeing it.
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
