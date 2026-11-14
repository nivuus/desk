// The page's loss of focus and visibility, as `input.ts` reads it.
//
// `blur` is fired on `window`, `visibilitychange` on `document`: this target
// unites the two under `CiblePage`, so that `input.ts` stays testable without
// a DOM. Wiring only, like `viewport-dom.ts`.

import type { CiblePage } from './input';

export function ciblePageDuDOM(): CiblePage {
    const source = (nom: 'blur' | 'visibilitychange'): EventTarget =>
        nom === 'blur' ? window : document;
    return {
        get hidden() {
            return document.hidden;
        },
        addEventListener: (nom, rappel) => source(nom).addEventListener(nom, rappel),
        removeEventListener: (nom, rappel) => source(nom).removeEventListener(nom, rappel),
    };
}
