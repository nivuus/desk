// The seven properties of password hashing.
//
// 🔴 The values used here are REALISTIC, never convenient: a password
// of ordinary length, a 16-byte salt, a 32-byte
// hash. It is the most expensive lesson of P1 — a suite that only writes
// `1_000`s declares portable a schema that refuses every real write.

import { describe, expect, it } from 'vitest';
import {
    analyser,
    doitEtreRehache,
    hacher,
    CURRENT_PARAMS,
    verify,
} from './mot-de-passe';

const MOT_DE_PASSE = 'un-mot-de-passe-ordinaire-42';

describe('hacher', () => {
    it('returns the shape scrypt$N$r$p$salt$fingerprint, with the current parameters', async () => {
        const encode = await hacher(MOT_DE_PASSE);
        expect(encode).toMatch(/^scrypt\$16384\$8\$1\$[A-Za-z0-9_-]+\$[A-Za-z0-9_-]+$/);
        const { algo, params, sel, empreinte } = analyser(encode);
        expect(algo).toBe('scrypt');
        expect(params).toEqual(CURRENT_PARAMS);
        // REAL sizes: 16 bytes of salt, 32 of hash.
        expect(sel).toHaveLength(16);
        expect(empreinte).toHaveLength(32);
    });

    it('draws a fresh salt: two hashes of the same password differ', async () => {
        // A frozen salt would make both encodings identical, and two accounts
        // with the same password would be recognisable in the database.
        const a = await hacher(MOT_DE_PASSE);
        const b = await hacher(MOT_DE_PASSE);
        expect(a).not.toBe(b);
    });
});

describe('verifier', () => {
    it('accepts the right password', async () => {
        const encode = await hacher(MOT_DE_PASSE);
        expect(await verify(MOT_DE_PASSE, encode)).toBe(true);
    });

    it('refuses a wrong password', async () => {
        const encode = await hacher(MOT_DE_PASSE);
        expect(await verify('un-mot-de-passe-ordinaire-43', encode)).toBe(false);
    });

    it('returns false WITHOUT THROWING on a truncated fingerprint', async () => {
        // 🔴 MEASURED on 19 August 2026 on Node v24.9.0:
        //     timingSafeEqual(Buffer.from('aa'), Buffer.from('aaa'))
        //     -> THROWS `Input buffers must have the same byte length`
        // A shortened hash in the database — column too short, partial
        // write, format of an earlier version — would therefore make the
        // verification THROW. The HTTP caller would answer 500 where it must answer
        // 401, and the difference in behaviour would on its own be an oracle.
        //
        // ⚠️ The hash is really SHORTER, never empty: an empty
        // string could be caught by an upstream shape check and never
        // reach `timingSafeEqual`. The test would then not measure
        // what it announces.
        const encode = await hacher(MOT_DE_PASSE);
        const morceaux = encode.split('$');
        morceaux[5] = morceaux[5].slice(0, 20);
        const tronque = morceaux.join('$');
        expect(analyser(tronque).empreinte.length).toBeLessThan(32);
        await expect(verify(MOT_DE_PASSE, tronque)).resolves.toBe(false);
    });

    it('THROWS on an unknown algorithm, rather than returning false', async () => {
        // A silent `false` would be indistinguishable from a wrong
        // password: nobody could diagnose a database written by a
        // future version.
        const encode = (await hacher(MOT_DE_PASSE)).replace(/^scrypt/, 'argon2id');
        await expect(verify(MOT_DE_PASSE, encode)).rejects.toThrow(/argon2id/);
    });
});

describe('doitEtreRehache', () => {
    it('tells true on an N below the current one, false on the current one', async () => {
        const current = await hacher(MOT_DE_PASSE);
        expect(doitEtreRehache(current)).toBe(false);
        const faible = await hacher(MOT_DE_PASSE, { N: 4096, r: 8, p: 1 });
        expect(doitEtreRehache(faible)).toBe(true);
    });
});
