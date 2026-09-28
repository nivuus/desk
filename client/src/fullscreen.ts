// Fullscreen and Keyboard Lock.
//
// §4.1 of the games framing makes Windows the master of fullscreen, in a
// SINGLE direction: Windows decides, the browser follows, never
// the reverse. This module carries both entries into fullscreen — the
// toggle button (`attachFullscreen`, a local user gesture) and
// the arming triggered by the `fullscreen` message the agent relays to
// follow the state of the Windows window (`armerPleinEcran` /
// `armerPleinEcranAuDOM`, wired in `main.ts` on `AgentControl.fullscreen`)
// — but neither of them sends anything back up to Windows: the
// browser never forces the state of the remote window. That is what makes
// any oscillation impossible.
//
// Keyboard Lock is not a comfort: Escape is both the key that
// exits browser fullscreen and the pause menu key of almost all
// games. Without it, every pause leaves fullscreen.
//
// Like pointer.ts, dependencies are INJECTED rather than read from the
// global objects (`document`, `navigator`), which makes the module testable
// without a DOM.

/** What this module needs from `navigator`: only the Keyboard Lock API. */
export interface NavigateurClavier {
    keyboard?: {
        lock(codes?: string[]): Promise<void>;
        unlock(): void;
    };
}

/**
 * Locks all keys to the page. Without an argument: that is game
 * mode. The user exits by long-pressing Escape, a behaviour provided by
 * the API.
 *
 * Never throws: on Firefox and Safari `navigator.keyboard` is absent, and
 * a rejection must not propagate into the event handler.
 */
export async function verrouillerClavier(navigateur: NavigateurClavier): Promise<void> {
    if (!navigateur.keyboard) return;
    try {
        await navigateur.keyboard.lock();
    } catch (error) {
        console.warn('Keyboard Lock refused', error);
    }
}

/** What this module needs from the element whose fullscreen is requested. */
export interface CibleEcran {
    requestFullscreen(): Promise<void>;
}

/** What this module needs from the toggle button. */
export interface BoutonPleinEcran {
    dataset: { actif?: string };
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

/** What this module needs from the document: the current fullscreen state. */
export interface DocumentPleinEcran {
    readonly fullscreenElement: CibleEcran | null;
    exitFullscreen(): Promise<void>;
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

export interface FullscreenOptions {
    bouton: BoutonPleinEcran;
    cible: CibleEcran;
    doc: DocumentPleinEcran;
    navigateur?: NavigateurClavier;
}

/**
 * Wires the toggle button onto `cible`. The click requests or leaves
 * fullscreen; `fullscreenchange` is the only source of truth for the displayed state
 * and for keyboard (un)locking — including when the exit comes
 * from somewhere other than the button (Escape after a long press, F11, etc.).
 *
 * Returns a detach function that removes the two listeners set.
 */
export function attachFullscreen({ bouton, cible, doc, navigateur = {} }: FullscreenOptions): () => void {
    const onClick = (): void => {
        if (doc.fullscreenElement) {
            void doc.exitFullscreen();
        } else {
            void cible.requestFullscreen();
        }
    };

    const onChange = (): void => {
        const actif = doc.fullscreenElement === cible;
        bouton.dataset.actif = String(actif);
        if (actif) {
            void verrouillerClavier(navigateur);
        } else {
            navigateur.keyboard?.unlock();
        }
    };

    bouton.addEventListener('click', onClick);
    doc.addEventListener('fullscreenchange', onChange);

    return () => {
        bouton.removeEventListener('click', onClick);
        doc.removeEventListener('fullscreenchange', onChange);
    };
}

// Default values for use in the real browser (see task 15: wiring).
export function attachFullscreenAuDOM(
    options: Omit<FullscreenOptions, 'doc' | 'navigateur'> & { navigateur?: NavigateurClavier },
): () => void {
    return attachFullscreen({
        doc: document as unknown as DocumentPleinEcran,
        navigateur: navigator as unknown as NavigateurClavier,
        ...options,
    });
}

/** What arming needs from an event target (the document). */
export interface CibleEvenement {
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

export interface ArmementOptions {
    cible: CibleEcran;
    doc: DocumentPleinEcran;
    ecouteurs: CibleEvenement;
}

/** The gestures carrying a transient user activation. */
const GESTES = ['pointerdown', 'keydown'] as const;

/**
 * Arms the fullscreen entry on the next user gesture.
 *
 * **We arm, we do not act.** `requestFullscreen()` requires a transient user
 * activation; a message received on a data channel is not
 * one, and the call would be rejected. It is the mechanism retained in §4.1 of the games
 * framing, and the same one the multi-window spike validated for `window.open()`:
 * a single mechanism for both needs.
 *
 * The keyboard counts as much as the pointer: a player with a gamepad or
 * keyboard has no reason to click.
 *
 * **Keyboard Lock is not to be requested here**: `attachFullscreen` already locks
 * on `fullscreenchange`, whatever the origin of the entry.
 *
 * Returns a detach function, to call at the end of the session.
 */
export function armerPleinEcran({ cible, doc, ecouteurs }: ArmementOptions): () => void {
    if (doc.fullscreenElement) return () => {};

    const detacher = (): void => {
        for (const geste of GESTES) ecouteurs.removeEventListener(geste, surGeste);
    };
    const surGeste = (): void => {
        detacher();
        void cible.requestFullscreen();
    };
    for (const geste of GESTES) ecouteurs.addEventListener(geste, surGeste);
    return detacher;
}

/** Default values for use in the real browser. */
export function armerPleinEcranAuDOM(cible: CibleEcran): () => void {
    return armerPleinEcran({
        cible,
        doc: document as unknown as DocumentPleinEcran,
        ecouteurs: document as unknown as CibleEvenement,
    });
}
