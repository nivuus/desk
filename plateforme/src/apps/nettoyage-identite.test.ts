// The cleanup round also retires dead identity rows — both engines.
//
// 🔴 THE DEFECT: every refresh-token rotation inserts a `jeton_rafraichissement`
// row and every session a `session` row; rows were revoked or closed, never
// deleted. Growth bounded by use, never by a retention.
//
// ⚠️ A TOKEN ROW IS NOT PURGED ON ITS OWN, ITS FAMILY IS. A rotated row is what
// lets `tourner` recognise a REPLAY and revoke the family, the thief's live
// token included. Deleting it while the family still has a live member would
// turn a detected replay into an unknown token. A family goes only once its
// NEWEST token has been expired for the whole retention: nothing of it can be
// used any more.

import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { ouvrirMagasin } from './icones';
import { ouvrirMagasinTranches } from './magasin-tranches';
import { RETENTION_IDENTITE_MS, unTour } from './nettoyage';

let base: Pilote | undefined;
let racine: string | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
    if (racine !== undefined) rmSync(racine, { recursive: true, force: true });
    racine = undefined;
});

const MS = 1_787_136_773_742;
const JOUR_MS = 24 * 60 * 60_000;

async function jeton(
    p: Pilote, id: string, famille: string, expireA: number, revoqueA: number | null,
): Promise<void> {
    await p.executer(
        'INSERT INTO jeton_rafraichissement(id,utilisateur_id,famille,empreinte,remplace_par,cree_a,expire_a,revoque_a)'
            + ' VALUES(?,?,?,?,?,?,?,?)',
        [id, 'u1', famille, `empreinte-${id}`, null, expireA - 30 * JOUR_MS, expireA, revoqueA],
    );
}

async function session(p: Pilote, id: string, fermeeA: number | null): Promise<void> {
    await p.executer(
        'INSERT INTO session(id,nom_session,ouverte_a,fermee_a,motif) VALUES(?,?,?,?,?)',
        [id, `s-${id}`, MS - 500 * JOUR_MS, fermeeA, fermeeA === null ? null : 'fin'],
    );
}

async function ids(p: Pilote, table: string): Promise<string[]> {
    const lignes = await p.interroger<{ id: string }>(`SELECT id FROM ${table} ORDER BY id`, []);
    return lignes.map((l) => l.id);
}

describe(`retention of identity rows, engine=${MOTEUR}`, () => {
    it('🔴 deletes dead token families and old closed sessions, keeps everything still meaningful', async () => {
        base = await baseNeuve('retention-identite');
        await base.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)', [
            'u1', 'u1@exemple.test', 'scrypt$1$1$1$x$y', MS,
        ]);
        const longtemps = MS - RETENTION_IDENTITE_MS - JOUR_MS;

        // Family A: every token expired long ago — it goes, whole.
        await jeton(base, 'a1', 'fa', longtemps - JOUR_MS, longtemps - JOUR_MS);
        await jeton(base, 'a2', 'fa', longtemps, null);
        // Family B: an OLD rotated row, and a LIVE successor — it stays, whole.
        await jeton(base, 'b1', 'fb', longtemps, longtemps - 29 * JOUR_MS);
        await jeton(base, 'b2', 'fb', MS + 10 * JOUR_MS, null);
        // Family C: expired, but more recently than the retention — it stays.
        await jeton(base, 'c1', 'fc', MS - JOUR_MS, null);

        await session(base, 's-vieille', longtemps);
        await session(base, 's-recente', MS - JOUR_MS);
        await session(base, 's-ouverte', null);

        racine = mkdtempSync(join(tmpdir(), 'retention-'));
        await unTour({
            base,
            magasin: ouvrirMagasin(join(racine, 'i'), () => {}),
            tranches: ouvrirMagasinTranches(join(racine, 't'), () => {}),
            maintenant: MS,
        });

        expect(await ids(base, 'jeton_rafraichissement')).toEqual(['b1', 'b2', 'c1']);
        expect(await ids(base, 'session')).toEqual(['s-ouverte', 's-recente']);
    });
});
