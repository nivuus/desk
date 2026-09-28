import { attachInput } from './input';
import { connectSession } from './webrtc';
import { attachStats } from './stats';
import { armerLeSon } from './audio';
import { createStatus } from './status';
import { createTerminalScreenInDOM } from './ecran-terminal';
import { attachPointerAuDOM } from './pointer';
import { attachGamepadAuDOM } from './gamepad';
import { armerPleinEcranAuDOM, attachFullscreenAuDOM } from './fullscreen';
import { texteLien } from './lien';
import { annoncerLeViewportInitial, annonceurDeViewport } from './viewport-dom';
import { attachVisibilite } from './visibilite';
import { attacherBoutonMicro } from './micro';
import { attacherAccentAuDOM } from './accent-dom';
import { attacherPressePapierAuDOM } from './presse-papier-dom';
import type { Recu } from './presse-papier';
import { attacherResizeAuDOM } from './resize-dom';
import { adresseSignaling } from './adresse-plateforme';
import { sessionIdFromParams } from './session-id';

const video = document.querySelector<HTMLVideoElement>('#remote')!;
const statusElement = document.querySelector<HTMLDivElement>('#status')!;
const statsElement = document.querySelector<HTMLDivElement>('#stats')!;
const fullscreenElement = document.querySelector<HTMLButtonElement>('#fullscreen')!;
const microElement = document.querySelector<HTMLButtonElement>('#micro')!;

// Single write point of the status banner: protects a TERMINAL message
// (end of session, failure) against being overwritten by an ordinary message
// arriving after it. See `status.ts` for the complete justification.
// The SECOND target is the full-frame screen of terminal states (S4, task 9):
// `createStatus` raises it for a `terminal` message and for it alone.
const statut = createStatus(statusElement, createTerminalScreenInDOM());

// The session and signaling can be set through the URL to ease
// trials: ?session=abc123&signaling=ws://192.168.3.2:8080/signal
//
// 🔴 WITHOUT A PARAMETER, THE ADDRESS FOLLOWS THE PAGE'S PROTOCOL — `wss:` if the page
// is on `https:`, `ws:` otherwise —, AND ITS PORT. The earlier literal,
// `ws://<host>:8080`, was MIXED CONTENT behind the TLS proxy: the
// browser refused the connection, the page loaded anyway, and the
// media never got established. The rule lives in `adresse-plateforme.ts`,
// which is PURE and tested — no Node test can see a mixed content refusal.
const params = new URLSearchParams(window.location.search);
const sessionId = sessionIdFromParams(params);
const signalingUrl = adresseSignaling(window.location, params.get('signaling'));

// 🔴 WITHOUT A PARAMETER, THIS PAGE REFUSES AND STOPS THERE — it NO LONGER invents
// a `demo` session (see `session-id.ts`: that vestige, found in PRODUCTION
// on August 30th, 2026, made `connectSession` fail without a token, with a
// message INDISTINGUISHABLE from a real network failure). `throw` stops
// the evaluation of THIS ES module: no connection is attempted afterwards.
if (sessionId === undefined) {
    statut.show('Aucune session indiquée — ouvrez une application depuis le hub.', {
        terminal: true,
        ton: 'danger',
    });
    throw new Error('main.ts : aucun paramètre `session` dans l’URL');
}

// The viewport announcement lives in `viewport-dom.ts`, extracted in batch 33: this
// file was at EXACTLY 500 lines and the addition made it cross its
// ceiling. It is its THIRD extraction — see the extracted file's header.
//
// The INITIAL announcement goes out BEFORE any WebRTC connection: it is what decides
// the resolution of the virtual output, and nothing can be created on the agent side
// before it is known.
annoncerLeViewportInitial(sessionId);

// Timer of the audio banner ("click to enable sound"), shared between
// `onControl` (wired before the connection promise resolves) and the
// `.then()` where `armerLeSon` is called (after). It no longer needs to be
// guarded by an end-of-session flag: `statut` carries that guard at
// the root, for all writers. It is cancelled anyway at the end of the
// session so as not to leave an obsolete timer running for nothing.
let bandeau: number | undefined;

