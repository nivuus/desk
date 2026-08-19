// Le dépôt `utilisateur`, joué contre le pilote que `PLATEFORME_BASE` désigne.
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
import { creerUtilisateur, lireParEmail, remplacerEmpreinte } from './utilisateur';

/// Une époque réelle en millisecondes, la même que `base/pilotes.test.ts`.
const MS = 1_787_136_773_742;

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe(`dépôt utilisateur, moteur=${MOTEUR}`, () => {
    it('crée un compte et le relit par courriel, à l’époque EXACTE écrite', async () => {
        base = await baseNeuve('util-creer');
        const empreinte = await hacher('un-mot-de-passe-ordinaire-42');
        const id = await creerUtilisateur(base, 'ada@exemple.test', empreinte, MS);

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

    it('REFUSE un second compte au même courriel', async () => {
        // L'index UNIQUE de `0001-socle.sql:32` : le retirer ferait passer les
        // deux insertions, et deux comptes homonymes rendraient
        // l'authentification non déterministe.
        base = await baseNeuve('util-unique');
        const empreinte = await hacher('un-mot-de-passe-ordinaire-42');
        await creerUtilisateur(base, 'ada@exemple.test', empreinte, MS);
        await expect(creerUtilisateur(base, 'ada@exemple.test', empreinte, MS + 1))
            .rejects.toThrow();
        // Et rien n'a été ajouté.
        const toutes = await base.interroger<{ n: number | string }>(
            'SELECT COUNT(*) AS n FROM utilisateur',
            [],
        );
        expect(Number(toutes[0].n)).toBe(1);
    });

    it('rend undefined sur un courriel inconnu, JAMAIS une exception', async () => {
        // Un `lignes[0].id` sur un tableau vide lèverait, et l'appelant HTTP
        // répondrait 500 là où il doit répondre 401 — l'écart de comportement
        // serait à lui seul un oracle d'énumération de comptes.
        base = await baseNeuve('util-inconnu');
        await expect(lireParEmail(base, 'personne@exemple.test')).resolves.toBeUndefined();
    });

    it('remplace l’empreinte, et RIEN d’autre', async () => {
        base = await baseNeuve('util-rehache');
        const ancienne = await hacher('un-mot-de-passe-ordinaire-42', { N: 4096, r: 8, p: 1 });
        const id = await creerUtilisateur(base, 'ada@exemple.test', ancienne, MS);

        const neuve = await hacher('un-mot-de-passe-ordinaire-42');
        await remplacerEmpreinte(base, id, neuve);

        const ligne = await lireParEmail(base, 'ada@exemple.test');
        expect(ligne!.empreinte_mdp).toBe(neuve);
        // Le courriel et l'instant de création sont intacts : un `UPDATE` trop
        // large les emporterait sans que rien ne le dise.
        expect(ligne!.email).toBe('ada@exemple.test');
        expect(Number(ligne!.cree_a)).toBe(MS);
        expect(ligne!.id).toBe(id);
    });

    it('donne un identifiant distinct à chaque compte', async () => {
        base = await baseNeuve('util-ids');
        const empreinte = await hacher('un-mot-de-passe-ordinaire-42');
        const un = await creerUtilisateur(base, 'ada@exemple.test', empreinte, MS);
        const deux = await creerUtilisateur(base, 'grace@exemple.test', empreinte, MS + 1);
        expect(deux).not.toBe(un);
    });
});
