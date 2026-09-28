// Status banner: single write point, which protects a TERMINAL message
// against being overwritten by an ORDINARY message arriving after it.
//
// Five writers access the banner: four in `main.ts` (ready, session
// ended, sound arming, connection failure) and `webrtc.ts`, which writes
// through the `onStatus` callback (stream received, connection state change,
// offer sent, answer received). `connectionstatechange` fires BY
// CONSTRUCTION right after `session-end` — `RTCPeerConnection` closes in
// reaction to the end of the session — so without a guard, "connection: disconnected"
// systematically overwrites "session ended: …", the more
// informative message of the two. Routing all writers through this module rather than
// keeping the old ad hoc `sessionTerminee` flag in `main.ts` closes that
// hole at the root: no caller can forget the guard any more, because
// it no longer has direct access to `element.textContent`.
//
// Dependencies are injected, as in `audio.ts`, to stay
// testable without a DOM.
//
// ── THE TERMINAL SCREEN (sub-block S4, task 9) ────────────────────────────
// A SECOND, OPTIONAL target receives the TERMINAL messages — and them
// alone. It is plugged in HERE rather than in `main.ts` for the very reason
// that gave birth to this module: it is the single write point, and a
// caller writing `ecran.montrer(...)` next to `statut.show(...)`
// would be two writes nothing forces to stay in agreement. Without a target, the
// behaviour is the pre-S4 one, to the line: the eleven tests of
// `status.test.ts` pass UNCHANGED, and if they had to change, the
// terminal guard would have been weakened.

import type { EcranTerminal, TonTerminal } from './ecran-terminal';

/// What this module needs from a display element.
export interface CibleStatut {
    textContent: string;
    dataset: { hidden?: string };
}

export interface OptionsAffichage {
    /// A TERMINAL message (end of session, failure) is never again overwritten
    /// by an ordinary message. A second terminal message does replace the
    /// first: the last definitive information wins.
    terminal?: boolean;
    /// A PERSISTENT message resists `masquer()`, but lets itself be replaced
    /// by a following message. It serves conditions that last — a degraded
    /// network, for instance: their display must not be erased by the
    /// timer of a neighbouring banner that arrived before it, a timer
    /// the caller has no way of knowing about.
    persistant?: boolean;
    /// The TONE of a terminal message, read by the screen alone: a normal end
    /// is not an error. Without effect on the banner, and without any effect
    /// outside a terminal message. Default: `neutre`.
    ton?: TonTerminal;
}

export interface Statut {
    show(message: string, options?: OptionsAffichage): void;
    /// Hides the banner — unless a terminal or persistent message is
    /// displayed: it must stay visible until another message
    /// replaces it.
    masquer(): void;
    /// Lifts the persistence of the current message then hides, as if that
    /// message had never carried `persistant: true` — without touching the
    /// `terminal` protection, which stays untouchable: it is what prevents
    /// an end of session from being overwritten by a routine banner, and nothing
    /// must weaken it.
    ///
    /// `masquer()` deliberately protects a persistent message: it is what
    /// lets it survive the timer of a neighbouring banner that does not know
    /// it exists (see `OptionsAffichage.persistant`). But when
    /// the caller that displayed this message KNOWS the condition that
    /// justified it has ceased (e.g. a sleeping window has just woken up),
    /// it needs an explicit way to say so — without redisplaying an
    /// empty message as a workaround, which would make the banner flicker
    /// and copy the problem to the next persistent message.
    expirer(): void;
}

export function createStatus(element: CibleStatut, ecran?: EcranTerminal): Statut {
    let terminal = false;
    let persistant = false;

    const masquer = () => {
        if (terminal || persistant) return;
        element.dataset.hidden = 'true';
    };

    return {
        show(message, options) {
            const estTerminal = options?.terminal ?? false;
            if (terminal && !estTerminal) return;
            terminal = terminal || estTerminal;
            // Reassigned at each call, unlike `terminal`: an
            // ordinary message following an alert lifts the persistence —
            // a return to a normal state must be able to hide again.
            persistant = options?.persistant ?? false;
            element.textContent = message;
            element.dataset.hidden = 'false';
            // AFTER the guard above, hence never for an ordinary
            // or persistent message, and again for a SECOND terminal one — which
            // replaces the first, on the screen as on the banner.
            if (estTerminal) ecran?.montrer(message, options?.ton ?? 'neutre');
        },
        masquer,
        expirer() {
            persistant = false;
            masquer();
        },
    };
}
