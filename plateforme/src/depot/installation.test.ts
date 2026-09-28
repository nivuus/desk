// The two upload repositories, under `test:sqlite` AND under `test:postgres`.
//
// 🔴 THE VALUES ARE REALISTIC, NEVER CONVENIENT: the timestamps carry an
// EPOCH MAGNITUDE and the sizes that of a real installer. It is the most
// expensive lesson of P1 — the double pass only wrote `1_000`s, and
// declared portable a schema Postgres refused for every real write.
//
// 🔴 THIS FILE CARRIES THE THREE REDS OF MIGRATION `0006`, and without it
// it would have none: a migration alone cannot fail other than by
// not applying.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import {
    compterEnCours,
    create as createUpload,
    lireParId as lireTeleversement,
    lirePlusVieuxQue,
    sceller,
    remove,
} from './televersement';
import {
    avancer,
    create as createInstallation,
    lireEnAttentePourVm,
    lireParId as lireInstallation,
    terminer,
} from './installation';

let base: Pilote | undefined;

/// The magnitude that really broke Postgres in P1.
const MS = 1_787_136_773_742;
/// A 3 GB installer: beyond Postgres' 32-bit integer.
const TROIS_GO = 3_221_225_472;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function socle(p: Pilote): Promise<{ user: string; vm: string }> {
    await p.executer(
        'INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
        ['u-1', 'a@b.c', 'scrypt$1$1$1$x$y', MS],
    );
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [
        'v-1',
        'g3',
        '192.168.3.2',
    ]);
    return { user: 'u-1', vm: 'v-1' };
}