// Like `bandeau` above: `onControl` is wired before the promise of
// `connectSession` resolves, so these variables must exist before the call,
// otherwise they would be in the temporal dead zone at the first message received.
let pointeur: ReturnType<typeof attachPointerAuDOM> | undefined;
let manette: ReturnType<typeof attachGamepadAuDOM> | undefined;
let detacherPleinEcran: ReturnType<typeof attachFullscreenAuDOM> | undefined;
let detacherArmement: (() => void) | undefined;
let detacherVisibilite: ReturnType<typeof attachVisibilite> | undefined;
// The mic (workstream E). Declared here for the same reason as its neighbours:
// `onControl` is wired BEFORE the promise of `connectSession` resolves,
// and the `ready` message — which decides whether the button appears — can arrive before
// the `.then()` that builds the control.
let micro: ReturnType<typeof attacherBoutonMicro> | undefined;
// The VM's clipboard (sub-block P1). Declared here for the same reason
// as its neighbours: `onControl` is wired BEFORE the promise of
// `connectSession` resolves. What becomes of a message arriving before attachment
// is written on `PressePapierAttache.recevoir`.
let pressePapier: ReturnType<typeof attacherPressePapierAuDOM> | undefined;
// The window's accent colour (sub-block A1). ⚠️ **A `const`, where its
// neighbours are `let | undefined`**, and the reason is that it depends on NOTHING
// : neither the session, nor the `.then()`, nor an earlier message — only the
// document, which already exists. There is therefore no window during which a
// message could arrive without a recipient, and no `?.` to write.
const accent = attacherAccentAuDOM({
    lireToken: (nom) =>
        getComputedStyle(document.documentElement).getPropertyValue(nom).trim(),
    // 🔴 `documentElement`, NEVER `document.body`: a token set on `body`
    // is invisible to `getComputedStyle(document.documentElement)`, and it is
    // the red run of the acceptance's criterion ②.
    poserToken: (nom, value) => document.documentElement.style.setProperty(nom, value),
});
/// Has the agent announced `Capabilities.clipboard`?
///
/// **A `let` reread by a closure, never a value passed at attach time**:
/// `Capabilities` arrives before `Ready` but nothing guarantees it precedes
/// `attachInput`, and a boolean frozen at mount time would be `false` forever.
/// It is the same order independence as the rest of this file.
///
/// `undefined` on a pre-P2 agent: `Boolean(undefined)` is `false`,
/// so nothing is armed, and `Ctrl+V` keeps its earlier behaviour.
let collageArme = false;
// `mic` as the agent announced it, `undefined` included. Remembered because
// `ready` can precede the button's construction: without that, an announcement
// arriving early would be lost and the button would stay hidden forever,
// WITH NOTHING to say so — the silent failure mode this repository
// paid for on the visibility announcement (see below).
let micAnnonce: boolean | undefined;
// The last `clipboard` received, remembered for exactly the same reason as
// `micAnnonce` just above: `onControl` is wired BEFORE the promise of
// `connectSession` resolves, so a message arriving in that interval
// would be lost WITH NOTHING to say so. And since P3 the agent emits the current state
// at the window's REGISTRATION, which falls precisely in that interval.
//
// ⚠️ These two lines are WIRING, and they are covered by NO
// TEST — `main.ts` has none and cannot have any. The RULE they
// route — "replay the remembered one at mount" — lives in
// `presse-papier-dom.ts`, where four tests hold it. Their only end-to-end
// check is the acceptance's criterion ①: a window attached AFTER the
// copy.
let lastClipboard: Recu | undefined;
let manetteAnnoncee = false;
let bandeauManette: number | undefined;

// Timer of the network banner: only a message WITHOUT an alert hides itself
// (same pattern as the "ready" banner below). An alert message stays
// displayed as long as the condition lasts; cancelling it before arming a new one
// prevents an obsolete hiding from erasing a warning that arrived meanwhile.
let bandeauLien: number | undefined;

