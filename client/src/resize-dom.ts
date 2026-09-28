// Plugging resizing into the DOM: `ResizeObserver`, 200 ms
// smoothing, emission on the control channel, and the instrumentation of D9's
// legacy no. 7.
//
// **Extracted from `main.ts` VERBATIM on August 20th, 2026**, task 15 of the
// clipboard's sub-block P1. The plan set a MEASUREMENT GATE on `main.ts` — "if
// the after exceeds 480 lines, extract" — computing it on a file at
// 451 lines. It had 460 at the time of the addition, and the clipboard
// wiring, although already reduced to a minimum by the extraction of
// `presse-papier-dom.ts` the plan named, left `main.ts` at 487: the
// gate was crossed. This file is the second extraction, and it follows
// the repository's doctrine — **extract, never compress**.
//
// The split is the one already used by `presse-papier.ts` /
// `presse-papier-dom.ts`: `resize.ts` carries the PURE rule (`RejeuResize`,
// DOM-free, tested), this file carries its only branch onto the browser.
//
// 🔴 **THE SYNCHRONICITY INVARIANT CROSSES THIS EXTRACTION, AND IT HAD TO BE
// CHECKED** (D9's legacy no. 12, full text below): the `.then()` of
// `main.ts` must run without an interposed `await` up to the final
// `addEventListener`. The function below is called SYNCHRONOUSLY from that `.then()`
// and awaits nothing itself, so the invariant is preserved — moving it into
// an `async` function or calling it behind an `await` would break it
// exactly like an interposed `await`.

import { RejeuResize } from './resize';
import { encodeResize } from '../../proto/ts/control';

/// What this module needs from the session: the control channel, and nothing
/// else. The object returned by `connectSession` conforms to it.
export interface SessionResize {
    controlChannel: RTCDataChannel;
}

/// Plugs in size tracking. **To call SYNCHRONOUSLY** — see the header.
/// What this module does with the viewport announcement, when it is given one.
///
/// 🔴 **WHY THE VIEWPORT GOES BACK FROM HERE, AND NOT FROM `main.ts`.** `main.ts`
/// announces the viewport **only once, at load**, to the shell page that
/// opened it (`window.opener.postMessage`): it is that announcement that decides
/// the size of the virtual output. Nothing re-announced it afterwards — so
/// that the SUPERVISOR, which only knows the desired size through that message,
/// kept forever the one of the opening day, and put the window back
/// onto it every second.
///
/// 🔴 **AND IT MUST START FROM THE SAME MEASUREMENT AS THE `Resize`, THAT IS THE WHOLE
/// POINT.** The control channel's `Resize` goes to the SENSOR (cropping and
/// encoder); the `postMessage`'s `viewport` goes to the SUPERVISOR (Windows
/// window and retained size). Both processes apply the same pure
/// rule (`windows_source_sortie::size_for_viewport`) on the same bound: if
/// they were given two different NUMBERS — `video.clientWidth` here and
/// `window.innerWidth` there —, they would compute two sizes and fight
/// at 1 Hz. One measurement, two recipients.
export interface AnnonceViewport {
    (largeur: number, hauteur: number): void;
}

