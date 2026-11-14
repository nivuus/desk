// The page's loss of focus and visibility, as `input.ts` reads it.
//
// `blur` is fired on `window`, `visibilitychange` on `document`: this target
// unites the two under `PageTarget`, so that `input.ts` stays testable without
// a DOM. Wiring only, like `viewport-dom.ts`.

import type { PageTarget } from './input';

export function pageTargetFromDom(): PageTarget {
    const source = (name: 'blur' | 'visibilitychange'): EventTarget =>
        name === 'blur' ? window : document;
    return {
        get hidden() {
            return document.hidden;
        },
        addEventListener: (name, callback) => source(name).addEventListener(name, callback),
        removeEventListener: (name, callback) => source(name).removeEventListener(name, callback),
    };
}
