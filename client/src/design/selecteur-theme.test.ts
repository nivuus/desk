import { describe, expect, it } from 'vitest';
import { installThemeSelector, type BoutonDeTheme } from './selecteur-theme';
import { CLE_THEME } from './theme';

/**
 * Doubles WRITTEN BY HAND, on the pattern of `faireBouton()` of
 * `client/src/fullscreen.test.ts`: `client/` has NEITHER jsdom NOR happy-dom, and the
 * module therefore reads no global in the tested path.
 *
 * ⚠️ NONE OF THESE DOUBLES IS A NO-OP. Sub-block D10 paid for "a fake
 * source that implements a side effect as a NO-OP makes a whole family of
 * defects invisible to host tests": here the store KEEPS what is
 * written to it, the root KEEPS its attribute, and each button KEEPS its
 * successive `aria-pressed`.
 */
function banc(themeInitial: string | null = null) {
    const ecrits: Array<[string, string]> = [];
    const stockage = new Map<string, string>();
    if (themeInitial !== null) stockage.set(CLE_THEME, themeInitial);

    const coffre = {
        getItem: (cle: string) => stockage.get(cle) ?? null,
        setItem: (cle: string, value: string) => {
            ecrits.push([cle, value]);
            stockage.set(cle, value);
        },
    };

    let attribut: string | null = null;
    const racine = {
        setAttribute: (_nom: string, value: string) => { attribut = value; },
        removeAttribute: () => { attribut = null; },
    };

    const boutons: Array<BoutonDeTheme & { cliquer(): void }> = [];
    const createButton = (): BoutonDeTheme => {
        let clic: (() => void) | null = null;
        const bouton = {
            dataset: {} as { theme?: string },
            textContent: null as string | null,
            attributs: new Map<string, string>(),
            setAttribute(nom: string, value: string) { this.attributs.set(nom, value); },
            addEventListener(_type: 'click', ecouteur: () => void) { clic = ecouteur; },
            cliquer() { clic?.(); },
        };
        boutons.push(bouton);
        return bouton;
    };

    let surStorage: ((e: { key: string | null; newValue: string | null }) => void) | null = null;
    const source = {
        addEventListener(
            _type: 'storage',
            ecouteur: (e: { key: string | null; newValue: string | null }) => void,
        ) { surStorage = ecouteur; },
    };

    let rappels = 0;
    installThemeSelector({
        hote: { append: () => {} },
        racine,
        coffre,
        source,
        createButton,
        apres: () => { rappels += 1; },
    });

    const parTheme = (nom: string) => boutons.find((b) => b.dataset.theme === nom)!;
    const presse = (nom: string) =>
        (parTheme(nom) as unknown as { attributs: Map<string, string> }).attributs.get('aria-pressed');

    return {
        ecrits,
        boutons,
        parTheme,
        presse,
        attributRacine: () => attribut,
        rappels: () => rappels,
        // 🔴 THE STORE IS UPDATED BEFORE THE BROADCAST, BECAUSE THAT IS
        // WHAT THE BROWSER DOES: the HTML specification guarantees that
        // the `storage` event is fired AFTER the storage area has been
        // modified. A double that broadcast without writing would make the
        // marking test fail FOR A REASON UNRELATED to the defect it exercises —
        // and the fix would look like it does not work. It was the first
        // draft of this bench, and it was corrected here and not in the code.
        emettreStorage: (key: string | null, newValue: string | null) => {
            if (key !== null) {
                if (newValue === null) stockage.delete(key);
                else stockage.set(key, newValue);
            }
            surStorage?.({ key, newValue });
        },
    };
}

describe('installerSelecteurDeTheme', () => {
    it('sets exactly three buttons, one per state', () => {
        const b = banc();
        expect(b.boutons.map((x) => x.dataset.theme)).toEqual(['systeme', 'clair', 'sombre']);
    });

    it('a click on « light » WRITES THE KEY', () => {
        // First half, separated from the second as check §7.5 separates
        // its own: writing and applying are two effects, and `expect`
        // stops at the first failure.
        const b = banc();
        (b.parTheme('clair') as unknown as { cliquer(): void }).cliquer();
        expect(b.ecrits).toEqual([[CLE_THEME, 'clair']]);
    });

    it('a click on « light » SETS `data-theme` LOCALLY', () => {
        const b = banc();
        (b.parTheme('clair') as unknown as { cliquer(): void }).cliquer();
        expect(b.attributRacine()).toBe('clair');
    });

    it('`aria-pressed` follows the click', () => {
        const b = banc();
        expect(b.presse('systeme')).toBe('true');
        (b.parTheme('sombre') as unknown as { cliquer(): void }).cliquer();
        expect(b.presse('sombre')).toBe('true');
        expect(b.presse('systeme')).toBe('false');
    });

    it('`aria-pressed` follows a `storage` COMING FROM ANOTHER WINDOW', () => {
        // 🔴 THE RED OF THE FIX, and it falls on the code from before S3.
        // This module declared the defect itself: "IT DOES NOT CALL
        // `marquer()` AGAIN […] an `aria-pressed` set here stays that of the previous
        // theme as long as the user does not click in THIS window. The
        // defect is real and it is PRE-EXISTING".
        //
        // On an INSTRUMENT, it was a nuisance. On the PRODUCT, it is an
        // interface that LIES about its own state — and the case of a neighbouring
        // window changing the theme is the NOMINAL case of multi-window,
        // which is the raison d'être of the `storage` mechanism (spec §4.2).
        const b = banc();
        expect(b.presse('systeme')).toBe('true');
        b.emettreStorage(CLE_THEME, 'sombre');
        expect(b.presse('sombre')).toBe('true');
        expect(b.presse('systeme')).toBe('false');
    });

    it('applies the theme received through `storage` to the root', () => {
        const b = banc();
        b.emettreStorage(CLE_THEME, 'clair');
        expect(b.attributRacine()).toBe('clair');
    });

    it('a `storage` on the NEIGHBOURING key `guac.jeton.acces` changes NOTHING', () => {
        // ⚠️ THE REAL NEIGHBOURING KEY, never an invented key: `jeton.ts`
        // declares it and `connexion.ts` really writes it on each sign-in, so
        // any neighbouring window receives that very event. Without the filter,
        // `data-theme` would hold a JWT.
        const b = banc('clair');
        b.emettreStorage('guac.jeton.acces', 'eyJhbGciOi.faux.jeton');
        expect(b.attributRacine()).toBe('clair');
        expect(b.presse('clair')).toBe('true');
        expect(b.rappels()).toBe(0);
    });

    it("writes NOTHING to the store when reacting to a `storage`", () => {
        // Otherwise two windows would bounce the event back and forth without end.
        // ⚠️ THIS TEST IS NOT GUARANTEED BY THE SIGNATURE, unlike that
        // of `onStorageChanged`: here the closure HOLDS the store and
        // READS it on each marking. It can therefore fall, which is why it is
        // written.
        const b = banc();
        b.emettreStorage(CLE_THEME, 'sombre');
        expect(b.ecrits).toEqual([]);
    });
});
