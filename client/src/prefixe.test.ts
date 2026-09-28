import { describe, expect, it } from 'vitest';

import {
    CLE_PREFIXE,
    composer,
    clearPrefix,
    lirePrefixe,
    poserPrefixe,
    prefixeDeLaVm,
    retenirLePrefixe,
} from './prefixe';

/// An in-memory store: the module must never touch `localStorage`
/// other than through the default of its argument (precedent of `jeton.ts`).
function coffre(entrees: Record<string, string> = {}) {
    return {
        getItem: (cle: string) => entrees[cle] ?? null,
        setItem: (cle: string, value: string) => {
            entrees[cle] = value;
        },
        removeItem: (cle: string) => {
            delete entrees[cle];
        },
    };
}

describe('lirePrefixe', () => {
    it('returns the store prefix when there is one', () => {
        expect(lirePrefixe(coffre({ [CLE_PREFIXE]: 'Zm9vYmFy' }), '')).toBe('Zm9vYmFy');
    });

    it('falls back to the query string when the store is empty', () => {
        expect(lirePrefixe(coffre(), '?prefixe=Zm9vYmFy')).toBe('Zm9vYmFy');
    });

    /// 🔴 THE PRIORITY ORDER, AND IT MATTERS: reversed, a `?prefixe=` left
    /// in a bookmarked URL would overwrite on EVERY reload the prefix
    /// the platform set, and the page would open the sessions of another VM.
    it('prefers the store to the query string', () => {
        expect(lirePrefixe(coffre({ [CLE_PREFIXE]: 'DU-COFFRE' }), '?prefixe=DE-L-URL')).toBe(
            'DU-COFFRE',
        );
    });

    /// Neither one nor the other: empty string, and certainly not `undefined` nor an
    /// exception — the page must fall back to `bureau`, exactly as
    /// before P3 (spec §10).
    it('returns the empty string when neither the store nor the query carry anything', () => {
        expect(lirePrefixe(coffre(), '')).toBe('');
        expect(lirePrefixe(coffre(), '?autre=chose')).toBe('');
    });

    /// An empty `?prefixe=` is an absence, not an empty prefix that
    /// would give `':bureau'`.
    it('treats an empty prefix in the query as an absence', () => {
        expect(lirePrefixe(coffre(), '?prefixe=')).toBe('');
    });

    /// ⚠️ THIS TEST WAS BORN FROM A MUTATION THAT STAYED GREEN: removing the
    /// `duCoffre !== ''` turned NOTHING red, for lack of a case where the store
    /// carries the key empty. An empty prefix in the store is an absence — otherwise
    /// it would hide the query and the page would fall back to `bureau`
    /// although it was explicitly given a VM name.
    it('treats an empty prefix in the store as an absence', () => {
        expect(lirePrefixe(coffre({ [CLE_PREFIXE]: '' }), '?prefixe=DE-L-URL')).toBe('DE-L-URL');
    });
});

describe('composer', () => {
    /// 🔴 THE MOST IMPORTANT TEST OF THE FILE. It is the only one that guarantees
    /// that the absent prefix restores EXACTLY today's
    /// behaviour. A silent `':bureau'` is the name of no existing
    /// session: the shell page would wait for a window that never comes,
    /// and nothing would report it.
    it("without a prefix, the session keeps exactly its current name", () => {
        expect(composer('', 'bureau')).toBe('bureau');
        expect(composer('', 'w-1')).toBe('w-1');
    });

    it('with a prefix, it precedes the name and is separated from it by a colon', () => {
        expect(composer('Zm9vYmFy', 'bureau')).toBe('Zm9vYmFy:bureau');
    });
});

