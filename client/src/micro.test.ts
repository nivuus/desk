import { describe, expect, it, vi } from 'vitest';

import { attacherBoutonMicro, attacherMicro, type EtatMicro } from './micro';
import { domError, faussePiste, fauxBouton, fauxFlux, fauxSender } from './micro.fixtures';
describe('attacherMicro — the toggle and its four states', () => {
    it('at start, the sender has no track and the state is « closed »', () => {
        const sender = fauxSender();
        const micro = attacherMicro({
            sender,
            demanderFlux: async () => fauxFlux(faussePiste()),
        });

        // Nothing was asked of the browser: permission is asked ON
        // CLICK, never at session opening (spec §9, "Privacy").
        expect(sender.recus).toEqual([]);
        expect(micro.etat()).toBe('ferme');
    });

    it('the toggle calls getUserMedia only once, then replaceTrack', async () => {
        const piste = faussePiste();
        const sender = fauxSender();
        const demanderFlux = vi.fn(async () => fauxFlux(piste));
        const etats: Array<[EtatMicro, string | undefined]> = [];
        const micro = attacherMicro({
            sender,
            demanderFlux,
            surEtat: (etat, detail) => etats.push([etat, detail]),
        });

        await expect(micro.basculer()).resolves.toBe('actif');

        expect(demanderFlux).toHaveBeenCalledTimes(1);
        // The constraints of spec §7: AEC, noise suppression, automatic
        // gain. The AEC is the BROWSER's, the only place that
        // knows both the captured stream and the played-back stream.
        //
        // ⚠️ THE LITERAL IS COPIED, AND IT IS THE CRUX OF THE TEST. The
        // previous draft imported the module's constant and asserted
        // `toHaveBeenCalledWith(CONTRAINTES)`: both sides of the equality
        // read the same object, and replacing it with `audio: true` left
        // this test GREEN. The mutation was played and seen green before the fix.
        expect(demanderFlux).toHaveBeenCalledWith({
            audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
        });
        expect(sender.recus).toEqual([piste]);
        expect(micro.etat()).toBe('actif');
        expect(etats).toEqual([['actif', undefined]]);
    });

    it("switching off calls replaceTrack(null) AND track.stop()", async () => {
        const piste = faussePiste();
        const sender = fauxSender();
        const micro = attacherMicro({ sender, demanderFlux: async () => fauxFlux(piste) });

        await micro.basculer();
        await expect(micro.basculer()).resolves.toBe('ferme');

        // ⚠️ THE CENTRAL TEST OF THIS MODULE. `enabled = false` alone would leave the
        // device open and Chrome's indicator on: spec §9
        // calls this visual lie unacceptable "on this very
        // feature". BOTH assertions are needed — a test that only
        // checked `replaceTrack(null)` would not tell the two
        // implementations apart, and would therefore guard nothing.
        expect(piste.arretee).toBe(true);
        expect(sender.recus).toEqual([piste, null]);
        expect(micro.etat()).toBe('ferme');
    });

    it('a permission refusal leads to the « refused » state without an exception', async () => {
        const sender = fauxSender();
        const etats: Array<[EtatMicro, string | undefined]> = [];
        const micro = attacherMicro({
            sender,
            demanderFlux: async () => {
                throw domError('NotAllowedError');
            },
            surEtat: (etat, detail) => etats.push([etat, detail]),
        });

        // Does NOT THROW: the microphone never kills a working session
        // (spec §10). `resolves` would fail if the promise rejected.
        await expect(micro.basculer()).resolves.toBe('refuse');
        expect(micro.etat()).toBe('refuse');
        expect(sender.recus).toEqual([]);

        // The detail carries HOW TO RESTORE the permission (spec §10), not
        // only the fact of the refusal: without it the user who clicked
        // "block" once has no way left to go back.
        const [, detail] = etats[0];
        expect(detail).toMatch(/address bar/);
    });

    it("the absence of an input device leads to « unavailable », not to « refused »", async () => {
        const sender = fauxSender();
        const micro = attacherMicro({
            sender,
            demanderFlux: async () => {
                throw domError('NotFoundError');
            },
        });

        // Two DISTINCT situations in the table of spec §10: "permission
        // refused" wants a message to restore it, "no input
        // device" wants a disabled button. Confusing them would send
        // the user to adjust a permission that is not at fault.
        await expect(micro.basculer()).resolves.toBe('indisponible');
        expect(micro.etat()).toBe('indisponible');
    });

    it('a stream without an audio track leads to « unavailable », not to « active »', async () => {
        const sender = fauxSender();
        const micro = attacherMicro({
            sender,
            demanderFlux: async () => ({ getAudioTracks: () => [] }) as unknown as MediaStream,
        });

        await expect(micro.basculer()).resolves.toBe('indisponible');
        expect(sender.recus).toEqual([]);
    });

    it('two quick clicks request the stream only once', async () => {
        const piste = faussePiste();
        let debloquer!: (flux: MediaStream) => void;
        const enAttente = new Promise<MediaStream>((resolve) => {
            debloquer = resolve;
        });
        const demanderFlux = vi.fn(() => enAttente);
        const micro = attacherMicro({ sender: fauxSender(), demanderFlux });

        // The second click lands while the permission dialog
        // is still open: without a guard, it would request a SECOND stream, hence
        // a second track — and the first would leak, never stopped, microphone
        // open for the life of the page.
        const premier = micro.basculer();
        const second = micro.basculer();
        debloquer(fauxFlux(piste));

        await expect(premier).resolves.toBe('actif');
        await expect(second).resolves.toBe('ferme');
        expect(demanderFlux).toHaveBeenCalledTimes(1);
    });

    it('a detach during the stream request stops the obtained track', async () => {
        const piste = faussePiste();
        let debloquer!: (flux: MediaStream) => void;
        const enAttente = new Promise<MediaStream>((resolve) => {
            debloquer = resolve;
        });
        const sender = fauxSender();
        const micro = attacherMicro({ sender, demanderFlux: () => enAttente });

        // The session ends while the dialog is open, and
        // the user allows AFTERWARDS. Without this guard, the track arrives into
        // the void: nobody holds it any more, `detacher()` already ran,
        // and the microphone stays open until the tab is closed.
        const bascule = micro.basculer();
        micro.detacher();
        debloquer(fauxFlux(piste));

        await expect(bascule).resolves.toBe('ferme');
        expect(piste.arretee).toBe(true);
        expect(sender.recus).toEqual([]);
    });

    it('after detaching, a click requests nothing any more', async () => {
        const demanderFlux = vi.fn(async () => fauxFlux(faussePiste()));
        const micro = attacherMicro({ sender: fauxSender(), demanderFlux });

        micro.detacher();
        await expect(micro.basculer()).resolves.toBe('ferme');
        expect(demanderFlux).not.toHaveBeenCalled();
    });

    it('detaching an ACTIVE mic really switches the track off', async () => {
        const piste = faussePiste();
        const sender = fauxSender();
        const micro = attacherMicro({ sender, demanderFlux: async () => fauxFlux(piste) });

        await micro.basculer();
        micro.detacher();

        // `webrtc.ts::close()` already stops the sender's track, but a session
        // end must ALSO bring the button state back to "closed": that is what
        // `main.ts` gets by calling this detach.
        expect(piste.arretee).toBe(true);
        expect(micro.etat()).toBe('ferme');
    });

    it('a refusal is not final: the next click retries', async () => {
        const piste = faussePiste();
        let premierAppel = true;
        const demanderFlux = vi.fn(async () => {
            if (premierAppel) {
                premierAppel = false;
                throw domError('NotAllowedError');
            }
            return fauxFlux(piste);
        });
        const micro = attacherMicro({ sender: fauxSender(), demanderFlux });

        // The user blocked, then restored the permission in the site
        // settings — exactly what the message of the "refused" state asks
        // them to do. A terminal state would make this advice inapplicable.
        await expect(micro.basculer()).resolves.toBe('refuse');
        await expect(micro.basculer()).resolves.toBe('actif');
        expect(demanderFlux).toHaveBeenCalledTimes(2);
    });
});

