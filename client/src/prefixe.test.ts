import { describe, expect, it } from 'vitest';

import { CLE_PREFIXE, composer, lirePrefixe } from './prefixe';

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
