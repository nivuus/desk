import { describe, expect, it } from 'vitest';

import {
    CLE_PREFIXE,
    composer,
    effacerPrefixe,
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
        setItem: (cle: string, valeur: string) => {
            entrees[cle] = valeur;
        },
        removeItem: (cle: string) => {
            delete entrees[cle];
        },
    };
}

describe('lirePrefixe', () => {
    it('rend le préfixe du coffre quand il y en a un', () => {
        expect(lirePrefixe(coffre({ [CLE_PREFIXE]: 'Zm9vYmFy' }), '')).toBe('Zm9vYmFy');
    });

    it('retombe sur la chaîne de requête quand le coffre est vide', () => {
        expect(lirePrefixe(coffre(), '?prefixe=Zm9vYmFy')).toBe('Zm9vYmFy');
    });

    /// 🔴 L'ORDRE DE PRIORITÉ, ET IL COMPTE : inversé, un `?prefixe=` resté
    /// dans une URL en favori écraserait à CHAQUE rechargement le préfixe que
    /// la plateforme a posé, et la page ouvrirait les sessions d'une autre VM.
    it('préfère le coffre à la chaîne de requête', () => {
        expect(lirePrefixe(coffre({ [CLE_PREFIXE]: 'DU-COFFRE' }), '?prefixe=DE-L-URL')).toBe(
            'DU-COFFRE',
        );
    });

    /// Ni l'un ni l'autre : chaîne vide, et surtout pas `undefined` ni une
    /// exception — la page doit retomber sur `bureau`, exactement comme
    /// avant P3 (spec §10).
    it('rend la chaîne vide quand ni le coffre ni la requête ne portent rien', () => {
        expect(lirePrefixe(coffre(), '')).toBe('');
        expect(lirePrefixe(coffre(), '?autre=chose')).toBe('');
    });

    /// Un `?prefixe=` vide est une absence, pas un préfixe vide qui
    /// vaudrait `':bureau'`.
    it('traite un préfixe vide de la requête comme une absence', () => {
        expect(lirePrefixe(coffre(), '?prefixe=')).toBe('');
    });

    /// ⚠️ CE TEST EST NÉ D'UNE MUTATION RESTÉE VERTE : retirer le
    /// `duCoffre !== ''` ne rougissait RIEN, faute d'un cas où le coffre
    /// porte la clé à vide. Un préfixe vide au coffre est une absence — sans
    /// quoi il masquerait la requête et la page retomberait sur `bureau`
    /// alors qu'on lui a explicitement nommé une VM.
    it('traite un préfixe vide du coffre comme une absence', () => {
        expect(lirePrefixe(coffre({ [CLE_PREFIXE]: '' }), '?prefixe=DE-L-URL')).toBe('DE-L-URL');
    });
});

describe('composer', () => {
    /// 🔴 LE TEST LE PLUS IMPORTANT DU FICHIER. C'est le seul qui garantisse
    /// que le préfixe absent restitue EXACTEMENT le comportement
    /// d'aujourd'hui. Un `':bureau'` silencieux n'est le nom d'aucune session
    /// existante : la page-shell attendrait une fenêtre qui ne vient jamais,
    /// et rien ne le signalerait.
    it("sans préfixe, la session garde exactement son nom d'aujourd'hui", () => {
        expect(composer('', 'bureau')).toBe('bureau');
        expect(composer('', 'w-1')).toBe('w-1');
    });

    it('avec un préfixe, il précède le nom et en est séparé par deux points', () => {
        expect(composer('Zm9vYmFy', 'bureau')).toBe('Zm9vYmFy:bureau');
    });
});

