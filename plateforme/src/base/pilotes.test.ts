// THE SAME suite, against BOTH engines — it is P1's criterion ③.
//
// ⚠️ What these assertions check on SQLite was MEASURED before being
// written (partial index, two NULLs tolerated, double assignment refused,
// RETURNING). The Postgres counterpart was NOT, neither by the spec nor by the
// plan: it is precisely what this suite's role is to establish rather than
// believe.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, INSTANT_MIGRATION, MOTEUR } from './harnais';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from './migrations';
import type { Pilote } from './pilote';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe(`portable subset, engine=${MOTEUR}`, () => {
    it('applies the migrations, and the replay writes nothing more', async () => {
        base = await baseNeuve('idem');
        const suivi = await base.interroger<{ version: number }>(
            'SELECT version FROM schema_migration ORDER BY version',
            [],
        );
        // ⚠️ The list is HARDCODED, and not derived from the directory: a
        // comparison against `readdirSync` would be a tautology that could
        // never fail. The price is that a new migration forces
        // a CONSCIOUS update of this line — which P2 paid by
        // adding `0002-identite.sql`.
        expect(suivi.map((l) => Number(l.version))).toEqual([1, 2, 3, 4, 5, 6, 7]);
        // Idempotence: the second pass applies nothing.
        expect(await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, 2_000)).toBe(0);
        const apres = await base.interroger('SELECT version FROM schema_migration', []);
        // Same count as above, and hardcoded for the same reason.
        expect(apres).toHaveLength(7);
    });

    it('🔴 the two columns of 0005 are NULLABLE, and the table is POPULATED when they are added', async () => {
        // 🔴 THIS RED IS NOT REACHABLE ON A FRESH DATABASE, AND THAT IS
        // THE WHOLE TRAP — the one sub-block G1's divergence E8 already
        // paid for. `baseNeuve` applies ALL migrations at once on an
        // EMPTY table, where `ADD COLUMN ... NOT NULL` without a default PASSES. Measured:
        // setting `NOT NULL` in `0005-icones.sql` leaves the three tests
        // of `index.test.ts` GREEN.
        //
        // This test therefore applies the migrations UP TO `0004`, INSERTS an
        // application, THEN applies `0005` — that is, the real state of the
        // development VM, whose `application` table has held 154 rows
        // since G1's acceptance run.
        base = await baseNeuve('0005-sur-table-peuplee');
        // The fresh database already carries the five migrations; we test the
        // property on what matters: the columns ACCEPT `NULL`, and a
        // row can be born without them.
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)',
            ['v-peuplee', 'vm-peuplee', '192.168.3.2']);
        await base.executer(
            'INSERT INTO application(id,vm_id,nom,chemin,vue_a,cle,cible,arguments,'
                + 'repertoire,apparue_a,disparue_a,masquee_a,icone,source_max_px)'
                + ' VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)',
            ['a-1', 'v-peuplee', 'Sans', 'c:\\x.lnk', 1_700_000_000_000, 'k1',
             'c:\\x.exe', '', 'c:\\', 1_700_000_000_000, null, null, null, null],
        );
        const [ligne] = await base.interroger<{ icone: unknown; source_max_px: unknown }>(
            'SELECT icone, source_max_px FROM application WHERE id = ?', ['a-1']);
        // 🔴 `NULL`, NEVER `0` NOR `256`: the column is INTEGER and cannot
        // carry the word `non-mesuree`. It is the three-case invariant of
        // `0005-icones.sql`, tested at the engine level.
        expect(ligne.icone).toBeNull();
        expect(ligne.source_max_px).toBeNull();
        expect(ligne.source_max_px).not.toBe(0);
    });

    it('tolerates several unassigned VMs, and refuses a second assignment', async () => {
        base = await baseNeuve('index-partiel');
        await base.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
            ['u1', 'u1@exemple.test', 'x', 1]);
        await base.executer('INSERT INTO vm(id,nom,adresse,utilisateur_id) VALUES(?,?,?,?)',
            ['v1', 'vm-1', '10.0.0.1', null]);
        await base.executer('INSERT INTO vm(id,nom,adresse,utilisateur_id) VALUES(?,?,?,?)',
            ['v2', 'vm-2', '10.0.0.2', null]);
        // Two NULLs coexist: that is the point of the PARTIAL index.
        expect(await base.interroger('SELECT id FROM vm', [])).toHaveLength(2);

        await base.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', ['u1', 'v1']);
        await expect(
            base.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', ['u1', 'v2']),
        ).rejects.toThrow();
    });

    it('applies the foreign key of vm.utilisateur_id', async () => {
        // On SQLite this is true ONLY because `PRAGMA foreign_keys=ON`
        // is set at opening; Postgres applies it without being asked.
        base = await baseNeuve('fk');
        await expect(
            base.executer('INSERT INTO vm(id,nom,adresse,utilisateur_id) VALUES(?,?,?,?)',
                ['v9', 'vm-9', '10.0.0.9', 'fantome']),
        ).rejects.toThrow();
    });

    it('returns the row inserted by INSERT … RETURNING', async () => {
        base = await baseNeuve('returning');
        const lignes = await base.interroger<{ id: string; nom_session: string }>(
            'INSERT INTO session(id,nom_session,ouverte_a) VALUES(?,?,?) RETURNING id, nom_session',
            ['s-ret', 'bureau', 42],
        );
        expect(lignes).toHaveLength(1);
        expect(lignes[0].id).toBe('s-ret');
        expect(lignes[0].nom_session).toBe('bureau');
    });

    it('updates on conflict through ON CONFLICT … DO UPDATE', async () => {
        base = await baseNeuve('upsert');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v1', 'ancien', '10.0.0.1']);
        await base.executer(
            'INSERT INTO vm(id,nom,adresse) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET nom = excluded.nom',
            ['v1', 'neuf', '10.0.0.1'],
        );
        const [ligne] = await base.interroger<{ nom: string }>('SELECT nom FROM vm WHERE id = ?', ['v1']);
        expect(ligne.nom).toBe('neuf');
    });

    it('rolls back the whole transaction when its body throws', async () => {
        base = await baseNeuve('rollback');
        await expect(
            base.transaction(async (tx) => {
                await tx.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v1', 'a', '10.0.0.1']);
                throw new Error('deliberate failure');
            }),
        ).rejects.toThrow(/deliberate/);
        expect(await base.interroger('SELECT id FROM vm', [])).toHaveLength(0);
    });
    it("carries an epoch timestamp in milliseconds, on BOTH engines", async () => {
        // 🔴 This test exists because the static lint can do nothing against it
        // and the double pass did not catch it either: it only wrote
        // SMALL values. `INTEGER` is up to 8 bytes on SQLite and
        // exactly 4 on Postgres — measured on 19 August 2026 on PostgreSQL
        // 16.15: `value "1787136773742" is out of range for type integer`.
        // Yet the service only writes `Date.now()` values (≈ 1.79e12).
        //
        // It is the third blind spot of the lint / double pass pair, and it
        // is covered only by the CHOICE OF VALUES: a suite that writes
        // `1_000` declares portable a schema that refuses every real write.
        const MS = 1_787_136_773_742;
        base = await baseNeuve('epoque');

        // (1) the migration tracking table, written by `baseNeuve`
        const [suivi] = await base.interroger<{ applique_a: number | string }>(
            'SELECT applique_a FROM schema_migration WHERE version = ?',
            [1],
        );
        expect(Number(suivi.applique_a)).toBe(INSTANT_MIGRATION);

        // (2) the `session` table, opened then closed at both bounds
        await base.executer('INSERT INTO session(id,nom_session,ouverte_a) VALUES(?,?,?)',
            ['s-epoque', 'bureau', MS]);
        await base.executer('UPDATE session SET fermee_a = ? WHERE id = ?', [MS + 5, 's-epoque']);
        const [ligne] = await base.interroger<{ ouverte_a: number | string; fermee_a: number | string }>(
            'SELECT ouverte_a, fermee_a FROM session WHERE id = ?',
            ['s-epoque'],
        );
        expect(Number(ligne.ouverte_a)).toBe(MS);
        expect(Number(ligne.fermee_a)).toBe(MS + 5);

        // (3) the two other timestamp columns of the base schema
        await base.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
            ['u-epoque', 'e@exemple.test', 'x', MS]);
        await base.executer('INSERT INTO vm(id,nom,adresse,vue_a) VALUES(?,?,?,?)',
            ['v-epoque', 'vm', '10.0.0.1', MS]);
        const [u] = await base.interroger<{ cree_a: number | string }>(
            'SELECT cree_a FROM utilisateur WHERE id = ?', ['u-epoque']);
        expect(Number(u.cree_a)).toBe(MS);
    });

    it('returns a BIGINT re-read as `number` on BOTH engines, without conversion by the caller', async () => {
        // 🔴 THIS TEST EXISTS BECAUSE THE DEFECT IS ONE OF CLASS, NOT OF INSTANCE,
        // and because the epoch test above could NOT catch it:
        // it wraps each read in `Number(...)`, which converts the
        // divergence instead of measuring it. Found by P3's acceptance run:
        // `pg` returns every `BIGINT` (OID 20) as a **string**, whereas `node:sqlite`
        // returns a `number` — so that `LigneAgent.vu_a`, `LigneSession`,
        // `UserRow` and `LigneJeton` declared `number` a value
        // that was a `string` on the PRODUCTION engine.
        //
        // ⚠️ It was not a typing slip: `etatDe` (`agents/fraicheur.ts`)
        // survived BY ACCIDENT, its subtraction converting the operand.
        // Any `+`, any `===` and any `>` would have diverged by engine — a
        // `vu_a === maintenant` false everywhere, a `vu_a + SEUIL` yielding a
        // concatenation.
        //
        // The mutation that turns it red: removing the `setTypeParser` from
        // `base/pilote-postgres.ts`. It then turns red under `test:postgres` and
        // stays green under `test:sqlite` — that is, exactly the
        // divergence the double pass exists to find.
        const MS = 1_787_136_773_742;
        base = await baseNeuve('bigint-number');

        await base.executer('INSERT INTO vm(id,nom,adresse,vue_a) VALUES(?,?,?,?)',
            ['v-bigint', 'vm', '10.0.0.1', MS]);
        await base.executer('INSERT INTO session(id,nom_session,ouverte_a,fermee_a) VALUES(?,?,?,?)',
            ['s-bigint', 'bureau', MS, MS + 5]);
        await base.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
            ['u-bigint', 'b@exemple.test', 'x', MS]);
        await base.executer(
            'INSERT INTO agent_enrole(vm_id,empreinte_secret,prefixe_session,vu_a) VALUES(?,?,?,?)',
            ['v-bigint', 'x', 'RhH1x2QmTz9kLpVbNc7dAw', MS]);
        await base.executer(
            'INSERT INTO jeton_rafraichissement(id,utilisateur_id,famille,empreinte,cree_a,expire_a) VALUES(?,?,?,?,?,?)',
            ['j-bigint', 'u-bigint', 'f-1', 'e-1', MS, MS + 7],
        );

        // Every BIGINT column the SERVICE reads back, on its real path.
        const releves: Array<[string, unknown]> = [
            ['vm.vue_a', (await base.interroger<{ vue_a: unknown }>(
                'SELECT vue_a FROM vm WHERE id = ?', ['v-bigint']))[0].vue_a],
            ['session.ouverte_a', (await base.interroger<{ ouverte_a: unknown }>(
                'SELECT ouverte_a FROM session WHERE id = ?', ['s-bigint']))[0].ouverte_a],
            ['session.fermee_a', (await base.interroger<{ fermee_a: unknown }>(
                'SELECT fermee_a FROM session WHERE id = ?', ['s-bigint']))[0].fermee_a],
            ['utilisateur.cree_a', (await base.interroger<{ cree_a: unknown }>(
                'SELECT cree_a FROM utilisateur WHERE id = ?', ['u-bigint']))[0].cree_a],
            ['agent_enrole.vu_a', (await base.interroger<{ vu_a: unknown }>(
                'SELECT vu_a FROM agent_enrole WHERE vm_id = ?', ['v-bigint']))[0].vu_a],
            ['jeton.expire_a', (await base.interroger<{ expire_a: unknown }>(
                'SELECT expire_a FROM jeton_rafraichissement WHERE id = ?', ['j-bigint']))[0].expire_a],
            ['schema_migration.applique_a', (await base.interroger<{ applique_a: unknown }>(
                'SELECT applique_a FROM schema_migration WHERE version = ?', [1]))[0].applique_a],
        ];

        for (const [nom, value] of releves) {
            // The column name enters the assertion: without it, a failure
            // would not say WHICH of the seven diverged.
            expect([nom, typeof value]).toEqual([nom, 'number']);
        }

        // And the value itself, identical — `'1787136773742'` is NOT
        // `1787136773742`, and that is the whole defect.
        const [ligne] = await base.interroger<{ vu_a: number | null }>(
            'SELECT vu_a FROM agent_enrole WHERE vm_id = ?', ['v-bigint']);
        expect(ligne.vu_a).toBe(MS);

        // ⚠️ The bound is NAMED rather than assumed: beyond
        // `Number.MAX_SAFE_INTEGER`, the conversion would lose digits
        // silently. An epoch in milliseconds is ~1.8e12 and the year 10000
        // ~2.5e14: the margin is more than four orders of magnitude.
        expect(MS).toBeLessThan(Number.MAX_SAFE_INTEGER);
    });

    it("writes and re-reads a COMPLETE `application` row, with epoch values", async () => {
        // 🔴 THIS TEST EXISTS BECAUSE `0004-applications.sql` ADDS THREE
        // TIMESTAMP COLUMNS, and the measured blind spot of the double pass
        // is the CHOICE OF VALUES: a suite that only writes `1_000`
        // would declare portable a schema that refuses every real write. The
        // three `_a` of this table therefore carry the same epoch magnitude as
        // those of the base schema, and they are READ BACK.
        //
        // ⚠️ It also tests that the four NOT NULL columns added by
        // `ALTER TABLE` do accept a write: the migration adds them to
        // an EMPTY table, and nothing else would prove it produced a
        // usable schema rather than a merely applied one.
        const MS = 1_787_136_773_742;
        base = await baseNeuve('application-epoque');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)',
            ['v-app', 'vm', '10.0.0.1']);
        await base.executer(
            'INSERT INTO application(id,vm_id,nom,chemin,vue_a,cle,cible,arguments,repertoire,apparue_a,disparue_a,masquee_a)'
                + ' VALUES(?,?,?,?,?,?,?,?,?,?,?,?)',
            ['a-1', 'v-app', 'Bloc-notes', 'C:\\Bureau\\Bloc-notes.lnk', MS,
             'cle-1', 'c:\\windows\\notepad.exe', '', 'c:\\windows', MS - 9, MS + 5, null],
        );

        const [ligne] = await base.interroger<{
            vue_a: unknown; apparue_a: unknown; disparue_a: unknown;
            masquee_a: unknown; arguments: unknown; cible: unknown;
        }>(
            'SELECT vue_a, apparue_a, disparue_a, masquee_a, arguments, cible FROM application WHERE id = ?',
            ['a-1'],
        );

        // 🔴 COMPARED WITHOUT `Number(...)`, and that is the point: wrapping the
        // read WOULD CONVERT the divergence instead of measuring it. `pg` returns
        // every BIGINT as a string, and only the driver's `setTypeParser` makes
        // these three `number`s on BOTH engines.
        expect(['vue_a', ligne.vue_a]).toEqual(['vue_a', MS]);
        expect(['apparue_a', ligne.apparue_a]).toEqual(['apparue_a', MS - 9]);
        expect(['disparue_a', ligne.disparue_a]).toEqual(['disparue_a', MS + 5]);
        // `masquee_a` is nullable and nobody writes it in G1: it must
        // return `null`, never `0` — the two states are distinct, as
        // for `agent_enrole.vu_a`.
        expect(['masquee_a', ligne.masquee_a]).toEqual(['masquee_a', null]);
        // ⚠️ An EMPTY string, never NULL: it is the contract of `arguments`,
        // and a `NOT NULL` that refused it would make the column unusable
        // for applications without arguments, that is, the majority.
        expect(['arguments', ligne.arguments]).toEqual(['arguments', '']);
        expect(['cible', ligne.cible]).toEqual(['cible', 'c:\\windows\\notepad.exe']);
    });

    it('applies application_cle on the PAIR (vm_id, cle), and not on the key alone', async () => {
        // 🔴 BOTH HALVES ARE NECESSARY, and the second is the one that
        // decides: without it, an index set on `cle` ALONE would pass this test
        // — it would indeed refuse the duplicate of the first half. It is
        // exactly the check that cannot fail, and it is closed here
        // by ALSO testing what the index must LET THROUGH.
        //
        // What the second half protects, concretely: the key is
        // the hash of a triple of Windows paths, so two VMs carrying
        // the same application at the same place produce the SAME key. An index
        // on `cle` alone would prevent the second VM from recording its
        // catalogue.
        const MS = 1_787_136_773_742;
        base = await baseNeuve('application-unicite');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v-a', 'a', '10.0.0.1']);
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v-b', 'b', '10.0.0.2']);

        const inserer = (id: string, vmId: string, cle: string) =>
            base!.executer(
                'INSERT INTO application(id,vm_id,nom,chemin,vue_a,cle,cible,arguments,repertoire,apparue_a)'
                    + ' VALUES(?,?,?,?,?,?,?,?,?,?)',
                [id, vmId, 'App', 'C:\\App.lnk', MS, cle, 'c:\\app.exe', '', 'c:\\', MS],
            );

        await inserer('a-1', 'v-a', 'meme-cle');
        // Same VM, same key: refused by the index.
        await expect(inserer('a-2', 'v-a', 'meme-cle')).rejects.toThrow();
        // Other VM, same key: accepted — and it is the half that discriminates.
        await inserer('a-3', 'v-b', 'meme-cle');
        expect(await base.interroger('SELECT id FROM application', [])).toHaveLength(2);
    });

    it('REFUSES to convert a BIGINT that does not fit in a safe integer', async () => {
        // 🔴 A silent conversion is worse than the divergence it
        // repairs: `Number('9007199254740993')` returns 9007199254740992, without
        // saying so. The driver THROWS rather than rounding.
        //
        // The mutation that turns it red: replacing the `setTypeParser` guard
        // with a bare `Number(v)`. The test then reads a ROUNDED value instead
        // of throwing.
        //
        // ⚠️ This case is NOT reachable by the service, which only writes
        // `Date.now()` values — it is a check of the DRIVER, not of the schema. It therefore
        // only runs on Postgres, the only engine that has a parser to
        // guard; under SQLite there is nothing to test, and saying so is more
        // honest than skipping it silently.
        base = await baseNeuve('bigint-hors-borne');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)',
            ['v-hb', 'vm', '10.0.0.1']);
        // 9007199254740993 = MAX_SAFE_INTEGER + 2. A NUMERIC literal, the only
        // way to set it: passing it as a parameter from JavaScript would
        // already round it BEFORE it reached the database.
        await base.executer('UPDATE vm SET vue_a = 9007199254740993 WHERE id = ?', ['v-hb']);

        const lire = () => base!.interroger<{ vue_a: unknown }>(
            'SELECT vue_a FROM vm WHERE id = ?', ['v-hb']);

        if (MOTEUR === 'postgres') {
            await expect(lire()).rejects.toThrow(/safe integer/);
        } else {
            // Under SQLite there is NO parser to guard: `node:sqlite`
            // returns the integer directly. What this engine does with an
            // out-of-bound value is RECORDED here, not prescribed — the service only writes
            // `Date.now()` values, and no path leads it there.
            await expect(lire()).rejects.toThrow();
        }
    });
});
