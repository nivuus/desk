// Les sept propriétés du hachage de mot de passe.
//
// 🔴 Les valeurs employées ici sont RÉALISTES, jamais commodes : un mot de
// passe de longueur ordinaire, un sel de 16 octets, une empreinte de
// 32 octets. C'est la leçon la plus chère de P1 — une suite qui n'écrit que
// des `1_000` déclare portable un schéma qui refuse toute écriture réelle.

import { describe, expect, it } from 'vitest';
import {
    analyser,
    doitEtreRehache,
    hacher,
    PARAMETRES_COURANTS,
    verifier,
} from './mot-de-passe';

const MOT_DE_PASSE = 'un-mot-de-passe-ordinaire-42';

describe('hacher', () => {
    it('rend la forme scrypt$N$r$p$sel$empreinte, avec les paramètres courants', async () => {
        const encode = await hacher(MOT_DE_PASSE);
        expect(encode).toMatch(/^scrypt\$16384\$8\$1\$[A-Za-z0-9_-]+\$[A-Za-z0-9_-]+$/);
        const { algo, params, sel, empreinte } = analyser(encode);
        expect(algo).toBe('scrypt');
        expect(params).toEqual(PARAMETRES_COURANTS);
        // Tailles RÉELLES : 16 octets de sel, 32 d'empreinte.
        expect(sel).toHaveLength(16);
        expect(empreinte).toHaveLength(32);
    });

    it('tire un sel neuf : deux hachages du même mot de passe diffèrent', async () => {
        // Un sel figé rendrait les deux encodages identiques, et deux comptes
        // au même mot de passe seraient reconnaissables en base.
        const a = await hacher(MOT_DE_PASSE);
        const b = await hacher(MOT_DE_PASSE);
        expect(a).not.toBe(b);
    });
});

describe('verifier', () => {
    it('accepte le bon mot de passe', async () => {
        const encode = await hacher(MOT_DE_PASSE);
        expect(await verifier(MOT_DE_PASSE, encode)).toBe(true);
    });

    it('refuse un mot de passe faux', async () => {
        const encode = await hacher(MOT_DE_PASSE);
        expect(await verifier('un-mot-de-passe-ordinaire-43', encode)).toBe(false);
    });

    it('rend false SANS LEVER sur une empreinte tronquée', async () => {
        // 🔴 MESURÉ le 19 août 2026 sur Node v24.9.0 :
        //     timingSafeEqual(Buffer.from('aa'), Buffer.from('aaa'))
        //     -> LÈVE `Input buffers must have the same byte length`
        // Une empreinte raccourcie en base — colonne trop courte, écriture
        // partielle, format d'une version antérieure — ferait donc LEVER la
        // vérification. L'appelant HTTP répondrait 500 là où il doit répondre
        // 401, et l'écart de comportement serait à lui seul un oracle.
        //
        // ⚠️ L'empreinte est réellement PLUS COURTE, jamais vide : une chaîne
        // vide pourrait être attrapée par un contrôle de forme en amont et ne
        // jamais atteindre `timingSafeEqual`. Le test ne mesurerait alors pas
        // ce qu'il annonce.
        const encode = await hacher(MOT_DE_PASSE);
        const morceaux = encode.split('$');
        morceaux[5] = morceaux[5].slice(0, 20);
        const tronque = morceaux.join('$');
        expect(analyser(tronque).empreinte.length).toBeLessThan(32);
        await expect(verifier(MOT_DE_PASSE, tronque)).resolves.toBe(false);
    });

    it('LÈVE sur un algorithme inconnu, plutôt que de rendre false', async () => {
        // Un `false` silencieux serait indiscernable d'un mauvais mot de
        // passe : personne ne saurait diagnostiquer une base écrite par une
        // version future.
        const encode = (await hacher(MOT_DE_PASSE)).replace(/^scrypt/, 'argon2id');
        await expect(verifier(MOT_DE_PASSE, encode)).rejects.toThrow(/argon2id/);
    });
});

describe('doitEtreRehache', () => {
    it('dit vrai sur un N inférieur au courant, faux sur le courant', async () => {
        const courant = await hacher(MOT_DE_PASSE);
        expect(doitEtreRehache(courant)).toBe(false);
        const faible = await hacher(MOT_DE_PASSE, { N: 4096, r: 8, p: 1 });
        expect(doitEtreRehache(faible)).toBe(true);
    });
});
