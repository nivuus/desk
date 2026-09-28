// The harness of the double pass: it opens the driver that `PLATEFORME_BASE`
// designates, on a FRESH database, and applies the migrations.
//
// 🔴 A SKIP IS A FAILURE (spec §7.1). If `PLATEFORME_BASE=postgres` and
// the instance is unreachable, the tests FAIL with the cause. There is here
// no `it.skipIf`, no `describe.skip`, no `if (!disponible) return`. This repository has
// paid several times for a check that could not fail; a test that
// VANISHES when its dependency is missing is the same mistake in another
// form — it turns green a state it has not measured.

import type { Pilote } from './pilote';
import { ouvrirPostgres } from './pilote-postgres';
import { ouvrirSqlite } from './pilote-sqlite';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from './migrations';

export const MOTEUR = (process.env.PLATEFORME_BASE ?? 'sqlite') as 'sqlite' | 'postgres';

/// URL of the test instance, the one of `docker-compose.plateforme.yml`.
/// Overridable through `PLATEFORME_BASE_URL` to target another instance.
const URL_POSTGRES =
    process.env.PLATEFORME_BASE_URL ??
    'postgres://plateforme:plateforme-test@127.0.0.1:5433/plateforme_test';

/// The instant at which the test migrations are applied.
///
/// 🔴 It is a value of the MAGNITUDE OF AN EPOCH IN MILLISECONDS, and not
/// a convenient small number, because a small number measures nothing.
/// `1_000` fit in a 4-byte integer; `Date.now()` does not. The
/// production service writes only `Date.now()` values: a suite that writes
/// only `1_000` declares portable a schema that refuses any real write
/// on one of the two engines, without any test going red.
export const INSTANT_MIGRATION = 1_700_000_000_000;

/// The number of connections that ONE test database keeps open.
///
/// 🔴 WITHOUT THIS BOUND, THE SUITE BRINGS ITS INSTANCE DOWN — measured on 20 August
/// 2026. The `pg` default is TEN clients per driver; the suite opens
/// one hundred and twenty-eight databases over twenty-two files that vitest runs IN
/// PARALLEL, and each `baseNeuve` opens TWO drivers. Ten concurrent databases
/// are then enough to reach the `max_connections = 100` of the instance:
/// it returned `FATAL: sorry, too many clients already`, then a backend was
/// `terminated by signal 11: Segmentation fault` in the middle of a migration, and
/// the suite went back to 181 failures on perfectly healthy code.
///
/// ⚠️ TWO, AND NOT ONE: `transaction` TAKES a client for the whole duration of
/// its body (see the trap named in `pilote-postgres.ts`), and a query
/// issued on the pool meanwhile would wait forever for a second
/// client if the pool had only one. ⚠️ UNCALIBRATED beyond this
/// reasoning: no measurement has looked for the optimal value.
const MAX_CLIENTS_TEST = 2;

/// Opens a BLANK database and applies the migrations to it.
///
/// SQLite: an in-memory database, hence fresh by construction.
/// Postgres: a throwaway SCHEMA, specific to the call, put at the head of the
/// `search_path`. That is what lets several test files run
/// side by side without stepping on each other, where a global `DROP SCHEMA public`
/// would make them destroy one another.
export async function baseNeuve(nom: string): Promise<Pilote> {
    if (MOTEUR === 'sqlite') {
        const p = ouvrirSqlite(':memory:');
        await appliquerMigrations(p, REPERTOIRE_MIGRATIONS, INSTANT_MIGRATION);
        return p;
    }

    const schema = `t_${nom.replace(/[^a-z0-9]/gi, '_')}_${process.pid}_${compteur()}`;
    // The schema is interpolated, never parameterised: an SQL identifier cannot
    // be a query parameter, on any engine. It is built
    // here, from values that only this file provides, and filtered on
    // [a-z0-9_] — no outside input reaches it.
    const admin = ouvrirPostgres(URL_POSTGRES, MAX_CLIENTS_TEST);
    await admin.executer(`DROP SCHEMA IF EXISTS ${schema} CASCADE`, []);
    await admin.executer(`CREATE SCHEMA ${schema}`, []);
    await admin.fermer();

    const p = ouvrirPostgres(
        `${URL_POSTGRES}?options=-c%20search_path%3D${schema}`,
        MAX_CLIENTS_TEST,
    );
    await appliquerMigrations(p, REPERTOIRE_MIGRATIONS, INSTANT_MIGRATION);
    return p;
}

let n = 0;
function compteur(): number {
    return ++n;
}

/// A DECORATOR around a real driver, which COUNTS the database accesses.
///
/// 🔴 IT IS NOT A FAKE, AND THAT IS THE POINT. A dummy driver would measure
/// something other than production; this one delegates EVERYTHING, and adds only a
/// counter. That is what makes the assertion "the throttled refusal does not
/// touch the database" decidable — hence "it derives no `scrypt`" —, where
/// measuring it in TIME would be unstable and where measuring it through a fake would say
/// nothing about the real path.
///
/// ⚠️ IT LIVES HERE RATHER THAN IN A TEST FILE because TWO files
/// use it — `http/routes-auth.test.ts` and `agents/canal.test.ts` — and
/// two copies would diverge at the first fix made to only one of them.
/// That is the exact reason why `agents/canal-harnais.ts` was
/// extracted in sub-block G1.
export function piloteCompteur(reel: Pilote): {
    pilote: Pilote;
    acces: () => number;
    remettre: () => void;
} {
    let n = 0;
    const pilote: Pilote = {
        async executer(sql, params) {
            n += 1;
            return reel.executer(sql, params);
        },
        async interroger<T>(sql: string, params: unknown[]): Promise<T[]> {
            n += 1;
            return reel.interroger<T>(sql, params);
        },
        transaction(corps) {
            return reel.transaction(corps);
        },
        fermer() {
            return reel.fermer();
        },
    };
    return { pilote, acces: () => n, remettre: () => { n = 0; } };
}
