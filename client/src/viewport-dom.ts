// Announcing the viewport to the tab that opened us: the measurement, and the
// two moments when it goes out.
//
// ⚠️ **THIS HEADER SAID "to the shell page" UNTIL THE FINAL REVIEW OF
// AUGUST 31ST, 2026**, and the word today designates a page that is now only
// a redirect. The opener is the HUB (`bureau/porteur-dom.ts`).
//
// 🔴 **AND THE RECIPIENT IS NOT ALWAYS THE ONE HOLDING THE SESSION —
// A DECLARED LIMIT, NOT FIXED (Important ⑤ of this same review).** The
// hub elects a CARRIER tab; a FOLLOWER tab can nonetheless reopen
// a window from its own click (`window.open` requires the activation of
// the tab that has the gesture, spec §4). The announcement below then goes to
// THAT follower: its `bureau.viewportRecu` returns immediately — its
// `connues` table is empty, no `fenetreOuverte` having ever fed it —
// and its `envoyer` is a no-op, for lack of a socket. **Consequence, that of
// batch 33: the cropping and the size stay those of the previous
// session, and nothing traces it.**
//
// **Extracted from `main.ts` VERBATIM (batch 33).** The file was at **EXACTLY 500
// lines** — its gate — and the batch's addition (the re-announcer, without which
// the supervisor keeps forever the size of the opening day)
// brought it to 535. `CLAUDE.md` forbids compressing: we extract.
//
// ⚠️ **This extraction FOLLOWS its addition instead of preceding it**, and it is
// said rather than disguised: the need for the re-announcer only appeared once
// the path of the `Resize` was measured, halfway through the batch. It is the THIRD extraction of
// `main.ts` (after `resize-dom.ts` and `presse-papier-dom.ts`), and the second
// time this file crosses its ceiling through an addition of twenty or so
// lines: better to know it before touching it.
//
// The split is the one already used by `resize.ts` / `resize-dom.ts`:
// `viewport.ts` carries the PURE rule (`viewportPair`, DOM-free, tested), this
// file carries its only branch onto the browser.

import { viewportPair } from './viewport';
import type { AnnonceViewport } from './resize-dom';

// Announcing the viewport to the shell page that opened us.
//
// It is this size that decides the resolution of the virtual output, hence
// the native resolution of the stream: nothing can be created on the agent side before
// it is known. The announcement therefore goes out BEFORE any WebRTC connection.
//
// `window.opener` is null when the page is opened by hand (trials,
// direct reload): in that case the agent is already running and there is nothing to
// ask — we do nothing rather than fail.
export function annoncerLeViewportInitial(sessionId: string): void {
    if (!window.opener || window.opener.closed) return;
    // SAME UNIT as the `Resize` emitted further down (`clientWidth × devicePixelRatio`).
    //
    // ⚠️ **The reason originally written here was already stale when it was
    // written, and it is the CROSS-CUTTING review at the end of branch D9 that
    // caught it.** It said: "without this factor, EACH connection of EACH
    // window would trigger a mode change, with a 25 to 100 % gap
    // (leg 7 of sub-block D8)". Yet task 3 of the same sub-block D9 — one commit
    // BEFORE the one that wrote this sentence — had removed the custom output
    // mode change (see the finding at the head of
    // `agent/src/capteur/plein_ecran.rs`). There is therefore no mode change
    // left to trigger. ⚠️ **The rest of that sentence — "`WindowsSource::
    // resize` returns first of all in `SortieEntiere` mode, and the *size
    // unchanged* short-circuit is not even reached any more" — has been FALSE since batch
    // 33**: `resize` now makes the cropping and the window follow the
    // viewport, and the "size unchanged" short-circuit is again what
    // avoids rebuilding an encoder every 200 ms while one drags
    // an edge. **What remains true, and is the reason to keep the
    // factor**: no DISPLAY mode is changed, neither here nor there.
    //
    // ✅ **What the factor REALLY fixes, and which justifies keeping it**:
    // the viewport announcement DECIDES the size of the virtual output created by the
    // supervisor (`superviseur/boucle.rs::creer_sortie`). Without dpr, a HiDPI
    // client received an output SMALLER than its real display surface,
    // hence an image upscaled by the browser. With it, the
    // output is born in device pixels, the unit in which the routine `Resize`
    // already speaks.
    //
    // ⚠️ **Unmeasured consequence, and declared as such (D9's legacy)**: at
    // `devicePixelRatio = 2`, a 1280×720 CSS window now requests an
    // output of 2560×1440, that is FOUR times the pixels to capture and encode,
    // and **nothing bounds this request** — `windows_source/sortie.rs::
    // borner_a_la_taille_max` (1920×1080) lost its last caller with the
    // mode change and is no longer plugged in anywhere. D6 recorded the
    // browser's decoder saturated from eight 1280×720 windows.
    //
    // There is only one `devicePixelRatio` at play: it is THIS page that announces, and
    // it is its own `ResizeObserver` that will emit the `Resize`.
    //
    // Multiply THEN round to even — `viewportPair` has a floor of 2, and
    // the reverse order would let an odd height through at an odd dpr.
    const dpr = window.devicePixelRatio;
    const { largeur, hauteur } = viewportPair(
        Math.round(window.innerWidth * dpr),
        Math.round(window.innerHeight * dpr),
    );
    window.opener.postMessage(
        { type: 'viewport', session: sessionId, largeur, hauteur },
        window.location.origin,
    );
}

// The announcer of the FOLLOWING resizes, passed to `attacherResizeAuDOM`.
//
// ⚠️ **It does NOT multiply again by `devicePixelRatio`**: `resize-dom.ts` already measures
// in device pixels (`video.clientWidth * window.devicePixelRatio`),
// and applying it twice would request four times the pixels for a HiDPI
// client. It is the same unit as the initial announcement above, which multiplies
// because it starts from `innerWidth`, itself in CSS pixels.
//
// `undefined` — hence no announcement — when the page has no opener: there
// is then no shell page to talk to, and `postMessage` on `null`
// would throw.
export function annonceurDeViewport(sessionId: string): AnnonceViewport | undefined {
    return window.opener && !window.opener.closed
        ? (largeur: number, hauteur: number) => {
              window.opener.postMessage(
                  { type: 'viewport', session: sessionId, largeur, hauteur },
                  window.location.origin,
              );
          }
        : undefined;
}
