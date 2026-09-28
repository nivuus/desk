// Le dépôt `user`, joué contre le pilote que `PLATEFORME_BASE` désigne.
//
// 🔴 LES VALEURS SONT RÉELLES, jamais commodes : l'empreinte est produite par
// `hacher` — donc de la longueur qu'une base de production portera —, et
// l'horodatage est une ÉPOQUE EN MILLISECONDES. C'est le troisième angle mort
// du couple lint / double passe, celui du CHOIX DES VALEURS : `1_000` tient
// dans un entier 4 octets, `Date.now()` n'y tient pas.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { hacher } from '../identite/mot-de-passe';
import { createUser, lireParEmail, remplacerEmpreinte } from './utilisateur';

/// Une époque réelle en millisecondes, la même que `base/pilotes.test.ts`.
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
        // La longueur réelle d'une empreinte scrypt, relue sans troncature :
        // une colonne trop courte ferait ensuite LEVER `timingSafeEqual`.
        expect(ligne!.empreinte_mdp).toBe(empreinte);
        // 🔴 Valeur EXACTE, et de magnitude d'époque : c'est l'assertion qui
        // aurait rougi sur une colonne INTEGER côté Postgres.
        expect(Number(ligne!.cree_a)).toBe(MS);
    });

    it('REFUSES a second account with the same email', async () => {
        // L'index UNIQUE de `0001-socle.sql:32` : le retirer ferait passer les
        // deux insertions, et deux comptes homonymes rendraient
        // l'authentification non déterministe.
        base = await baseNeuve('util-unique');
        const empreinte = await hacher('an-ordinary-password-42');
        await createUser(base, 'ada@exemple.test', empreinte, MS);
        await expect(createUser(base, 'ada@exemple.test', empreinte, MS + 1))
            .rejects.toThrow();
        // Et rien n'a été ajouté.
        const all = await base.interroger<{ n: number | string }>(
            'SELECT COUNT(*) AS n FROM utilisateur',
            [],
        );
        expect(Number(all[0].n)).toBe(1);
    });

    it('returns undefined on an unknown email, NEVER an exception', async () => {
        // Un `lignes[0].id` sur un tableau vide lèverait, et l'appelant HTTP
        // répondrait 500 là où il doit répondre 401 — l'écart de comportement
        // serait à lui seul un oracle d'énumération de comptes.
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
        // Le courriel et l'instant de création sont intacts : un `UPDATE` trop
        // large les emporterait sans que rien ne le dise.
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
