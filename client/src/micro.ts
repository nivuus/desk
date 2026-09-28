// The browser's microphone towards the agent (workstream E): the toggle, and its
// four states.
//
// **Permission is requested ON CLICK, never at session opening**
// (spec §9). The upstream transceiver, for its part, is declared from the initial offer and
// WITHOUT A TRACK (`webrtc.ts`): that is what allows turning the mic on by a
// simple `replaceTrack`, without a second offer, whereas `connectSession` has
// no path to renegotiate.
//
// **Switching off is REAL.** `replaceTrack(null)` AND `track.stop()`.
// `enabled = false` alone would leave the device open and Chrome's indicator
// lit: spec §9 calls this visual lie unacceptable "on
// this function precisely", and it is the only function of the product that
// captures the user at home.
//
// Dependencies are INJECTED rather than read from the global objects
// (`navigator.mediaDevices`), like `audio.ts`, `resize.ts` and
// `visibilite.ts`: that is what makes the module testable without a DOM.

/// The four states, and what each one asks of the button (spec §10).
///
/// Spec §9 announces THREE — "closed, active, refused by the browser".
/// The fourth comes from its own table §10, which distinguishes "permission
/// refused by the user" (a message to restore it) from "no input
/// device on the browser side" (button disabled, with
/// the explanation). Confusing them would send one to adjust a permission that is not
/// at fault.
export type EtatMicro = 'ferme' | 'actif' | 'refuse' | 'indisponible';

/// The capture constraints (spec §7).
///
/// **Echo cancellation is the BROWSER's**, a decision of spec §4:
/// it is the only place that knows both the captured stream and the
/// played-back stream. ⚠️ It is structurally INCOMPLETE with multiple windows
/// (decision 7 of this workstream's plan): each page only cancels what IT
/// plays back, and has no knowledge of the sound of neighbouring windows, which
/// nonetheless come out of the same speaker. It is not repairable here.
///
/// ⚠️ **PRIVATE, and its test COPIES the literal instead of importing it.** A
/// first wording exported it and the test asserted
/// `toHaveBeenCalledWith(CONTRAINTES)`: both sides of the equality then read
/// the SAME object, and replacing the three constraints with `audio: true`
/// left the test GREEN — mutation played, seen green, fixed. It is the
/// "check unable to fail" this repository has paid for since D6: a reference
/// value is not shared with what checks it.
const CONTRAINTES: MediaStreamConstraints = {
    audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
};

/// What the "refused" state message must carry: HOW to restore the
/// permission (spec §10), not only the fact of the refusal. Without that,
/// the user who clicked "block" once has no visible way left
/// to go back — Chrome never offers the dialog again.
const DETAIL_REFUS =
    "microphone refused — allow it in the site settings (icon to the left of the address bar), then click again";

/// Same for the absence of a device.
const DETAIL_SANS_PERIPHERIQUE = "no microphone detected on this computer";

/// The error `name`s `getUserMedia` uses for a PERMISSION refusal,
/// and those alone. Everything else — `NotFoundError`, `NotReadableError`,
/// `OverconstrainedError`, `AbortError`, any failure — describes an
/// absent or unusable device, not a choice of the user.
///
/// ⚠️ The fallback is "unavailable", not "refused", and it is
/// deliberate: displaying "allow the microphone" to someone whose mic is
/// unplugged sends them looking for a setting that will change nothing.
const REFUS_DE_PERMISSION = new Set(['NotAllowedError', 'SecurityError', 'PermissionDeniedError']);

export interface OptionsMicro {
    /// The sender of the upstream track, returned by `connectSession`.
    sender: Pick<RTCRtpSender, 'replaceTrack'>;
    /// `navigator.mediaDevices.getUserMedia` en production.
    demanderFlux: (contraintes: MediaStreamConstraints) => Promise<MediaStream>;
    /// Called at EACH state change. `detail` carries the message meant for
    /// the user for the two failure states, and nothing for the two others.
    surEtat?: (etat: EtatMicro, detail?: string) => void;
}

export interface Micro {
    /// Turns on if off, off if on. **NEVER rejects**: the mic
    /// never kills a working session (spec §10). Returns the state reached.
    basculer(): Promise<EtatMicro>;
    etat(): EtatMicro;
    /// End of session: really switches off, and makes the toggle inert.
    detacher(): void;
}