connectSession({
    signalingUrl,
    sessionId,
    video,
    onStatus: (message) => statut.show(message),
    onControl(message) {
        if (message.type === 'ready') {
            statut.show(`prêt — ${message.width}×${message.height}`);
            setTimeout(() => statut.masquer(), 1500);
            // The mic button ONLY appears if the agent says it has the cable
            // (spec §10). `message.mic` is passed AS IS: the rule "its
            // absence means false" lives in `micro.ts`, where a test guards it,
            // rather than in an `if` here nothing would exercise.
            micAnnonce = message.mic;
            micro?.annoncerDisponibilite(micAnnonce);
        } else if (message.type === 'session-end') {
            window.clearTimeout(bandeau);
            window.clearTimeout(bandeauManette);
            window.clearTimeout(bandeauLien);
            // Without these three detachments, the gamepad's 4 ms `setInterval`
            // (and the pointer/fullscreen listeners) keep
            // running after the end of the session — nothing else
            // stops them, the page stays open as long as the user does not
            // close it themselves.
            pointeur?.detacher();
            manette?.detacher();
            detacherPleinEcran?.();
            detacherArmement?.();
            detacherVisibilite?.();
            // End of session: turning the mic off must be REAL, and
            // `session.close()` is not called on this path. Without this
            // Chrome's indicator would stay lit after the end (spec §9).
            micro?.detacher();
            // Same reason as the detachments above: the `focus`
            // listener would otherwise outlive the end of the session and write the
            // local clipboard for a dead session.
            pressePapier?.detacher();
            // `neutre`: the user closed the remote application, it
            // is not an error. The tone ONLY serves the terminal screen.
            statut.show(`session terminée : ${message.reason}`, {
                terminal: true,
                ton: 'neutre',
            });
        } else if (message.type === 'pointer') {
            pointeur?.surMessagePointeur(message.visible, message.shape);
        } else if (message.type === 'rumble') {
            manette?.surVibration(message.left, message.right);
        } else if (message.type === 'asleep') {
            if (message.asleep) {
                const texte =
                    message.reason === 'evincee'
                        ? 'image figée : trop de fenêtres actives'
                        : 'image figée : fenêtre masquée';
                // `persistant`: the state lasts as long as the window sleeps, it
                // must not be erased by a neighbouring banner's timer.
                statut.show(texte, { persistant: true });
            } else {
                // `masquer()` deliberately protects a persistent message: the
                // wake-up must therefore explicitly lift that persistence,
                // otherwise the "image frozen: …" banner would stay displayed
                // forever after the real wake-up (see status.ts).
                statut.expirer();
            }
        } else if (message.type === 'fullscreen') {
            // SINGLE direction: the Windows application decides, the browser follows.
            // Leaving requires no user activation; entering does — hence
            // the arming.
            detacherArmement?.();
            detacherArmement = undefined;
            if (message.active) {
                detacherArmement = armerPleinEcranAuDOM(document.documentElement);
            } else {
                void document.exitFullscreen().catch(() => {
                    // Leaving a fullscreen we do not have is without
                    // consequence: the user may have left it themselves.
                });
            }
        } else if (message.type === 'link') {
            const t = texteLien(message);
            window.clearTimeout(bandeauLien);
            if (t.alerte) {
                // `persistant` protects this message against the `masquer()` of a
                // NEIGHBOURING timer ("ready" banner, "gamepad detected",
                // etc.) whose existence this module does not — and must not —
                // know about. Otherwise an alert displayed within
                // the firing window of one of those banners would disappear while
                // the network is still degraded. The status banner
                // also already protects terminal messages: a
                // network warning will not overwrite an end of session.
                statut.show(t.resume, { persistant: true });
            } else {
                // Routine information: it clears itself, like
                // the "ready" banner. Displaying an ordinary message lifts the
                // persistence of a previous alert (see status.ts), so
                // a return to `bonne` makes it stop by itself.
                statut.show(t.resume);
                bandeauLien = window.setTimeout(() => statut.masquer(), 1500);
            }
        } else if (message.type === 'capabilities') {
            // `gamepad: false` means the remote machine can offer
            // NO gamepad, not that the client has none plugged in: a
            // message distinct from the gamepad banner's below, otherwise
            // the user would believe their gamepad was at fault.
            if (!message.gamepad) {
                statut.show('manette indisponible sur cette machine');
                setTimeout(() => statut.masquer(), 4000);
            }
            // No logic here either: this flag only GATES
            // the keyboard exception of `input.ts`. Without it, `PRESSE_PAPIER=0`
            // would give the worst of both worlds — the client would hold back
            // `Ctrl+V` while no one would inject it on the VM side.
            collageArme = Boolean(message.clipboard);
        } else if (message.type === 'clipboard') {
            // No logic here: the whole decision — write or defer,
            // report a refusal, shout at the second failure — lives in the attached
            // module, itself backed by `presse-papier.ts`, pure and tested.
            // The memory is set BEFORE the `?.`, never in an `else`
            // branch: setting it in the `else` would make the two paths diverge
            // the day one of them changed.
            lastClipboard = { texte: message.text, octets: message.bytes };
            pressePapier?.recevoir(lastClipboard);
        } else if (message.type === 'accent') {
            // No logic here: conform, refuse, set — everything lives in
            // `accent-dom.ts`, backed by `accent.ts`, pure and tested.
            accent.recevoir(message.couleur);
        } else if (message.type === 'mic-state') {
            // One LINE that delegates: the whole doctrine lives in `micro.ts`.
            micro?.annoncerExclusivite(message.granted);
        }
    },
})
    .then((session) => {
        attachInput({
            video,
            channel: session.inputChannel,
            // `window` and not `video`: that is where the keyboard listeners
            // already lived before P2.
            clavier: window,
            collageArme: () => collageArme,
        });
        attachStats(session.pc, statsElement);
        video.focus();

        const envoyer = (payload: Uint8Array): void => {
            if (session.inputChannel.readyState === 'open') {
                // Same assertion as in input.ts: `RTCDataChannel.send`
                // requires a `Uint8Array<ArrayBuffer>`, yet the buffers produced
                // by `proto/ts/input.ts` are always backed by a real
                // `ArrayBuffer` in practice — only the typing is too broad.
                session.inputChannel.send(payload as Uint8Array<ArrayBuffer>);
            }
        };

        pointeur = attachPointerAuDOM({
            envoyer,
            surEchec: () => statut.show('cliquez dans l\'image pour prendre la souris'),
        });

        manette = attachGamepadAuDOM({
            envoyer,
            surPresence: (present) => {
                if (present && !manetteAnnoncee) {
                    manetteAnnoncee = true;
                    window.clearTimeout(bandeauManette);
                    statut.show('manette détectée');
                    setTimeout(() => statut.masquer(), 1500);
                }
            },
        });

        // The Gamepad API exposes NO gamepad before a press on one of
        // its buttons: a plugged-in, silent gamepad is indistinguishable
        // from the absence of a gamepad. We say so, rather than letting one conclude
        // there is a failure — same pattern as the audio banner below, including
        // the delay: no point explaining it to someone who has already pressed.
        bandeauManette = window.setTimeout(() => {
            if (!manetteAnnoncee) statut.show('manette : appuyez sur un bouton pour l\'activer');
        }, 4000);

        detacherPleinEcran = attachFullscreenAuDOM({ bouton: fullscreenElement, cible: document.documentElement });

        // The mic. `micSender` comes from the `sendonly` transceiver declared without
        // a track in the initial offer: turning on is just a `replaceTrack`, and
        // requires no renegotiation.
        micro = attacherBoutonMicro({
            bouton: microElement,
            sender: session.micSender,
            // Permission is only requested HERE, on click, never at
            // session opening (spec §9).
            demanderFlux: (contraintes) => navigator.mediaDevices.getUserMedia(contraintes),
            // Only the two failure states carry a message; it says
            // HOW to restore the permission, not only that it is missing.
            // `persistant`: the user must have time to read it and
            // go and follow it, a neighbouring timer must not erase it.
            surMessage: (texte) => statut.show(texte, { persistant: true }),
        });
        // `ready` may have arrived BEFORE this point: replaying it is the only way
        // for the button to appear in that case. The callback is harmless if
        // the announcement has not come yet (`undefined` leaves it hidden).
        micro.annoncerDisponibilite(micAnnonce);

        // The VM's clipboard. `writeText` ALONE — never `readText`:
        // see the header of `presse-papier-dom`. `window` carries the `focus`
        // that `document` does not carry, as for `armerLeSon` below.
        pressePapier = attacherPressePapierAuDOM({            write: (texte) => navigator.clipboard.writeText(texte),
            focalise: () => document.hasFocus(),
            cible: window,
            // The CONTROL channel, never the input one: a paste
            // requires an ORDER (the Windows clipboard first, `Ctrl+V`
            // next), and the input channel is `ordered: false`. Same
            // channel, and same gesture, as `encodeResize` above.
            emettre: (message) => {
                if (session.controlChannel.readyState === 'open') {
                    session.controlChannel.send(message);
                }
            },
            // `persistant`, on the EXACT pattern of the mic: size refusal and
            // repeated failure both require a user gesture.
            surMessage: (texte) => statut.show(texte, { persistant: true }),
            // What arrived BEFORE this point, if anything. `undefined`
            // leaves the pre-P3 behaviour word for word.
            initial: lastClipboard,
        });

        // Sound starts muted and is enabled at the first gesture. A banner only
        // shows if no gesture came after a few
        // seconds — no point explaining to someone who has already clicked.
        armerLeSon({
            media: video,
            cible: window,
            surEtat(actif) {
                if (actif) {
                    window.clearTimeout(bandeau);
                    statut.masquer();
                } else {
                    bandeau = window.setTimeout(() => {
                        statut.show('cliquez pour activer le son');
                    }, 4000);
                }
            },
        });

        // `document` does not carry `focus`/`blur`: they go on `window`. The
        // target unites both sources under the interface the module expects.
        //
        // Attaching only happens once `controlChannel` is really open.
        // `connectSession` resolves right after `setRemoteDescription`: at that
        // instant the channel is still `connecting` (ICE/DTLS/SCTP have not
        // finished), and `attachVisibilite` sends its initial announcement
        // SYNCHRONOUSLY at attach time. Attaching too early would therefore make that very
        // first send fail — and if the window then stays visible and focused
        // without any `focus`/`blur`/`visibilitychange` ever firing
        // (the common case of a window opening in the foreground and
        // staying there), nothing would re-emit afterwards: exactly the silent
        // failure mode — window never woken, no `WARN`
        // on the agent side — that the cautious memorisation of `last` in
        // visibilite.ts mitigates but cannot, on its own, eliminate if
        // no second trigger ever happens.
        const startVisibilityAnnouncement = () => {
            detacherVisibilite = attachVisibilite(
                {
                    get hidden() {
                        return document.hidden;
                    },
                    get focalisee() {
                        return document.hasFocus();
                    },
                    addEventListener(nom, rappel) {
                        if (nom === 'visibilitychange') document.addEventListener(nom, rappel);
                        else window.addEventListener(nom, rappel);
                    },
                    removeEventListener(nom, rappel) {
                        if (nom === 'visibilitychange') document.removeEventListener(nom, rappel);
                        else window.removeEventListener(nom, rappel);
                    },
                },
                (charge) => {
                    // This guard is no longer the main rampart against
                    // losing the initial announcement (ensured by the wait
                    // above): it stays useful for the residual case where the
                    // channel closed between two state changes.
                    if (session.controlChannel.readyState !== 'open') return false;
                    session.controlChannel.send(charge);
                    return true;
                },
            );
        };
        if (session.controlChannel.readyState === 'open') {
            startVisibilityAnnouncement();
        } else {
            session.controlChannel.addEventListener('open', startVisibilityAnnouncement, { once: true });
        }

        // Size tracking: `ResizeObserver`, smoothing, emission, and the
        // replay when the channel opens. Extracted from this file on August 20th, 2026
        // — see the header of `resize-dom.ts`, which carries the synchronicity
        // invariant (D9's legacy no. 12) and the reason for the extraction.
        //
        // 🔴 **SYNCHRONOUS CALL, and it must stay so**: slipping an `await`
        // before this line would break the replay SILENTLY.
        // The third argument is batch 33: the viewport goes back to the
        // shell page at EACH resize, from the very measurement that
        // produces the `Resize`. Without it, the supervisor keeps forever the
        // size of the opening day and puts the window back onto it every
        // second — see the header of `AnnonceViewport`.
        //
        // `window.opener` is null when the page is opened by hand: we then
        // pass NO announcer, exactly as the initial announcement
        // above does not go out in that case.
        attacherResizeAuDOM(video, session, annonceurDeViewport(sessionId));
    })
    .catch((error: unknown) => {
        statut.show(`échec : ${error instanceof Error ? error.message : String(error)}`, {
            terminal: true,
            ton: 'danger',
        });
    });
