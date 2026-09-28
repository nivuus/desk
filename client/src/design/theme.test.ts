import { describe, expect, it } from 'vitest';
import {
    CLE_THEME,
    appliquer,
    choisir,
    onStorageChanged,
    themeStocke,
} from './theme';
import amorce from './amorce-theme.js?raw';

/** Stand-in for `localStorage` — see the header of `theme.ts`. */
function coffreFactice(initial: Record<string, string> = {}) {
    const contenu = new Map(Object.entries(initial));
    return {
        contenu,
        ecritures: 0,
        getItem(cle: string) {
            return contenu.get(cle) ?? null;
        },
        setItem(this: { ecritures: number }, cle: string, value: string) {
            contenu.set(cle, value);
            this.ecritures += 1;
        },
    };
}

/** Doublure de `document.documentElement`. */
function racineFactice() {
    return {
        attributs: new Map<string, string>(),
        setAttribute(this: { attributs: Map<string, string> }, nom: string, value: string) {
            this.attributs.set(nom, value);
        },
        removeAttribute(this: { attributs: Map<string, string> }, nom: string) {
            this.attributs.delete(nom);
        },
    };
}

describe('choisir — the WRITING window', () => {
    // 🔴 The next two tests are TWO separate `it()`, never two `expect`
    // of the same test. `expect` stops the test at the first assertion: a
    // single `it()` that checked the write THEN the attribute would stop at
    // the write, and forgetting the local application — the natural defect of this
    // mechanism, since `storage` does not fire for the writer — would
    // hide behind it. That is the lesson sub-block P2 paid for.

    it('writes the theme to the store, under the prefixed key', () => {
        const coffre = coffreFactice();
        choisir(coffre, racineFactice(), 'clair');
        expect(coffre.getItem(CLE_THEME)).toBe('clair');
    });

    it("applies the theme LOCALLY, because `storage` does not come back to the writer", () => {
        const racine = racineFactice();
        choisir(coffreFactice(), racine, 'clair');
        expect(racine.attributs.get('data-theme')).toBe('clair');
    });
});

describe('surStockageModifie — a NEIGHBOURING window', () => {
    it("sets `data-theme` from the event of another window", () => {
        const racine = racineFactice();
        onStorageChanged(racine, CLE_THEME, 'sombre');
        expect(racine.attributs.get('data-theme')).toBe('sombre');
    });

    it("removes the attribute when the neighbour went back to « systeme »", () => {
        // ⚠️ THIS TEST REPLACES the one the plan prescribed — "`onStorageChanged`
        // writes nothing to the store". The latter is UNSATISFIABLE AS A
        // TEST: the signature receives NO `Coffre`, so the function has
        // nothing to write and the assertion cannot fall. The property is
        // guaranteed by the TYPE, which is stronger than a test — it is
        // declared in the header of `theme.ts` rather than staged here.
        // The case below, on the other hand, is real and can fail: a neighbour
        // set back to "systeme" clears the key, and the attribute must disappear.
        const racine = racineFactice();
        racine.attributs.set('data-theme', 'clair');
        onStorageChanged(racine, CLE_THEME, null);
        expect(racine.attributs.has('data-theme')).toBe(false);
    });

    it("ignores an event carrying ANOTHER key — an access token", () => {
        // The key used here is not invented: `client/src/connexion.ts:57`
        // REALLY writes `guac.jeton.acces` at sign-in time, so
        // any neighbouring window receives this event. A handler that did not
        // filter the key would set `data-theme` from a JWT.
        const racine = racineFactice();
        racine.attributs.set('data-theme', 'clair');
        onStorageChanged(racine, 'guac.jeton.acces', 'eyJhbGciOiJIUzI1NiJ9.charge.signature');
        expect(racine.attributs.get('data-theme')).toBe('clair');
    });
});

describe('themeStocke — robustness', () => {
    it("falls back to « systeme » on an unknown value", () => {
        expect(themeStocke(coffreFactice({ [CLE_THEME]: 'bleu' }))).toBe('systeme');
    });

    it("falls back to « systeme » when the key is absent", () => {
        expect(themeStocke(coffreFactice())).toBe('systeme');
    });
});

describe('appliquer', () => {
    it("REMOVES `data-theme` for « systeme », instead of setting the attribute", () => {
        // Its ABSENCE means "systeme". A `data-theme="systeme"` would pass
        // the `:root:not([data-theme="sombre"])` selector of the media query,
        // but not `:root[data-theme="clair"]` — and nothing would break
        // VISIBLY. An invented attribute that breaks nothing is exactly the
        // kind of gap that survives ten sub-blocks.
        const racine = racineFactice();
        racine.attributs.set('data-theme', 'sombre');
        appliquer(racine, 'systeme');
        expect(racine.attributs.has('data-theme')).toBe(false);
    });
});

/**
 * THE GUARD BETWEEN `theme.ts` AND `amorce-theme.js` — and why it exists.
 *
 * `client/src/design/amorce-theme.js` runs BEFORE any module, inline
 * in the `<head>`: it can therefore import NOTHING, and it repeats the string
 * `guac.theme` as a literal. It is the only overlap between the two
 * files, and it is deliberate — but a deliberate overlap guarded by
 * nothing becomes a silent divergence: the bootstrap would read a key that nobody
 * writes any more, and the only symptom would be a flash of the wrong theme that
 * nobody looks at in review.
 *
 * ⚠️ This test is the exact pattern P2 used between `client/src/jeton.ts`
 * and `client/recette/jeton-recette.mjs`, and P2's guard WAS SEEN THROWING.
 * This one was too: see the S1 results document, task 10.
 *
 * ⚠️ It compares the STRING, never the behaviour. That the bootstrap does set
 * `data-theme` before the first paint is proven by no test — it is
 * the build that proves the INJECTION (task 10, Step 3) and nothing proves
 * the absence of a flash.
 */
describe('the anti-FOUC bootstrap and `theme.ts` cannot diverge on the key', () => {
    it('`amorce-theme.js` READS the value of `CLE_THEME` literally', () => {
        // 🔴 The assertion is about the CALL, not about the presence of the string
        // somewhere in the file. A first draft said
        // `toContain("'guac.theme'")`: it was satisfied by the
        // header comment, so that a bootstrap replaced by
        // `var t = null;` stayed GREEN on both assertions. Seen, measured,
        // and that is why the key is no longer written in that comment.
        expect(amorce).toContain(`getItem('${CLE_THEME}')`);
    });

    it('`amorce-theme.js` carries NO other `guac.*` key', () => {
        // Without this second assertion, adding a key to the bootstrap without removing
        // the old one would pass: `toContain` says nothing about what surrounds it.
        const cles = [...amorce.matchAll(/'(guac\.[a-z.]+)'/g)].map((m) => m[1]);
        expect([...new Set(cles)]).toEqual([CLE_THEME]);
    });
});
