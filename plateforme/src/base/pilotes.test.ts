// LA MÊME suite, contre les DEUX moteurs — c'est le critère ③ de P1.
//
// ⚠️ Ce que ces assertions vérifient sur SQLite était MESURÉ avant d'être
// écrit (index partiel, deux NULL tolérés, double attribution refusée,
// RETURNING). Le pendant Postgres ne l'était PAS, ni par la spec, ni par le
// plan : c'est précisément ce que cette suite a pour rôle d'établir plutôt que
// de croire.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, INSTANT_MIGRATION, MOTEUR } from './harnais';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from './migrations';
import type { Pilote } from './pilote';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe(`sous-ensemble portable, moteur=${MOTEUR}`, () => {
    it('applique les migrations, et le rejeu n’écrit rien de plus', async () => {
        base = await baseNeuve('idem');
        const suivi = await base.interroger<{ version: number }>(
            'SELECT version FROM schema_migration ORDER BY version',
            [],
        );
        // ⚠️ La liste est ÉCRITE EN DUR, et non dérivée du répertoire : une
        // comparaison contre `readdirSync` serait une tautologie qui ne
        // pourrait jamais échouer. Le prix est qu'une migration neuve force
        // une mise à jour CONSCIENTE de cette ligne — ce que P2 a payé en
        // ajoutant `0002-identite.sql`.
        expect(suivi.map((l) => Number(l.version))).toEqual([1, 2]);
        // Idempotence : le second passage n'applique rien.
        expect(await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, 2_000)).toBe(0);
        const apres = await base.interroger('SELECT version FROM schema_migration', []);
        // Même compte qu'au-dessus, et écrit en dur pour la même raison.
        expect(apres).toHaveLength(2);
    });

    it('tolère plusieurs VM non attribuées, et refuse une seconde attribution', async () => {
        base = await baseNeuve('index-partiel');
        await base.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
            ['u1', 'u1@exemple.test', 'x', 1]);
        await base.executer('INSERT INTO vm(id,nom,adresse,utilisateur_id) VALUES(?,?,?,?)',
            ['v1', 'vm-1', '10.0.0.1', null]);
        await base.executer('INSERT INTO vm(id,nom,adresse,utilisateur_id) VALUES(?,?,?,?)',
            ['v2', 'vm-2', '10.0.0.2', null]);
        // Deux NULL coexistent : c'est le point de l'index PARTIEL.
        expect(await base.interroger('SELECT id FROM vm', [])).toHaveLength(2);

        await base.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', ['u1', 'v1']);
        await expect(
            base.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', ['u1', 'v2']),
        ).rejects.toThrow();
    });

    it('applique la clé étrangère de vm.utilisateur_id', async () => {
        // Sur SQLite cela n'est vrai QUE parce que `PRAGMA foreign_keys=ON`
        // est posé à l'ouverture ; Postgres l'applique sans qu'on demande.
        base = await baseNeuve('fk');
        await expect(
            base.executer('INSERT INTO vm(id,nom,adresse,utilisateur_id) VALUES(?,?,?,?)',
                ['v9', 'vm-9', '10.0.0.9', 'fantome']),
        ).rejects.toThrow();
    });

    it('rend la ligne insérée par INSERT … RETURNING', async () => {
        base = await baseNeuve('returning');
        const lignes = await base.interroger<{ id: string; nom_session: string }>(
            'INSERT INTO session(id,nom_session,ouverte_a) VALUES(?,?,?) RETURNING id, nom_session',
            ['s-ret', 'bureau', 42],
        );
        expect(lignes).toHaveLength(1);
        expect(lignes[0].id).toBe('s-ret');
        expect(lignes[0].nom_session).toBe('bureau');
    });

    it('met à jour sur conflit par ON CONFLICT … DO UPDATE', async () => {
        base = await baseNeuve('upsert');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v1', 'ancien', '10.0.0.1']);
        await base.executer(
            'INSERT INTO vm(id,nom,adresse) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET nom = excluded.nom',
            ['v1', 'neuf', '10.0.0.1'],
        );
        const [ligne] = await base.interroger<{ nom: string }>('SELECT nom FROM vm WHERE id = ?', ['v1']);
        expect(ligne.nom).toBe('neuf');
    });

    it('annule toute la transaction quand son corps lève', async () => {
        base = await baseNeuve('rollback');
        await expect(
            base.transaction(async (tx) => {
                await tx.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v1', 'a', '10.0.0.1']);
                throw new Error('échec délibéré');
            }),
        ).rejects.toThrow(/délibéré/);
        expect(await base.interroger('SELECT id FROM vm', [])).toHaveLength(0);
    });
    it("porte un horodatage d'époque en millisecondes, sur les DEUX moteurs", async () => {
        // 🔴 Ce test existe parce que le lint statique ne peut rien contre lui
        // et que la double passe ne l'attrapait pas non plus : elle n'écrivait
        // que de PETITES valeurs. `INTEGER` vaut jusqu'à 8 octets sur SQLite et
        // exactement 4 sur Postgres — mesuré le 19 août 2026 sur PostgreSQL
        // 16.15 : `value "1787136773742" is out of range for type integer`.
        // Le service n'écrit pourtant que des `Date.now()` (≈ 1,79e12).
        //
        // C'est le troisième angle mort de la paire lint / double passe, et il
        // n'est couvert que par le CHOIX DES VALEURS : une suite qui écrit
        // `1_000` déclare portable un schéma qui refuse toute écriture réelle.
        const MS = 1_787_136_773_742;
        base = await baseNeuve('epoque');

        // (1) la table de suivi des migrations, écrite par `baseNeuve`
        const [suivi] = await base.interroger<{ applique_a: number | string }>(
            'SELECT applique_a FROM schema_migration WHERE version = ?',
            [1],
        );
        expect(Number(suivi.applique_a)).toBe(INSTANT_MIGRATION);

        // (2) la table `session`, ouverte puis close aux deux bornes
        await base.executer('INSERT INTO session(id,nom_session,ouverte_a) VALUES(?,?,?)',
            ['s-epoque', 'bureau', MS]);
        await base.executer('UPDATE session SET fermee_a = ? WHERE id = ?', [MS + 5, 's-epoque']);
        const [ligne] = await base.interroger<{ ouverte_a: number | string; fermee_a: number | string }>(
            'SELECT ouverte_a, fermee_a FROM session WHERE id = ?',
            ['s-epoque'],
        );
        expect(Number(ligne.ouverte_a)).toBe(MS);
        expect(Number(ligne.fermee_a)).toBe(MS + 5);

        // (3) les deux autres colonnes d'horodatage du socle
        await base.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
            ['u-epoque', 'e@exemple.test', 'x', MS]);
        await base.executer('INSERT INTO vm(id,nom,adresse,vue_a) VALUES(?,?,?,?)',
            ['v-epoque', 'vm', '10.0.0.1', MS]);
        const [u] = await base.interroger<{ cree_a: number | string }>(
            'SELECT cree_a FROM utilisateur WHERE id = ?', ['u-epoque']);
        expect(Number(u.cree_a)).toBe(MS);
    });
});