describe('poserPrefixe', () => {
    it('écrit le préfixe, que lirePrefixe relit', () => {
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
    it('LÈVE sur la chaîne vide, plutôt que de la coucher au coffre', () => {
        const c = coffre();
        expect(() => poserPrefixe(c, '')).toThrow(/préfixe vide/);
        expect(c.getItem(CLE_PREFIXE)).toBeNull();
    });

    /// 🔴 LE COFFRE EST UN PARAMÈTRE, ET IL EST HONORÉ. Un module qui aurait
    /// capté `globalThis.localStorage` — au chargement ou à l'appel — écrirait
    /// ailleurs que là où l'appelant l'a envoyé, et le test ci-dessus resterait
    /// vert parce que le préfixe serait bien quelque part.
    it('écrit dans le coffre PASSÉ, jamais dans un global', () => {
        const global = globalThis as { localStorage?: unknown };
        const avant = global.localStorage;
        const espion = coffre();
        global.localStorage = espion;
        try {
            const mien = coffre();
            poserPrefixe(mien, 'AB');
            expect(lirePrefixe(mien, '')).toBe('AB');
            expect(espion.getItem(CLE_PREFIXE)).toBeNull();
        } finally {
            if (avant === undefined) delete global.localStorage;
            else global.localStorage = avant;
        }
    });
});

describe('effacerPrefixe', () => {
    /// Laisser en place le préfixe d'une VM qu'on n'a plus ferait ouvrir des
    /// sessions au nom d'une autre machine. Et l'effacement est ce qui rend au
    /// mode d'essai local son `?prefixe=` : le coffre a la priorité, donc tant
    /// qu'il porte quelque chose la requête ne sert à rien.
    it('retire la clé, et rend la main à la chaîne de requête', () => {
        const c = coffre({ [CLE_PREFIXE]: 'ANCIEN' });
        effacerPrefixe(c);
        expect(lirePrefixe(c, '?prefixe=Q')).toBe('Q');
    });
});

/* ══ CE QUE LA REVUE FINALE DU 31 AOUT 2026 A AJOUTE ═════════════════════ */

describe('prefixeDeLaVm — la decision, PURE', () => {
    // 🔴 CETTE REGLE EXISTE PARCE QUE LE HUB NE POSAIT AUCUN PREFIXE (critique
    // ② de la revue finale). `poserPrefixe` n avait qu UN appelant de
    // production, sur la PAGE DE CONNEXION ; un visiteur derriere Pomerium
    // obtient son jeton SUR LE HUB et ne passe jamais par cet ecran. Le hub
    // ecoutait donc `bureau` pendant que l agent annoncait sur
    // `<prefixe>:bureau`, et AUCUN `fenetre-ouverte` n arrivait jamais.

    it('retient le prefixe annonce par la VM', () => {
        expect(prefixeDeLaVm('vm-7')).toEqual({ action: 'poser', prefixe: 'vm-7' });
    });

    it('efface quand la VM n en annonce AUCUN (`null`)', () => {
        // `catalogue.ts::VmListee.prefixe` est `string | null` : `null` veut
        // dire « cette VM n a pas de prefixe », et laisser celui d hier ferait
        // ouvrir les sessions au nom d une AUTRE machine.
        expect(prefixeDeLaVm(null)).toEqual({ action: 'effacer' });
    });

    it('efface sur la CHAINE VIDE, au lieu de faire lever `poserPrefixe`', () => {
        // ⚠️ `poserPrefixe` LEVE sur `''`, et c est juste POUR LUI : un
        // appelant qui n a pas de prefixe n en a pas a ecrire. Mais un service
        // qui annoncerait `prefixe: ''` n est pas une programmation fausse du
        // client -- et faire lever le peuplement du catalogue serait pire que
        // le defaut qu on repare.
        expect(prefixeDeLaVm('')).toEqual({ action: 'effacer' });
    });

    it('efface sur ce qui n est meme pas une chaine', () => {
        expect(prefixeDeLaVm(undefined)).toEqual({ action: 'effacer' });
        expect(prefixeDeLaVm(42)).toEqual({ action: 'effacer' });
    });
});

describe('retenirLePrefixe — l application au coffre', () => {
    it('ecrit le prefixe recu', () => {
        const c = coffre();
        retenirLePrefixe(c, 'vm-7');
        expect(lirePrefixe(c, '')).toBe('vm-7');
    });

    it('EFFACE celui d hier quand la VM n en annonce plus', () => {
        const c = coffre({ [CLE_PREFIXE]: 'ANCIEN' });
        retenirLePrefixe(c, null);
        expect(lirePrefixe(c, '')).toBe('');
    });

    it('ne LEVE pas sur une chaine vide', () => {
        const c = coffre({ [CLE_PREFIXE]: 'ANCIEN' });
        expect(() => retenirLePrefixe(c, '')).not.toThrow();
        expect(lirePrefixe(c, '')).toBe('');
    });
});
