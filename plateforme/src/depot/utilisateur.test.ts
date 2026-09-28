// The `user` repository, played against the driver `PLATEFORME_BASE` designates.
//
// 🔴 THE VALUES ARE REAL, never convenient: the hash is produced by
// `hacher` — hence of the length a production database will carry —, and
// the timestamp is an EPOCH IN MILLISECONDS. It is the third blind spot
// of the lint / double pass pair, that of the CHOICE OF VALUES: `1_000` fits
// in a 4-byte integer, `Date.now()` does not.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { hacher } from '../identite/mot-de-passe';
import { createUser, lireParEmail, remplacerEmpreinte } from './utilisateur';

/// A real epoch in milliseconds, the same as `base/pilotes.test.ts`.
const MS = 1_787_136_773_742;

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe(`user repository, engine=${MOTEUR}`, () => {
    it('creates an account and re-reads it by email, at the EXACT epoch written', async () => {
        base = await baseNeuve('user-create');
        const empreinte = await hacher('an-ordinary-password-42');
        const id = await createUser(base, 'ada@exemple.test', empreinte, MS);

        const ligne = await lireParEmail(base, 'ada@exemple.test');
        expect(ligne).toBeDefined();
        expect(ligne!.id).toBe(id);
        expect(ligne!.email).toBe('ada@exemple.test');
        // The real length of a scrypt hash, read back without truncation:
        // a column too short would then make `timingSafeEqual` THROW.
        expect(ligne!.empreinte_mdp).toBe(empreinte);
        // 🔴 EXACT value, and of epoch magnitude: it is the assertion that
        // would have turned red on an INTEGER column on the Postgres side.
        expect(Number(ligne!.cree_a)).toBe(MS);
    });

    it('REFUSES a second account with the same email', async () => {
        // The UNIQUE index of `0001-socle.sql:32`: removing it would let both
        // insertions pass, and two accounts with the same email would make
        // authentication non-deterministic.
        base = await baseNeuve('util-unique');
        const empreinte = await hacher('an-ordinary-password-42');
        await createUser(base, 'ada@exemple.test', empreinte, MS);
        await expect(createUser(base, 'ada@exemple.test', empreinte, MS + 1))
            .rejects.toThrow();
        // And nothing was added.
        const all = await base.interroger<{ n: number | string }>(
            'SELECT COUNT(*) AS n FROM utilisateur',
            [],
        );
        expect(Number(all[0].n)).toBe(1);
    });

    it('returns undefined on an unknown email, NEVER an exception', async () => {
        // A `lignes[0].id` on an empty array would throw, and the HTTP caller
        // would answer 500 where it must answer 401 — the difference in behaviour
        // would on its own be an account enumeration oracle.
        base = await baseNeuve('util-inconnu');
        await expect(lireParEmail(base, 'personne@exemple.test')).resolves.toBeUndefined();
    });

    it('replaces the fingerprint, and NOTHING else', async () => {
        base = await baseNeuve('util-rehache');
        const ancienne = await hacher('an-ordinary-password-42', { N: 4096, r: 8, p: 1 });
        const id = await createUser(base, 'ada@exemple.test', ancienne, MS);

        const neuve = await hacher('an-ordinary-password-42');
        await remplacerEmpreinte(base, id, neuve);

        const ligne = await lireParEmail(base, 'ada@exemple.test');
        expect(ligne!.empreinte_mdp).toBe(neuve);
        // The email and the creation instant are intact: a too broad `UPDATE`
        // would take them away without anything saying so.
        expect(ligne!.email).toBe('ada@exemple.test');
        expect(Number(ligne!.cree_a)).toBe(MS);
        expect(ligne!.id).toBe(id);
    });

    it('gives a distinct identifier to each account', async () => {
        base = await baseNeuve('util-ids');
        const empreinte = await hacher('an-ordinary-password-42');
        const un = await createUser(base, 'ada@exemple.test', empreinte, MS);
        const deux = await createUser(base, 'grace@exemple.test', empreinte, MS + 1);
        expect(deux).not.toBe(un);
    });
});