describe(`upload repository, engine=${MOTEUR}`, () => {
    it('creates, re-reads, and seals', async () => {
        base = await baseNeuve('tel-cree');
        const { user } = await socle(base);
        const ligne = await createUpload(
            base,
            {
                userId: user,
                nom: 'Firefox Setup 130.0.exe',
                taille: TROIS_GO,
                sha256: 'a'.repeat(64),
                chunkSize: 8 * 1024 * 1024,
            },
            MS,
        );
        expect(ligne.scelle_a).toBeNull();

        const relu = await lireTeleversement(base, ligne.id);
        expect(relu?.nom).toBe('Firefox Setup 130.0.exe');
        expect(relu?.taille).toBe(TROIS_GO);
        expect(relu?.cree_a).toBe(MS);
        expect(relu?.scelle_a).toBeNull();

        await sceller(base, ligne.id, MS + 5_000);
        expect((await lireTeleversement(base, ligne.id))?.scelle_a).toBe(MS + 5_000);
    });

    // 🔴 THE `BIGINT` RED, AND IT ONLY SHOWS ON THE POSTGRES PASS.
    // `INTEGER` is 8 bytes on SQLite and EXACTLY 4 on Postgres: a
    // 3 GB size and a `Date.now()` both overflow it. It is the
    // defect P1 found at acceptance, in its OWN migrations, and which
    // the two existing guards — the lexical lint and the double pass — could
    // see nothing of: `INTEGER` is a legal type, and the suite
    // only wrote small values.
    //
    // ⚠️ AND THE TEST COMPARES `typeof`, NOT ONLY THE VALUE: `pg` returns every
    // `int8` as TEXT, and `interroger<T>` does an `as T[]` — no typing would
    // catch it. It is the second defect P3 found, one of class, and the
    // remedy lives IN THE DRIVER (`setTypeParser`), never in a local patch.
    it('🔴 returns NUMBERS, not strings, on real magnitudes', async () => {
        base = await baseNeuve('tel-nombres');
        const { user } = await socle(base);
        const ligne = await createUpload(
            base,
            {
                userId: user,
                nom: 'gros.msi',
                taille: TROIS_GO,
                sha256: 'b'.repeat(64),
                chunkSize: 8 * 1024 * 1024,
            },
            MS,
        );
        const relu = await lireTeleversement(base, ligne.id);
        expect([typeof relu?.taille, typeof relu?.cree_a, typeof relu?.taille_tranche]).toEqual([
            'number',
            'number',
            'number',
        ]);
        expect(relu?.taille).toBe(TROIS_GO);
    });

    it('counts the IN PROGRESS ones, and a sealed one no longer is', async () => {
        base = await baseNeuve('tel-quota');
        const { user } = await socle(base);
        const a = await createUpload(
            base,
            { userId: user, nom: 'a.exe', taille: 1, sha256: 'c'.repeat(64), chunkSize: 8 },
            MS,
        );
        await createUpload(
            base,
            { userId: user, nom: 'b.exe', taille: 1, sha256: 'd'.repeat(64), chunkSize: 8 },
            MS,
        );
        expect(await compterEnCours(base, user)).toBe(2);
        await sceller(base, a.id, MS + 1);
        expect(await compterEnCours(base, user)).toBe(1);
        // Another user does not count toward the quota.
        expect(await compterEnCours(base, 'u-inconnu')).toBe(0);
    });

    it('the age sweep returns what is older than the bound', async () => {
        base = await baseNeuve('tel-age');
        const { user } = await socle(base);
        const vieux = await createUpload(
            base,
            { userId: user, nom: 'v.exe', taille: 1, sha256: 'e'.repeat(64), chunkSize: 8 },
            MS - 100_000,
        );
        await createUpload(
            base,
            { userId: user, nom: 'n.exe', taille: 1, sha256: 'f'.repeat(64), chunkSize: 8 },
            MS,
        );
        const a_purger = await lirePlusVieuxQue(base, MS - 1);
        expect(a_purger.map((l) => l.id)).toEqual([vieux.id]);
        await remove(base, vieux.id);
        expect(await lireTeleversement(base, vieux.id)).toBeUndefined();
    });

    // 🔴 THE FOREIGN KEY RED, FIRST HALF. Without
    // `REFERENCES user(id)`, this insertion WOULD PASS — and an
    // orphan upload would belong to nobody, hence would escape
    // any owner check. Foreign keys are ENFORCED
    // on both sides: `pilote-sqlite.ts` sets `PRAGMA foreign_keys = ON`.
    it('🔴 REFUSES an upload whose user does not exist', async () => {
        base = await baseNeuve('tel-orphelin');
        await expect(
            createUpload(
                base,
                { userId: 'u-fantome', nom: 'x.exe', taille: 1, sha256: 'g'.repeat(64), chunkSize: 8 },
                MS,
            ),
        ).rejects.toThrow();
    });
});

