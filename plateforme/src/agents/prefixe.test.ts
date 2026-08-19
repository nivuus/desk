// Le préfixe opaque de session : ce qui rend l'espace de noms GLOBAL.
//
// 🔴 La rouge centrale de ce fichier est « le préfixe ne contient jamais le
// séparateur ». Tout D2 du plan repose dessus : l'identifiant TURN vaut
// `<expiration>:<session>`, donc `<expiration>:<préfixe>:<nom>` une fois la
// session préfixée, et coturn coupe sur le PREMIER `:`. Si l'alphabet du
// préfixe pouvait porter un `:`, la première borne deviendrait ambiguë.

import { describe, expect, it } from 'vitest';
import { deriverIdentifiants } from '../signaling/ice';
import { SEPARATEUR, composer, decouper, nouveauPrefixe } from './prefixe';

describe('le préfixe opaque', () => {
    it('rend 22 caractères de l’alphabet base64url', () => {
        // 16 octets en base64url font 22 caractères sans remplissage. L'alphabet
        // exclut `+` et `/` : `/` casserait un jour un découpage d'URL, et `+`
        // se transforme en espace dans une chaîne de requête mal décodée.
        const p = nouveauPrefixe();
        expect(p).toHaveLength(22);
        expect(p).toMatch(/^[A-Za-z0-9_-]{22}$/);
    });

    it('rend deux valeurs DIFFÉRENTES sur deux appels', () => {
        // Une graine fixe rendrait le préfixe devinable, donc l'espace de noms
        // global mais pas opaque.
        expect(nouveauPrefixe()).not.toBe(nouveauPrefixe());
    });

    it('🔴 ne contient JAMAIS le séparateur, sur un grand nombre de tirages', () => {
        // C'est l'assertion sur laquelle repose la non-ambiguïté du format
        // TURN. 500 tirages, soit 11 000 caractères : un alphabet qui
        // porterait `:` le montrerait ici.
        for (let i = 0; i < 500; i += 1) {
            expect(nouveauPrefixe()).not.toContain(SEPARATEUR);
        }
    });

    it('compose `<préfixe>:<nom>`', () => {
        expect(composer('PPP', 'bureau')).toBe('PPP:bureau');
        expect(composer('PPP', 'w-1')).toBe('PPP:w-1');
    });

    it('compose SANS séparateur quand le préfixe est vide', () => {
        // 🔴 C'est ce qui restitue EXACTEMENT le comportement d'avant P3 —
        // `bureau`, `w-1`. Poser le séparateur inconditionnellement rendrait
        // `:bureau`, qui n'est le nom d'aucune session existante, et rien ne
        // le signalerait.
        expect(composer('', 'bureau')).toBe('bureau');
        expect(composer('', 'w-1')).toBe('w-1');
    });

    it('découpe sur le PREMIER séparateur, et rend un préfixe vide à défaut', () => {
        expect(decouper('PPP:bureau')).toEqual({ prefixe: 'PPP', nom: 'bureau' });
        // Sans préfixe : le mode d'essai local, qui doit rester EXPLICITE et
        // non lever.
        expect(decouper('bureau')).toEqual({ prefixe: '', nom: 'bureau' });
        // Découper sur le DERNIER séparateur laisserait passer `PPP:w-1` mais
        // casserait un nom qui porterait lui-même un `:`.
        expect(decouper('PPP:a:b')).toEqual({ prefixe: 'PPP', nom: 'a:b' });
    });

    it('🔴 laisse le PREMIER segment de l’identifiant TURN non ambigu', () => {
        // Le contrôle d'E6, figé sur la chaîne exacte comme `ice.test.ts:13`.
        // `maintenant` en millisecondes, la durée en secondes : 1 000 + 3 600.
        const p = 'AAAAAAAAAAAAAAAAAAAAAA';
        const { username } = deriverIdentifiants('secret', composer(p, 'bureau'), 3600, 1_000_000);
        expect(username).toBe('4600:AAAAAAAAAAAAAAAAAAAAAA:bureau');
        // Et c'est bien l'expiration que coturn lira en coupant sur le premier
        // `:` — mettre l'expiration APRÈS la session ferait tomber ceci.
        expect(username.split(SEPARATEUR)[0]).toBe('4600');
    });
});