describe('poserPrefixe', () => {
    it('writes the prefix, which lirePrefixe reads back', () => {
        const c = coffre();
        poserPrefixe(c, 'AB');
        expect(lirePrefixe(c, '')).toBe('AB');
    });

    /// 🔴 THE MOST IMPORTANT TEST OF THE FILE AFTER THAT OF `composer`.
    /// Writing an empty string to the store would not be neutral: `lirePrefixe`
    /// treats it as an absence, would fall back to `?prefixe=` then to `''`, and
    /// the page would SILENTLY join the shared namespace — the exact silent
    /// failure spec §10 names. An exception is what keeps it from going
    /// unnoticed.
    /// ⚠️ THE EXCEPTION IS MATCHED ON ITS TEXT, AND IT IS NOT FOR COMFORT.
    /// Written as a bare `toThrow()`, this test was GREEN while `poserPrefixe`
    /// did not exist yet: calling an absent function throws, and a
    /// `toThrow()` without a pattern is satisfied with that. It is the vacuous check this
    /// repository paid for four times in sub-block D10, caught here before green.
    it('THROWS on the empty string, rather than writing it to the store', () => {
        const c = coffre();
        expect(() => poserPrefixe(c, '')).toThrow(/empty prefix/);
        expect(c.getItem(CLE_PREFIXE)).toBeNull();
    });

    /// 🔴 THE STORE IS A PARAMETER, AND IT IS HONOURED. A module that had
    /// captured `globalThis.localStorage` — on load or on call — would write
    /// somewhere other than where the caller sent it, and the test above would stay
    /// green because the prefix would indeed be somewhere.
    it('writes into the PASSED store, never into a global', () => {
        const global = globalThis as { localStorage?: unknown };
        const before = global.localStorage;
        const espion = coffre();
        global.localStorage = espion;
        try {
            const mien = coffre();
            poserPrefixe(mien, 'AB');
            expect(lirePrefixe(mien, '')).toBe('AB');
            expect(espion.getItem(CLE_PREFIXE)).toBeNull();
        } finally {
            if (before === undefined) delete global.localStorage;
            else global.localStorage = before;
        }
    });
});

describe('effacerPrefixe', () => {
    /// Leaving in place the prefix of a VM we no longer have would open
    /// sessions in the name of another machine. And erasing is what gives
    /// the local trial mode back its `?prefixe=`: the store has priority, so as long
    /// as it carries something the query is useless.
    it('removes the key, and hands back to the query string', () => {
        const c = coffre({ [CLE_PREFIXE]: 'ANCIEN' });
        clearPrefix(c);
        expect(lirePrefixe(c, '?prefixe=Q')).toBe('Q');
    });
});

/* ══ WHAT THE FINAL REVIEW OF AUGUST 31ST 2026 ADDED ═════════════════════ */

describe('prefixeDeLaVm — the decision, PURE', () => {
    // 🔴 THIS RULE EXISTS BECAUSE THE HUB SET NO PREFIX (critical
    // ② of the final review). `poserPrefixe` had only ONE production
    // caller, on the SIGN-IN PAGE; a visitor behind Pomerium
    // gets their token ON THE HUB and never goes through that screen. The hub
    // therefore listened on `bureau` while the agent announced on
    // `<prefixe>:bureau`, and NO `fenetre-ouverte` ever arrived.

    it('retains the prefix announced by the VM', () => {
        expect(prefixeDeLaVm('vm-7')).toEqual({ action: 'poser', prefixe: 'vm-7' });
    });

    it('erases when the VM announces NONE (`null`)', () => {
        // `catalogue.ts::VmListee.prefixe` is `string | null`: `null` means
        // "this VM has no prefix", and keeping yesterday's would
        // open the sessions in the name of ANOTHER machine.
        expect(prefixeDeLaVm(null)).toEqual({ action: 'effacer' });
    });

    it('erases on the EMPTY STRING, instead of making `poserPrefixe` throw', () => {
        // ⚠️ `poserPrefixe` THROWS on `''`, and that is right FOR IT: a
        // caller who has no prefix has none to write. But a service
        // that announced `prefixe: ''` is not a programming error of the
        // client -- and making the catalogue population throw would be worse than
        // the defect being fixed.
        expect(prefixeDeLaVm('')).toEqual({ action: 'effacer' });
    });

    it('erases on what is not even a string', () => {
        expect(prefixeDeLaVm(undefined)).toEqual({ action: 'effacer' });
        expect(prefixeDeLaVm(42)).toEqual({ action: 'effacer' });
    });
});

describe('retenirLePrefixe — applying it to the store', () => {
    it('writes the received prefix', () => {
        const c = coffre();
        retenirLePrefixe(c, 'vm-7');
        expect(lirePrefixe(c, '')).toBe('vm-7');
    });

    it('ERASES yesterday one when the VM no longer announces one', () => {
        const c = coffre({ [CLE_PREFIXE]: 'ANCIEN' });
        retenirLePrefixe(c, null);
        expect(lirePrefixe(c, '')).toBe('');
    });

    it('does not THROW on an empty string', () => {
        const c = coffre({ [CLE_PREFIXE]: 'ANCIEN' });
        expect(() => retenirLePrefixe(c, '')).not.toThrow();
        expect(lirePrefixe(c, '')).toBe('');
    });
});