describe(`installation repository, engine=${MOTEUR}`, () => {
    async function withUpload(p: Pilote): Promise<{ vm: string; tel: string }> {
        const { user, vm } = await socle(p);
        const tel = await createUpload(
            p,
            {
                userId: user,
                nom: 'setup.exe',
                taille: TROIS_GO,
                sha256: 'a'.repeat(64),
                chunkSize: 8 * 1024 * 1024,
            },
            MS,
        );
        await sceller(p, tel.id, MS + 1);
        return { vm, tel: tel.id };
    }

    it('is born pending, advances, then finishes', async () => {
        base = await baseNeuve('inst-cycle');
        const { vm, tel } = await withUpload(base);
        const inst = await createInstallation(base, { vmId: vm, televersementId: tel }, MS);
        expect(inst.etat).toBe('en_attente');
        expect((await lireEnAttentePourVm(base, vm)).map((l) => l.id)).toEqual([inst.id]);

        await avancer(
            base,
            inst.id,
            { phase: 'transfert', octetsFaits: 8_388_608, octetsTotal: TROIS_GO, ecouleMs: 1_200 },
            MS + 2_000,
        );
        const enCours = await lireInstallation(base, inst.id);
        expect(enCours?.etat).toBe('en_cours');
        expect(enCours?.octets_total).toBe(TROIS_GO);
        // 🔴 AND THE RE-EMISSION STOPS: it is the FIRST of the two belts
        // against a double execution.
        expect(await lireEnAttentePourVm(base, vm)).toEqual([]);

        await terminer(
            base,
            inst.id,
            { issue: 'reussie', motif: null, codeSortie: 3010, journal: '', journalTronque: false },
            MS + 9_000,
        );
        const fini = await lireInstallation(base, inst.id);
        expect(fini?.etat).toBe('terminee');
        expect(fini?.issue).toBe('reussie');
        // ⚠️ 3010 IS A SUCCESS THAT ASKS FOR A REBOOT, and the database
        // REPORTS it next to the outcome without deducing anything from it.
        expect(fini?.code_sortie).toBe(3010);
        expect(fini?.terminee_a).toBe(MS + 9_000);
    });

    // 🔴 THE RED THAT MATTERS FOR RE-EMISSION. The platform RE-EMITS, so an
    // agent can report twice — and a late progress arriving
    // after the outcome would erase it AND set the state back to `en_cours`,
    // that is, out of `terminee`. The guard is `AND etat <> 'terminee'`
    // on BOTH writes; without it, this test sees the outcome disappear.
    it('🔴 a LATE progress does not erase an outcome already set', async () => {
        base = await baseNeuve('inst-tardive');
        const { vm, tel } = await withUpload(base);
        const inst = await createInstallation(base, { vmId: vm, televersementId: tel }, MS);
        await terminer(
            base,
            inst.id,
            { issue: 'sans-effet', motif: null, codeSortie: 0, journal: '', journalTronque: false },
            MS + 1_000,
        );
        await avancer(
            base,
            inst.id,
            { phase: 'execution', octetsFaits: 0, octetsTotal: 0, ecouleMs: 60_000 },
            MS + 2_000,
        );
        const relu = await lireInstallation(base, inst.id);
        expect(relu?.etat).toBe('terminee');
        expect(relu?.issue).toBe('sans-effet');
        expect(relu?.phase).toBe('');
    });

    it('an exit code NOT COLLECTED stays null, never a sentinel', async () => {
        base = await baseNeuve('inst-sans-code');
        const { vm, tel } = await withUpload(base);
        const inst = await createInstallation(base, { vmId: vm, televersementId: tel }, MS);
        await terminer(
            base,
            inst.id,
            {
                issue: 'issue-inconnue',
                motif: null,
                codeSortie: null,
                journal: 'last line',
                journalTronque: true,
            },
            MS + 1,
        );
        const relu = await lireInstallation(base, inst.id);
        expect(relu?.code_sortie).toBeNull();
        expect(relu?.journal_tronque).toBeTruthy();
    });

    // 🔴 THE FOREIGN KEY RED, SECOND HALF — and it is the one the
    // plan names: "the insertion of an installation for a nonexistent VM
    // PASSES instead of failing".
    it('🔴 REFUSES an installation for a non-existent VM', async () => {
        base = await baseNeuve('inst-vm-fantome');
        const { tel } = await withUpload(base);
        await expect(
            createInstallation(base, { vmId: 'v-fantome', televersementId: tel }, MS),
        ).rejects.toThrow();
    });

    it('🔴 REFUSES an installation for a non-existent upload', async () => {
        base = await baseNeuve('inst-tel-fantome');
        const { vm } = await withUpload(base);
        await expect(
            createInstallation(base, { vmId: vm, televersementId: 't-fantome' }, MS),
        ).rejects.toThrow();
    });

    // ⚠️ THIS REFUSAL IS WANTED, and it is the counterpart of the absence of `ON DELETE`:
    // the history of an installation must stay readable. The CHUNKS on
    // disk, for their part, are swept elsewhere — they are what costs
    // space, not the row.
    it('🔴 REFUSES to delete an upload that an installation references', async () => {
        base = await baseNeuve('inst-fk-refus');
        const { vm, tel } = await withUpload(base);
        await createInstallation(base, { vmId: vm, televersementId: tel }, MS);
        await expect(remove(base, tel)).rejects.toThrow();
    });
});
