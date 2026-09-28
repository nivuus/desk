// The `vm` repository, under BOTH engines — same harness as `pilotes.test.ts`.
//
// 🔴 THE TIMESTAMPS HAVE THE MAGNITUDE OF AN EPOCH IN MILLISECONDS,
// never convenient small numbers. It is the most expensive lesson of P1: the
// double pass only wrote `1_000`s, which fit in a 4-byte
// integer, and thus declared portable a schema Postgres refused for
// every real write.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { enroler, marquerVu } from './agent';
import { createUser } from './utilisateur';
import { attribuerSiLibre, detacher, lireParId, lireParNom, lister } from './vm';

/// A real epoch, not a small number: see the header.
const MS = 1_787_136_773_742;

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

/// Creates a VM. ⚠️ `depot/vm.ts` writes NO VM: the only creation
/// path is `admin/enroler-agent.ts`, and P4 adds no other (D1).
/// The tests therefore set the row themselves, as that command does.
async function poserVm(p: Pilote, id: string, nom: string): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [
        id,
        nom,
        '192.168.3.2',
    ]);
}

/// `vm.utilisateur_id` REFERENCES `user(id)`, and SQLite does enforce the
/// constraint (`pilote-sqlite.ts` sets `PRAGMA foreign_keys`). An invented
/// identifier would therefore make the assignment fail for a reason foreign to the
/// test.
async function seedUser(p: Pilote, email: string): Promise<string> {
    return createUser(p, email, 'empreinte-opaque-de-test', MS);
}

