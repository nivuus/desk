// Replacing a user's password revokes every refresh family of that user:
// whoever held a session opened with the old password loses it.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { emettre, tourner } from './jeton';
import { createUser, lireParEmail, remplacerMotDePasse } from './utilisateur';

const MS = 1_787_136_773_742;
let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe(`remplacerMotDePasse, engine=${MOTEUR}`, () => {
    it("stores the new digest and revokes all of the user's families, and no one else's", async () => {
        base = await baseNeuve('mdp-remplace');
        const ada = await createUser(base, 'ada@exemple.test', 'old-digest', MS);
        const bob = await createUser(base, 'bob@exemple.test', 'bob-digest', MS);
        const un = await emettre(base, ada, MS);
        const deux = await emettre(base, ada, MS);
        const deBob = await emettre(base, bob, MS);

        expect(await remplacerMotDePasse(base, ada, 'new-digest', MS + 1_000)).toBe(2);

        expect((await lireParEmail(base, 'ada@exemple.test'))?.empreinte_mdp).toBe('new-digest');
        expect(await tourner(base, un, MS + 2_000)).toEqual({ ok: false, motif: 'revoque' });
        expect(await tourner(base, deux, MS + 2_000)).toEqual({ ok: false, motif: 'revoque' });
        // Witness: another user's session is untouched.
        expect(await tourner(base, deBob, MS + 2_000)).toMatchObject({ ok: true });
    });
});
