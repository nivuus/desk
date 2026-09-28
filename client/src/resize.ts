/**
 * Retains the last observed size and says whether it must be emitted.
 *
 * **Why this object exists**: `main.ts`'s `ResizeObserver` gave up
 * silently when the control channel was not open at the moment its
 * debounce expired, and NEVER re-emitted — the observer only
 * fires again if the element changes size again. A lost size
 * was therefore lost forever (leg 10 of sub-block D8: two `Resize`s recorded for
 * five sessions).
 *
 * **Pure, DOM-free**: that is what makes it testable.
 */
export interface Size {
    largeur: number;
    hauteur: number;
}

export class RejeuResize {
    private derniere: Size | undefined;
    private emise: Size | undefined;

    /** The `ResizeObserver` saw a size. */
    observer(size: Size): void {
        this.derniere = size;
    }

    /** The size to emit, or `undefined` if there is nothing new. */
    aEmettre(): Size | undefined {
        const derniere = this.derniere;
        if (!derniere) return undefined;
        if (
            this.emise &&
            this.emise.largeur === derniere.largeur &&
            this.emise.hauteur === derniere.hauteur
        ) {
            return undefined;
        }
        return derniere;
    }

    /** The emission really took place. */
    confirmer(size: Size): void {
        this.emise = size;
    }
}