describe(`vm repository, engine=${MOTEUR}`, () => {
    it('🔴 `lister` returns the VM WITH its prefix and its `vu_a`, joined from agent_enrole', async () => {
        // 🔴 The red: replacing the `LEFT JOIN` with a `JOIN`. A VM enrolled
        // but that has never beaten would stay visible (its `vu_a` is null, not
        // its row) — but a VM created WITHOUT enrolment would disappear from
        // the inventory, without any error saying so.
        base = await baseNeuve('vm-lister');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');
        await marquerVu(base, 'v1', MS);
        // The second VM is NOT enrolled: it is the one that tests the LEFT.
        await poserVm(base, 'v2', 'w2');

        const lignes = await lister(base);
        expect(lignes.map((l) => l.id).sort()).toEqual(['v1', 'v2']);

        const v1 = lignes.find((l) => l.id === 'v1')!;
        expect(v1.nom).toBe('w1');
        expect(v1.adresse).toBe('192.168.3.2');
        expect(v1.prefixe_session).toBe('PREFIXEv1');
        expect(v1.utilisateur_id).toBeNull();

        const v2 = lignes.find((l) => l.id === 'v2')!;
        expect(v2.prefixe_session).toBeNull();
        expect(v2.vu_a).toBeNull();
    });

    it('🔴 `vu_a` is a NUMBER, not a string, after a real epoch', async () => {
        // 🔴 It is the CLASS defect P3 paid for: `pg` returns every `BIGINT`
        // as a STRING, and `interroger<T>` does an `as T[]` — no typing
        // could catch it. The remedy is in the driver
        // (`base/pilote-postgres.ts::setTypeParser`); this assertion is the
        // witness AT THE POINT OF USE, and it turns red under `test:postgres` if the
        // `setTypeParser` disappears.
        //
        // ⚠️ It would NOT turn red under `test:sqlite`, where the type has always been
        // right. A test green on a single engine says nothing here.
        base = await baseNeuve('vm-type-vua');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');
        await marquerVu(base, 'v1', MS);

        const [ligne] = await lister(base);
        expect(typeof ligne.vu_a).toBe('number');
        // And the EXACT VALUE: a `Number()` placed in the repository layer would hide the
        // type without anything saying so, but a truncation would show.
        expect(ligne.vu_a).toBe(MS);
    });

    it('`lireParId` and `lireParNom` return the same row, `undefined` on an unknown one', async () => {
        base = await baseNeuve('vm-lire');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');

        const parId = await lireParId(base, 'v1');
        const parNom = await lireParNom(base, 'w1');
        expect(parId).toEqual(parNom);
        expect(parId?.prefixe_session).toBe('PREFIXEv1');
        // NEVER an exception on an unknown: the reason would be indistinguishable
        // from a database defect, and on a route it would be an oracle.
        expect(await lireParId(base, 'v-inexistante')).toBeUndefined();
        expect(await lireParNom(base, 'w-inexistante')).toBeUndefined();
    });

    it('🔴 `attribuerSiLibre` on a FREE VM returns 1', async () => {
        base = await baseNeuve('vm-attrib-libre');
        await poserVm(base, 'v1', 'w1');
        const alice = await seedUser(base, 'alice@exemple.test');
        expect(await attribuerSiLibre(base, 'v1', alice)).toBe(1);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(alice);
    });

    it('🔴 `attribuerSiLibre` on an ALREADY TAKEN VM returns 0, without throwing', async () => {
        // 🔴 The red: removing `AND utilisateur_id IS NULL`. MEASURED on
        // BOTH engines (SQLite 3.50.4, PostgreSQL 16.15): the bare `UPDATE` then returns
        // `1 row` and alice's VM is STOLEN. The partial index
        // `vm_un_utilisateur` forbids NOTHING of this theft — it makes
        // `utilisateur_id` unique ACROSS ROWS, so it forbids a
        // user from having two VMs, never a VM from changing hands.
        base = await baseNeuve('vm-attrib-prise');
        await poserVm(base, 'v1', 'w1');
        const alice = await seedUser(base, 'alice@exemple.test');
        const bob = await seedUser(base, 'bob@exemple.test');
        await attribuerSiLibre(base, 'v1', alice);
        expect(await attribuerSiLibre(base, 'v1', bob)).toBe(0);
    });

    it('🔴 …and the ALREADY TAKEN VM did NOT change owner', async () => {
        // ⚠️ `it()` DISTINCT from the previous one, and it is property ②a
        // itself: `expect` interrupts a test at its first false
        // assertion, so that only one of the two would be tested. Under the
        // mutation, the count returned AND the row's state both change,
        // and both must show.
        base = await baseNeuve('vm-attrib-prise-etat');
        await poserVm(base, 'v1', 'w1');
        const alice = await seedUser(base, 'alice@exemple.test');
        const bob = await seedUser(base, 'bob@exemple.test');
        await attribuerSiLibre(base, 'v1', alice);
        await attribuerSiLibre(base, 'v1', bob);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(alice);
    });

    it('🔴 `attribuerSiLibre` for a user who ALREADY has a VM THROWS', async () => {
        // 🔴 The red: removing the `vm_un_utilisateur` index from
        // `0001-socle.sql`. MEASURED on both engines: the `UPDATE` then passes,
        // and the user ends up with two VMs. It is
        // property ②b, and it is distinct from ②a — this one THROWS, the other
        // returns `0 rows` without an exception.
        //
        // ⚠️ THE EXCEPTION TEXT DIFFERS FROM ONE ENGINE TO THE OTHER
        // (`UNIQUE constraint failed: vm.utilisateur_id` versus
        // `duplicate key value violates unique constraint "vm_un_utilisateur"`)
        // : this test asserts THAT IT THROWS, never what it says. No service code
        // compares this text either.
        base = await baseNeuve('vm-attrib-servi');
        await poserVm(base, 'v1', 'w1');
        await poserVm(base, 'v2', 'w2');
        const alice = await seedUser(base, 'alice@exemple.test');
        await attribuerSiLibre(base, 'v1', alice);
        await expect(attribuerSiLibre(base, 'v2', alice)).rejects.toThrow();
        // And `v2` stayed in the pool.
        expect((await lireParId(base, 'v2'))?.utilisateur_id).toBeNull();
    });

    it('`attribuerSiLibre` on an UNKNOWN VM returns 0 without throwing', async () => {
        // 🔴 The red: throwing. The reason would then be indistinguishable from a
        // database defect — and `changes = 0` does conflate THREE causes (E8), which
        // is precisely why the caller reads the row first.
        base = await baseNeuve('vm-attrib-inconnue');
        const alice = await seedUser(base, 'alice@exemple.test');
        expect(await attribuerSiLibre(base, 'v-inexistante', alice)).toBe(0);
    });

    it('🔴 `detacher` returns the VM to the pool, and a second assignment becomes possible again', async () => {
        // 🔴 The red: not setting back `null`. The VM would stay taken for life, and
        // `--detacher` of `admin:attribuer` would serve no purpose.
        base = await baseNeuve('vm-detacher');
        await poserVm(base, 'v1', 'w1');
        const alice = await seedUser(base, 'alice@exemple.test');
        const bob = await seedUser(base, 'bob@exemple.test');
        await attribuerSiLibre(base, 'v1', alice);
        expect(await detacher(base, 'v1')).toBe(1);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBeNull();
        expect(await attribuerSiLibre(base, 'v1', bob)).toBe(1);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(bob);
    });
});
