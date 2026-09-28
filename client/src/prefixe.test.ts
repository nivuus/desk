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

/// Un coffre en mémoire : le module ne doit jamais toucher `localStorage`
/// autrement que par le défaut de son argument (précédent de `jeton.ts`).
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

    /// 🔴 L'ORDRE DE PRIORITÉ, ET IL COMPTE : inversé, un `?prefixe=` resté
    /// dans une URL en favori écraserait à CHAQUE rechargement le préfixe que
    /// la plateforme a posé, et la page ouvrirait les sessions d'une autre VM.
    it('prefers the store to the query string', () => {
        expect(lirePrefixe(coffre({ [CLE_PREFIXE]: 'DU-COFFRE' }), '?prefixe=DE-L-URL')).toBe(
            'DU-COFFRE',
        );
    });

    /// Ni l'un ni l'autre : chaîne vide, et surtout pas `undefined` ni une
    /// exception — la page doit retomber sur `bureau`, exactement comme
    /// avant P3 (spec §10).
    it('returns the empty string when neither the store nor the query carry anything', () => {
        expect(lirePrefixe(coffre(), '')).toBe('');
        expect(lirePrefixe(coffre(), '?autre=chose')).toBe('');
    });

    /// Un `?prefixe=` vide est une absence, pas un préfixe vide qui
    /// vaudrait `':bureau'`.
    it('treats an empty prefix in the query as an absence', () => {
        expect(lirePrefixe(coffre(), '?prefixe=')).toBe('');
    });

    /// ⚠️ CE TEST EST NÉ D'UNE MUTATION RESTÉE VERTE : retirer le
    /// `duCoffre !== ''` ne rougissait RIEN, faute d'un cas où le coffre
    /// porte la clé à vide. Un préfixe vide au coffre est une absence — sans
    /// quoi il masquerait la requête et la page retomberait sur `bureau`
    /// alors qu'on lui a explicitement nommé une VM.
    it('treats an empty prefix in the store as an absence', () => {
        expect(lirePrefixe(coffre({ [CLE_PREFIXE]: '' }), '?prefixe=DE-L-URL')).toBe('DE-L-URL');
    });
});

describe('composer', () => {
    /// 🔴 LE TEST LE PLUS IMPORTANT DU FICHIER. C'est le seul qui garantisse
    /// que le préfixe absent restitue EXACTEMENT le comportement
    /// d'aujourd'hui. Un `':bureau'` silencieux n'est le nom d'aucune session
    /// existante : la page-shell attendrait une fenêtre qui ne vient jamais,
    /// et rien ne le signalerait.
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

    /// 🔴 LE TEST LE PLUS IMPORTANT DU FICHIER APRÈS CELUI DE `composer`.
    /// Écrire une chaîne vide dans le coffre ne serait pas neutre : `lirePrefixe`
    /// la traite comme une absence, retomberait sur `?prefixe=` puis sur `''`, et
    /// la page rejoindrait SILENCIEUSEMENT l'espace de noms partagé — la panne
    /// muette exacte que la spec §10 nomme. Une exception est ce qui l'empêche de
    /// passer inaperçue.
    /// ⚠️ L'EXCEPTION EST APPARIÉE SUR SON TEXTE, ET CE N'EST PAS DU CONFORT.
    /// Écrit `toThrow()` nu, ce test était VERT alors que `poserPrefixe`
    /// n'existait pas encore : appeler une fonction absente lève, et un
    /// `toThrow()` sans motif s'en contente. C'est le contrôle vacueux que ce
    /// dépôt a payé quatre fois au sous-bloc D10, attrapé ici avant le vert.
    it('THROWS on the empty string, rather than writing it to the store', () => {
        const c = coffre();
        expect(() => poserPrefixe(c, '')).toThrow(/empty prefix/);
        expect(c.getItem(CLE_PREFIXE)).toBeNull();
    });

    /// 🔴 LE COFFRE EST UN PARAMÈTRE, ET IL EST HONORÉ. Un module qui aurait
    /// capté `globalThis.localStorage` — au chargement ou à l'appel — écrirait
    /// ailleurs que là où l'appelant l'a envoyé, et le test ci-dessus resterait
    /// vert parce que le préfixe serait bien quelque part.
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
    /// Laisser en place le préfixe d'une VM qu'on n'a plus ferait ouvrir des
    /// sessions au nom d'une autre machine. Et l'effacement est ce qui rend au
    /// mode d'essai local son `?prefixe=` : le coffre a la priorité, donc tant
    /// qu'il porte quelque chose la requête ne sert à rien.
    it('removes the key, and hands back to the query string', () => {
        const c = coffre({ [CLE_PREFIXE]: 'ANCIEN' });
        clearPrefix(c);
        expect(lirePrefixe(c, '?prefixe=Q')).toBe('Q');
    });
});

/* ══ CE QUE LA REVUE FINALE DU 31 AOUT 2026 A AJOUTE ═════════════════════ */

describe('prefixeDeLaVm — the decision, PURE', () => {
    // 🔴 CETTE REGLE EXISTE PARCE QUE LE HUB NE POSAIT AUCUN PREFIXE (critique
    // ② de la revue finale). `poserPrefixe` n avait qu UN appelant de
    // production, sur la PAGE DE CONNEXION ; un visiteur derriere Pomerium
    // obtient son jeton SUR LE HUB et ne passe jamais par cet ecran. Le hub
    // ecoutait donc `bureau` pendant que l agent annoncait sur
    // `<prefixe>:bureau`, et AUCUN `fenetre-ouverte` n arrivait jamais.

    it('retains the prefix announced by the VM', () => {
        expect(prefixeDeLaVm('vm-7')).toEqual({ action: 'poser', prefixe: 'vm-7' });
    });

    it('erases when the VM announces NONE (`null`)', () => {
        // `catalogue.ts::VmListee.prefixe` est `string | null` : `null` veut
        // dire « cette VM n a pas de prefixe », et laisser celui d hier ferait
        // ouvrir les sessions au nom d une AUTRE machine.
        expect(prefixeDeLaVm(null)).toEqual({ action: 'effacer' });
    });

    it('erases on the EMPTY STRING, instead of making `poserPrefixe` throw', () => {
        // ⚠️ `poserPrefixe` LEVE sur `''`, et c est juste POUR LUI : un
        // appelant qui n a pas de prefixe n en a pas a ecrire. Mais un service
        // qui annoncerait `prefixe: ''` n est pas une programmation fausse du
        // client -- et faire lever le peuplement du catalogue serait pire que
        // le defaut qu on repare.
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