export function attacherMicro(options: OptionsMicro): Micro {
    let etat: EtatMicro = 'ferme';
    let piste: MediaStreamTrack | undefined;
    /// A stream request is in flight: the permission dialog is
    /// open, and the user can stay there a long time.
    let enVol = false;
    let detache = false;

    const annoncer = (nouvel: EtatMicro, detail?: string): EtatMicro => {
        etat = nouvel;
        options.surEtat?.(nouvel, detail);
        return etat;
    };

    /// Switching off, and the only one. **`stop()` BEFORE `replaceTrack(null)`**: the
    /// `stop()` is what releases the device and turns off the indicator, and it
    /// must survive a `replaceTrack` that would reject — which happens on
    /// an already closed `RTCPeerConnection` (`InvalidStateError`).
    const eteindre = (): void => {
        piste?.stop();
        piste = undefined;
    };

    const classer = (error: unknown): EtatMicro => {
        const nom = error instanceof Error ? error.name : '';
        if (REFUS_DE_PERMISSION.has(nom)) return annoncer('refuse', DETAIL_REFUS);
        // The browser's message is attached when it is not a plain
        // `NotFoundError`: on a failure, it is the only information we have.
        const detail =
            nom === 'NotFoundError' || nom === 'DevicesNotFoundError'
                ? DETAIL_SANS_PERIPHERIQUE
                : `mic unavailable: ${error instanceof Error ? error.message : String(error)}`;
        return annoncer('indisponible', detail);
    };

    const allumer = async (): Promise<EtatMicro> => {
        enVol = true;
        try {
            const flux = await options.demanderFlux(CONTRAINTES);
            const obtenue = flux.getAudioTracks()[0];

            // ⚠️ The session may have ended WHILE the dialog
            // was open, and the user granted afterwards. Without this
            // guard, the track arrives into the void: no one holds it
            // any more, `detacher()` has already run, and the mic stays open
            // until the tab is closed — exactly the leak
            // real switching off exists to prevent.
            if (detache) {
                obtenue?.stop();
                return etat;
            }

            // `getUserMedia({audio})` always returns a track in practice; if it
            // returns none, it is "unavailable" and not "active" — otherwise
            // the button would light up on an empty stream, and spec §10 says
            // a silent failure is the worst case.
            if (!obtenue) return annoncer('indisponible', DETAIL_SANS_PERIPHERIQUE);

            piste = obtenue;
            await options.sender.replaceTrack(obtenue);
            return annoncer('actif');
        } catch (error) {
            // The track may have been obtained before `replaceTrack` rejected:
            // stopping it is the only way not to leave the mic open
            // on an error path.
            eteindre();
            return classer(error);
        } finally {
            enVol = false;
        }
    };

    return {
        etat: () => etat,

        async basculer(): Promise<EtatMicro> {
            if (detache) return etat;
            // Two quick clicks: the second lands while the first one's
            // dialog is still open. Without this guard, it
            // would request a SECOND stream, hence a second track — and the
            // first would leak, never stopped.
            if (enVol) return etat;

            if (piste) {
                eteindre();
                // `replaceTrack(null)` after the `stop()`, and tolerated if it
                // rejects: on an already closed connection it throws
                // `InvalidStateError`, which must not prevent the state from
                // going back to "closed" nor make `basculer` reject.
                await options.sender.replaceTrack(null).catch(() => {});
                return annoncer('ferme');
            }

            // A refusal or an unavailability are NOT terminal: the
            // "refused" state tells the user to go and restore the permission,
            // and that advice would be inapplicable if the next click did not
            // retry. A replugged mic is the symmetric case.
            return allumer();
        },

        detacher(): void {
            detache = true;
            eteindre();
            annoncer('ferme');
        },
    };
}

// ── The button ──────────────────────────────────────────────────────────────
//
// Separated from `attacherMicro` for the same reason `attachFullscreen` is from
// `armerPleinEcran`: the toggle is a state machine that knows no
// DOM, the button is what shows it. The second depends on the first, never
// the reverse.