describe('attacherBoutonMicro — the button and its states', () => {
    it("the button stays HIDDEN as long as the agent has not announced `mic: true`", () => {
        const bouton = fauxBouton();
        attacherBoutonMicro({
            bouton,
            sender: fauxSender(),
            demanderFlux: async () => fauxFlux(faussePiste()),
        });

        expect(bouton.hidden).toBe(true);
    });

    it("ABSENT `mic` leaves the button hidden — an old agent does not carry it", () => {
        const bouton = fauxBouton();
        const controle = attacherBoutonMicro({
            bouton,
            sender: fauxSender(),
            demanderFlux: async () => fauxFlux(faussePiste()),
        });

        // ⚠️ Spec §10: the field is optional and ITS ABSENCE MEANS `false`. A
        // recent client talking to an agent from before project E must not
        // offer a button that would lead nowhere. The rule lives HERE, in
        // the tested module, rather than in an `if` of `main.ts` that nothing
        // would exercise.
        controle.annoncerDisponibilite(undefined);
        expect(bouton.hidden).toBe(true);
        controle.annoncerDisponibilite(false);
        expect(bouton.hidden).toBe(true);
        controle.annoncerDisponibilite(true);
        expect(bouton.hidden).toBe(false);
    });

    it('a click switches on, a second one switches off, and the button carries its state', async () => {
        const bouton = fauxBouton();
        const piste = faussePiste();
        const sender = fauxSender();
        const controle = attacherBoutonMicro({
            bouton,
            sender,
            demanderFlux: async () => fauxFlux(piste),
        });
        controle.annoncerDisponibilite(true);

        expect(bouton.dataset.etat).toBe('ferme');

        bouton.cliquer();
        await controle.enCours();
        expect(bouton.dataset.etat).toBe('actif');
        expect(sender.recus).toEqual([piste]);

        bouton.cliquer();
        await controle.enCours();
        expect(bouton.dataset.etat).toBe('ferme');
        expect(piste.arretee).toBe(true);
    });

    it('« unavailable » DISABLES the button, « refused » leaves it clickable', async () => {
        const bouton = fauxBouton();
        let nom = 'NotFoundError';
        const controle = attacherBoutonMicro({
            bouton,
            sender: fauxSender(),
            demanderFlux: async () => {
                throw domError(nom);
            },
        });
        controle.annoncerDisponibilite(true);

        bouton.cliquer();
        await controle.enCours();
        // Spec §10, "no input device" row: button DISABLED.
        expect(bouton.dataset.etat).toBe('indisponible');
        expect(bouton.disabled).toBe(true);

        nom = 'NotAllowedError';
        const autre = fauxBouton();
        const second = attacherBoutonMicro({
            bouton: autre,
            sender: fauxSender(),
            demanderFlux: async () => {
                throw domError(nom);
            },
        });
        second.annoncerDisponibilite(true);
        autre.cliquer();
        await second.enCours();
        // Spec §10, "permission refused" row: the message says how to
        // restore it, so the button must stay clickable to retry.
        expect(autre.dataset.etat).toBe('refuse');
        expect(autre.disabled).toBe(false);
    });

    it("the detail of the two failure states is passed up to the caller", async () => {
        const bouton = fauxBouton();
        const messages: string[] = [];
        const controle = attacherBoutonMicro({
            bouton,
            sender: fauxSender(),
            demanderFlux: async () => {
                throw domError('NotAllowedError');
            },
            surMessage: (texte) => messages.push(texte),
        });
        controle.annoncerDisponibilite(true);

        bouton.cliquer();
        await controle.enCours();

        // A single message, and it carries the remedy — not only the fact of the
        // refusal (spec §10).
        expect(messages).toHaveLength(1);
        expect(messages[0]).toMatch(/address bar/);
    });

    it('`detacher` switches the track off, hides the button and removes its listener', async () => {
        const bouton = fauxBouton();
        const piste = faussePiste();
        const controle = attacherBoutonMicro({
            bouton,
            sender: fauxSender(),
            demanderFlux: async () => fauxFlux(piste),
        });
        controle.annoncerDisponibilite(true);
        bouton.cliquer();
        await controle.enCours();

        controle.detacher();

        // Without removing the listener, a click after the session end
        // would restart a permission request on a dead session.
        expect(piste.arretee).toBe(true);
        expect(bouton.hidden).toBe(true);
        expect(bouton.listenerCount()).toBe(0);
    });
});
