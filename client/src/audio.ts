// Unlocking sound at the first user gesture.
//
// Chrome blocks audio playback without user activation, and the activation
// obtained on the home page DOES NOT CROSS the opening of a new
// window — measured by the multi-window spike of 07/28/2026. The session
// therefore starts muted and unmutes at the first gesture, whatever it is.
//
// It is the third use of the same arming spring, after window
// opening (spike variant 3) and the fullscreen toggle (game framing §4.1).
// No click is imposed: the one used to play is enough.
//
// Dependencies are INJECTED rather than read from global objects, which
// makes the module testable without DOM.

/// What this module needs from a media element: nothing but `muted`.
export interface CibleMedia {
    muted: boolean;
}

/// What it needs from a listening target.
export interface CibleGeste {
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

export interface OptionsSon {
    media: CibleMedia;
    cible: CibleGeste;
    /// Called with `false` at arming, then `true` at unmuting. Enough to
    /// display — and remove — a "click to enable sound" banner.
    surEtat?: (actif: boolean) => void;
}

/// Gestures that count as user activation for Chrome.
const GESTES = ['pointerdown', 'keydown'] as const;

/// Keys that do NOT count as user activation in the HTML sense, although
/// they trigger a `keydown` — `input.ts` forwards both
/// to the remote server, so a player pressing Shift or Esc before
/// any other key is a real case, not a theoretical one. A modifier alone
/// (`Shift`, `Control`, `Alt`, `Meta`) or `Escape` must not consume
/// the one-shot arming: without this filter, that gesture would exhaust it without
/// obtaining activation, and no later gesture would ever retry
/// unmuting.
const TOUCHES_SANS_ACTIVATION = new Set(['Shift', 'Control', 'Alt', 'Meta', 'Escape']);

/// Does the gesture count as user activation? True for any non-keyboard
/// gesture (`pointerdown`); for a `keydown`, false if the key is a
/// modifier alone or `Escape`. Works only on the received event,
/// without `instanceof KeyboardEvent` or access to `document`/`window`: these
/// globals are not guaranteed by the module's dependency injection
/// (see the file header), nor are they under Vitest.
function vautActivation(event: Event): boolean {
    if (event.type !== 'keydown') return true;
    const touche = (event as KeyboardEvent).key;
    return !TOUCHES_SANS_ACTIVATION.has(touche);
}

/// Arms unmuting. Returns a cancel function that removes the
/// listeners without unmuting.
export function armerLeSon(options: OptionsSon): () => void {
    const { media, cible, surEtat } = options;
    let fait = false;

    const retirer = () => {
        for (const geste of GESTES) {
            cible.removeEventListener(geste, activer);
        }
    };

    // Named so it can be removed. The listeners are removed at the
    // first gesture that actually unmutes: otherwise, every later gesture
    // would force `muted = false` again and would overwrite the choice of a
    // user who had muted the sound themselves.
    function activer(event: Event): void {
        if (fait) return;
        if (!vautActivation(event)) return;

        media.muted = false;
        if (media.muted) {
            // The browser refused unmuting (this gesture did not count
            // as activation in its eyes after all): the arming
            // stays available, the listeners stay in place so that the
            // next gesture retries.
            return;
        }

        fait = true;
        retirer();
        surEtat?.(true);
    }

    for (const geste of GESTES) {
        cible.addEventListener(geste, activer);
    }
    surEtat?.(false);

    return () => {
        if (fait) return;
        fait = true;
        retirer();
    };
}
