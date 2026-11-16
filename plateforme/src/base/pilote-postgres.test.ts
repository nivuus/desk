// The `pg` driver survives the loss of an idle connection.
//
// 🔴 THE DEFECT: `pg.Pool` emits `error` when an IDLE client loses its backend —
// a restart of Postgres, an image update, an administrator's
// `pg_terminate_backend`. With no listener, Node turns that event into an
// uncaught exception and the whole platform process dies, every session with it.
// The pool already discards the dead client and opens a new one on the next
// query: the only thing missing was someone to hear the event.
//
// ⚠️ ONLY POSTGRES HAS A POOL. Under SQLite the database is a file in the same
// process and there is no connection to lose; that pass checks the same
// promise — a query still answers after the round — and says it measures
// nothing more, rather than vanishing.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, MOTEUR, URL_POSTGRES } from './harnais';
import type { Pilote } from './pilote';
import { ouvrirPostgres } from './pilote-postgres';

let ouverts: Pilote[] = [];

afterEach(async () => {
    for (const p of ouverts) await p.fermer();
    ouverts = [];
    vi.restoreAllMocks();
});

describe(`loss of an idle connection, engine=${MOTEUR}`, () => {
    it('🔴 is logged, does not kill the process, and the next query reconnects', async () => {
        if (MOTEUR === 'sqlite') {
            const p = await baseNeuve('perte-connexion-sqlite');
            ouverts.push(p);
            expect(await p.interroger('SELECT 1 AS un', [])).toEqual([{ un: 1 }]);
            return;
        }

        const nom = `perte-idle-${process.pid}`;
        const separateur = URL_POSTGRES.includes('?') ? '&' : '?';
        const victime = ouvrirPostgres(`${URL_POSTGRES}${separateur}application_name=${nom}`, 1);
        const admin = ouvrirPostgres(URL_POSTGRES, 1);
        ouverts.push(victime, admin);
        const erreurs = vi.spyOn(console, 'error').mockImplementation(() => {});

        // One query leaves one IDLE client in the pool.
        expect(await victime.interroger('SELECT 1 AS un', [])).toEqual([{ un: 1 }]);
        const tues = await admin.interroger<{ ok: boolean }>(
            'SELECT pg_terminate_backend(pid) AS ok FROM pg_stat_activity WHERE application_name = ?',
            [nom],
        );
        expect(tues).toEqual([{ ok: true }]);

        // The pool learns it asynchronously, through the `error` event.
        const fin = Date.now() + 5000;
        while (erreurs.mock.calls.length === 0 && Date.now() < fin) {
            await new Promise((r) => setTimeout(r, 20));
        }
        expect(erreurs).toHaveBeenCalledTimes(1);
        expect(String(erreurs.mock.calls[0]?.[0])).toMatch(/idle/);

        expect(await victime.interroger('SELECT 2 AS deux', [])).toEqual([{ deux: 2 }]);
    });

    it('🔴 a transaction whose ROLLBACK fails still rejects with ITS OWN cause', async () => {
        const cause = new Error('cause of the transaction body');
        if (MOTEUR === 'sqlite') {
            const p = await baseNeuve('rollback-sqlite');
            ouverts.push(p);
            await expect(p.transaction(async () => { throw cause; })).rejects.toBe(cause);
            return;
        }

        const nom = `perte-tx-${process.pid}`;
        const separateur = URL_POSTGRES.includes('?') ? '&' : '?';
        const victime = ouvrirPostgres(`${URL_POSTGRES}${separateur}application_name=${nom}`, 1);
        const admin = ouvrirPostgres(URL_POSTGRES, 1);
        ouverts.push(victime, admin);
        vi.spyOn(console, 'error').mockImplementation(() => {});

        await expect(victime.transaction(async (tx) => {
            await tx.interroger('SELECT 1 AS un', []);
            // The connection dies under the transaction: its ROLLBACK will fail.
            await admin.interroger(
                'SELECT pg_terminate_backend(pid) AS ok FROM pg_stat_activity WHERE application_name = ?',
                [nom],
            );
            throw cause;
        })).rejects.toBe(cause);

        // The dead connection was not handed back: the next query answers.
        expect(await victime.interroger('SELECT 3 AS trois', [])).toEqual([{ trois: 3 }]);
    });
});