export function attacherResizeAuDOM(
    video: HTMLVideoElement,
    session: SessionResize,
    annoncerViewport?: AnnonceViewport,
): void {
    // Resizing rebuilds the encoding chain on the agent side:
    // we therefore only emit once the gesture is over, not at each pixel
    // covered while the user drags an edge.
    const rejeu = new RejeuResize();
    const emettreSiPossible = () => {
        const size = rejeu.aEmettre();
        if (!size) return;
        if (session.controlChannel.readyState !== 'open') {
            // Traced, and no longer mute: it is this silent `return` that lost
            // the `Resize`s without leaving the slightest trace (leg 10).
            console.warn('Resize différé : canal de contrôle non ouvert');
            return;
        }
        // Instrumentation of D9's legacy no. 7 (task 18, D10) — confirmation at the
        // emission point. Full reading grid on the
        // "ResizeObserver trigger" log below; this one only
        // confirms, for THIS `Resize` attempt, which of the three
        // outcomes happened.
        console.debug('[instrumentation resize] emission', {
            taille: size,
            clientWidth: video.clientWidth,
            clientHeight: video.clientHeight,
            innerWidth: window.innerWidth,
            innerHeight: window.innerHeight,
        });
        session.controlChannel.send(encodeResize(size.largeur, size.hauteur));
        // SAME SIZE, SAME INSTANT, TWO RECIPIENTS — see the header
        // of `AnnonceViewport`. Emitted AFTER the `Resize` and not before: the
        // sensor is the short path (data channel then named pipe), the
        // supervisor the long path (the platform's relay), and there is
        // no reason to delay the first for the second. **The order
        // of arrival does not matter anyway**: both
        // converge on the same value, and the second's gesture is then without
        // effect.
        annoncerViewport?.(size.largeur, size.hauteur);
        rejeu.confirmer(size);
    };

    let resizeTimer: number | undefined;
    const observer = new ResizeObserver(() => {
        // Instrumentation of D9's legacy no. 7 (task 18, D10): sub-block D8
        // had designated this link — between `window.innerWidth` (what the page
        // announces at opening) and the actual emission of the `Resize`, "leg 10"
        // in D8's numbering — without ever having measured it. Three
        // outcomes can be read from this log and the emission one above,
        // in THIS reading ORDER — none concludes beyond what
        // it establishes, and the control channel stays a hypothesis in
        // its own right (see the `console.warn` above):
        //   1. NO "trigger" log for a session that never emits
        //      a `Resize` (case D9: w-2, w-5) ⟹ the observer never
        //      arms or is never called back — the link is
        //      UPSTREAM of layout, in the wiring of
        //      `observer.observe(video)` or the session's construction.
        //   2. Log present, `clientWidth`/`clientHeight` FOLLOWS
        //      `innerWidth`/`innerHeight` ⟹ neither the observer nor
        //      layout are at fault; the link is elsewhere.
        //   3. Log present, `clientWidth`/`clientHeight` DOES NOT FOLLOW
        //      `innerWidth`/`innerHeight` ⟹ the CSS layout of
        //      the `<video>` element is at fault.
        // This log, taken before the 200 ms smoothing, settles case 1;
        // the emission log above confirms 2 or 3 for the attempt
        // that actually succeeds.
        console.debug('[instrumentation resize] declenchement ResizeObserver', {
            clientWidth: video.clientWidth,
            clientHeight: video.clientHeight,
            innerWidth: window.innerWidth,
            innerHeight: window.innerHeight,
        });
        window.clearTimeout(resizeTimer);
        resizeTimer = window.setTimeout(() => {
            rejeu.observer({
                largeur: Math.round(video.clientWidth * window.devicePixelRatio),
                hauteur: Math.round(video.clientHeight * window.devicePixelRatio),
            });
            emettreSiPossible();
        }, 200);
    });
    observer.observe(video);
    // The replay: when the channel opens, the retained size goes out again.
    //
    // ⚠️ NON-OBVIOUS INVARIANT (D9's legacy no. 12): this `.then()` must
    // run ENTIRELY SYNCHRONOUSLY, without an interposed `await`
    // between the construction of `rejeu` / of the `ResizeObserver` above and
    // this `addEventListener`. An `await` slipped there would yield to the
    // event loop: if the channel opened during the wait,
    // the listener would be set AFTER the `open` event, it would never be
    // called, and the replay would be broken SILENTLY — no error, no
    // log, just a lost size. It is exactly the failure
    // mode the replay exists to repair.
    //
    // No test guards this invariant, and it is a decision, not an
    // oversight: seeing it red would require simulating `RTCDataChannel` and the whole
    // cycle of `createSession`, that is, mocking the entire
    // session. A test that cannot be seen red at a reasonable cost
    // would add nothing to what this comment already says.
    session.controlChannel.addEventListener('open', emettreSiPossible);
}
