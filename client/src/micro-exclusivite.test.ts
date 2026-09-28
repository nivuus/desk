// Block E3: the VM cable's exclusivity refusal, on the button side.
//
// ⚠️ **File BORN FROM AN EXTRACTION**, not from a thematic split: these
// tests lived in `micro.test.ts`, which they took from 411 to 515 lines —
// past the 500 gate. See the header of `micro.fixtures.ts`.

import { describe, expect, it } from 'vitest';

import { attacherBoutonMicro } from './micro';
import { faussePiste, fauxBouton, fauxFlux, fauxSender } from './micro.fixtures';

describe("attacherBoutonMicro — the cable exclusivity (block E3)", () => {

    // ── Block E3: the VM cable's exclusivity refusal ────────────────────────

    /// The common setup: button on, messages captured.
    async function boutonAllume() {
        const bouton = fauxBouton();
        const messages: string[] = [];
        const controle = attacherBoutonMicro({
            bouton,
            sender: fauxSender(),
            demanderFlux: async () => fauxFlux(faussePiste()),
            surMessage: (texte) => messages.push(texte),
        });
        controle.annoncerDisponibilite(true);
        bouton.cliquer();
        await controle.enCours();
        expect(bouton.dataset.etat).toBe('actif');
        return { bouton, controle, messages };
    }

    /// 🔴 It is red R5, and its most important half is the SECOND
    /// assertion: the state does not move.
    ///
    /// Filing this refusal under `'refuse'` would confuse two causes that call for
    /// two OPPOSITE gestures — one is fixed in the browser settings,
    /// the other by closing the other window. And turning off the button of a window
    /// whose browser really emits, capture indicator on, is
    /// the visual lie spec §9 "Privacy" rules out.
    it("an exclusivity refusal changes the LABEL and the banner, never the state", async () => {
        const { bouton, controle, messages } = await boutonAllume();
        const titreNominal = bouton.title;

        controle.annoncerExclusivite(false);

        expect(bouton.dataset.etat).toBe('actif');
        expect(bouton.disabled).toBe(false);
        expect(bouton.title).not.toBe(titreNominal);
        expect(bouton.title).toMatch(/another window/);
        expect(messages.at(-1)).toMatch(/another window/);
    });

    it("resuming sets the nominal label back and SAYS so", async () => {
        const { bouton, controle, messages } = await boutonAllume();
        const titreNominal = bouton.title;

        controle.annoncerExclusivite(false);
        controle.annoncerExclusivite(true);

        expect(bouton.title).toBe(titreNominal);
        expect(messages.at(-1)).toMatch(/heard by the VM again/);
    });

    /// Without this guard, every `mic-state { granted: true }` — the COMMON case,
    /// that of a lone window — would push a "heard again" banner
    /// to a window that never stopped being heard.
    it("a grant that follows no refusal pushes no banner", async () => {
        const { controle, messages } = await boutonAllume();
        const before = messages.length;
        controle.annoncerExclusivite(true);
        controle.annoncerExclusivite(true);
        expect(messages.length).toBe(before);
    });

    /// ⚠️ A `mic-state` in flight arriving after a switch-off would overwrite
    /// the title of a CLOSED button with a label that talks about an open microphone.
    it("a mic-state received with the mic closed touches nothing", async () => {
        const bouton = fauxBouton();
        const messages: string[] = [];
        const controle = attacherBoutonMicro({
            bouton,
            sender: fauxSender(),
            demanderFlux: async () => fauxFlux(faussePiste()),
            surMessage: (texte) => messages.push(texte),
        });
        controle.annoncerDisponibilite(true);
        const titreFerme = bouton.title;

        controle.annoncerExclusivite(false);

        expect(bouton.dataset.etat).toBe('ferme');
        expect(bouton.title).toBe(titreFerme);
        expect(messages).toEqual([]);
    });

    /// Switching off then on again while the other window STILL holds the cable
    /// must raise the banner again. Without resetting the flag on
    /// a state transition, the second refusal would be seen as "not a change".
    it("a lasting refusal comes back after switching off and on again", async () => {
        const { bouton, controle, messages } = await boutonAllume();
        controle.annoncerExclusivite(false);
        expect(messages.at(-1)).toMatch(/another window/);

        bouton.cliquer();
        await controle.enCours();
        expect(bouton.dataset.etat).toBe('ferme');
        bouton.cliquer();
        await controle.enCours();
        expect(bouton.dataset.etat).toBe('actif');

        const before = messages.length;
        controle.annoncerExclusivite(false);
        expect(messages.length).toBe(before + 1);
        expect(bouton.title).toMatch(/another window/);
    });
});
