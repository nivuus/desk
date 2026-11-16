// Migration 0008, played on a POPULATED database — against BOTH engines.
//
// 🔴 A FRESH DATABASE PROVES NOTHING ABOUT A MIGRATION: `baseNeuve` applies all
// files at once on empty tables, where an `ADD COLUMN` that would be refused on
// a table carrying rows passes. Production carries rows. This file therefore
// stops at 0007, writes the rows a running service has (a user, a VM, a sealed
// upload, a finished installation), THEN applies the real directory.

import { copyFileSync, mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, INSTANT_MIGRATION, MOTEUR } from './harnais';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from './migrations';
import type { Pilote } from './pilote';

let base: Pilote | undefined;
let avant: string | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
    if (avant !== undefined) rmSync(avant, { recursive: true, force: true });
    avant = undefined;
});

const MS = 1_787_136_773_742;

/// A copy of the migrations directory holding every file BELOW `version`.
function migrationsAvant(version: number): string {
    const dir = mkdtempSync(join(tmpdir(), 'migrations-avant-'));
    for (const f of readdirSync(REPERTOIRE_MIGRATIONS)) {
        const tete = /^(\d+)/.exec(f);
        if (tete !== null && Number(tete[1]) < version) {
            copyFileSync(join(REPERTOIRE_MIGRATIONS, f), join(dir, f));
        }
    }
    return dir;
}

describe(`migration 0008 on a populated database, engine=${MOTEUR}`, () => {
    it('adds `contenu_purge_a` as NULL to existing uploads and keeps every row', async () => {
        avant = migrationsAvant(8);
        base = await baseNeuve('migration-0008-peuplee', avant);
        await base.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)', [
            'u1', 'u1@exemple.test', 'scrypt$1$1$1$x$y', MS,
        ]);
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v1', 'vm-1', '192.168.3.2']);
        await base.executer(
            'INSERT INTO televersement(id,utilisateur_id,nom,taille,sha256,taille_tranche,cree_a,scelle_a)'
                + ' VALUES(?,?,?,?,?,?,?,?)',
            ['t1', 'u1', 'setup.exe', 4_000_000_000, 'a'.repeat(64), 8_388_608, MS, MS + 1],
        );
        await base.executer(
            'INSERT INTO installation(id,vm_id,televersement_id,demandee_a,etat,phase,octets_faits,'
                + 'octets_total,ecoule_ms,code_sortie,issue,motif,journal,journal_tronque,terminee_a,maj_a)'
                + ' VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)',
            ['i1', 'v1', 't1', MS, 'terminee', 'execution', 0, 0, 10, 0, 'reussie', null, '', 0, MS + 2, MS + 2],
        );

        expect(await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, INSTANT_MIGRATION + 1)).toBe(1);

        const lignes = await base.interroger<{ id: string; taille: number; contenu_purge_a: number | null }>(
            'SELECT id, taille, contenu_purge_a FROM televersement',
            [],
        );
        expect(lignes).toEqual([{ id: 't1', taille: 4_000_000_000, contenu_purge_a: null }]);
        // The column takes an epoch in milliseconds on both engines (BIGINT).
        await base.executer('UPDATE televersement SET contenu_purge_a = ? WHERE id = ?', [MS, 't1']);
        const relue = await base.interroger<{ contenu_purge_a: number }>(
            'SELECT contenu_purge_a FROM televersement WHERE id = ?',
            ['t1'],
        );
        expect(relue[0]?.contenu_purge_a).toBe(MS);
        const inst = await base.interroger<{ televersement_id: string }>(
            'SELECT televersement_id FROM installation WHERE id = ?',
            ['i1'],
        );
        expect(inst).toEqual([{ televersement_id: 't1' }]);
    });
});