/// What this module needs from the button, and nothing more.
export interface BoutonMicro {
    hidden: boolean;
    disabled: boolean;
    title: string;
    dataset: { etat?: string };
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

export interface OptionsBoutonMicro extends OptionsMicro {
    bouton: BoutonMicro;
    /// The message meant for the user, on the two failure states
    /// only. `main.ts` pushes it into the status banner.
    surMessage?: (texte: string) => void;
}

export interface ControleBoutonMicro {
    /// To call on the agent's `ready` message, with `message.mic` AS
    /// IS — `undefined` included.
    annoncerDisponibilite(mic: boolean | undefined): void;
    /// To call on each `mic-state` from the agent (block E3): is this mic
    /// HEARD by the VM?
    ///
    /// 🔴 **It is NOT a fifth state, and above all it is not
    /// `'refuse'`.** `'refuse'` designates the PERMISSION refusal by the
    /// browser, which is repaired in the site's settings; this one is
    /// repaired by closing the other window. Confusing them would send the user
    /// to adjust a permission that is not at fault — the exact defect the
    /// fourth state (`'indisponible'`) already exists to avoid.
    ///
    /// 🔴 **The state stays `'actif'`, and lying would be forbidden**: the track IS
    /// open, the browser EMITS, and Chrome's capture indicator is
    /// lit. Turning off the button of a window that really captures is
    /// exactly the visual lie spec §9 "Privacy" calls
    /// unacceptable "on this function precisely".
    annoncerExclusivite(granted: boolean): void;
    /// The toggle in progress, so that the test can await it. In production
    /// no one awaits it: a click is asynchronous by nature.
    enCours(): Promise<unknown>;
    detacher(): void;
}

/// What the button displays on hover, per state.
const TITRES: Record<EtatMicro, string> = {
    ferme: 'Microphone — click to talk',
    actif: 'Microphone active — click to mute',
    refuse: 'Microphone refused — see the message',
    indisponible: 'No microphone available',
};

/// What the button displays when it really captures but ANOTHER
/// window holds the VM's cable (block E3).
///
/// ⚠️ **It says both halves, and the order matters**: first that the mic
/// IS open — otherwise the user would believe the button broken while
/// Chrome's indicator is lit —, then that this window is not
/// heard, and because of what.
const TITRE_NON_ENTENDU =
    'Microphone active — but another window holds the VM microphone: this one is not heard there';

/// The same fact, as a banner. ⚠️ **The wording is a HUMAN JUDGEMENT**, and
/// it joins the list this repository has kept since `BPP_MIN`: no measurement will
/// say whether it is clear. No one has read it on screen to date.
const DETAIL_NON_ENTENDU =
    'Your microphone is open, but another window holds the VM microphone — close it, or mute its microphone, to be heard from this one.';

/// And the return, which must be said: a banner that rises without ever
/// coming down would suggest the defect after it disappeared.
const DETAIL_ENTENDU = 'Your microphone is heard by the VM again.';

export function attacherBoutonMicro(options: OptionsBoutonMicro): ControleBoutonMicro {
    const { bouton, surMessage } = options;
    let enVol: Promise<unknown> = Promise.resolve();
    /// Last exclusivity verdict RECEIVED. `true` at first: as long as the agent
    /// has said nothing, there is no refusal to show — and a lone window,
    /// which is the common case, will never receive anything but `true`.
    let entendu = true;

    const micro = attacherMicro({
        ...options,
        surEtat(etat, detail) {
            bouton.dataset.etat = etat;
            bouton.title = TITRES[etat];
            // ⚠️ Every state transition RESETS the exclusivity verdict.
            // Without that, turning the mic off then on again while the other
            // window still holds the cable would leave `entendu === false`:
            // the next `mic-state { granted: false }` would be seen as "not
            // a change", and the banner would never rise again. It is the
            // client counterpart of the agent-side transition.
            entendu = true;
            // ⚠️ ONLY "unavailable" disables. A permission refusal leaves
            // the button clickable, because its message says to go and restore
            // the permission then click again (spec §10): disabling it
            // would make that advice inapplicable.
            bouton.disabled = etat === 'indisponible';
            if (detail) surMessage?.(detail);
            options.surEtat?.(etat, detail);
        },
    });

    // The initial state is written right away: without it, the CSS would have no
    // selector to hook onto before the first click.
    bouton.dataset.etat = micro.etat();
    bouton.title = TITRES[micro.etat()];

    const onClick = (): void => {
        // `basculer` never rejects (spec §10); the `catch` is a safety belt
        // so that nothing surfaces as an "unhandled rejection" if this invariant
        // came to be broken by a future change.
        enVol = micro.basculer().catch((error: unknown) => {
            console.warn('mic toggle failed', error);
        });
    };
    bouton.addEventListener('click', onClick);

    return {
        annoncerExclusivite(granted) {
            // ⚠️ **Nothing is done as long as the mic is not ACTIVE.** The agent
            // only emits this message at the first upstream packet, hence with the mic
            // open; but a `mic-state` arriving after a switch-off
            // (a frame in flight, a loop round late) would overwrite the
            // title of a closed button with a label speaking of an open
            // mic. The only state where this fact makes sense is `'actif'`.
            if (micro.etat() !== 'actif') return;
            if (granted) {
                // Only puts back the nominal title if we had announced the
                // opposite: without this guard, each `mic-state { granted: true }`
                // would push a "heard again" banner to a window that
                // never stopped being heard.
                if (!entendu) surMessage?.(DETAIL_ENTENDU);
                bouton.title = TITRES.actif;
            } else {
                bouton.title = TITRE_NON_ENTENDU;
                surMessage?.(DETAIL_NON_ENTENDU);
            }
            entendu = granted;
        },
        annoncerDisponibilite(mic) {
            // ⚠️ `mic` is read AS IS, and its absence means `false` (spec §10):
            // an agent predating workstream E does not carry the field, and a
            // button leading nowhere is worse than no button.
            // `!!` and not `!== false`: it is falsiness that decides.
            bouton.hidden = !mic;
        },
        enCours: () => enVol,
        detacher() {
            bouton.removeEventListener('click', onClick);
            micro.detacher();
            bouton.hidden = true;
        },
    };
}
