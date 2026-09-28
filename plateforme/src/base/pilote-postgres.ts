// The `pg` driver — the production one, and the second half of the double
// pass that tests the portable SQL subset.
//
// `pg` is pure JavaScript: the "no native dependency" property
// (§4.2 of the scoping, written after the wreck of `fuse-native`) holds.
//
// Each `executer`/`interroger` passes its SQL through `rendreMarqueurs` BEFORE
// sending it: the service queries are written only once, in `?`
// style, and it is this driver that translates them. The refusal of string literals
// that `rendreMarqueurs` carries is therefore applied to every query that goes through
// Postgres, not just documented.

import pg from 'pg';
import { type Pilote, rendreMarqueurs } from './pilote';

/// 🔴 `pg` RETURNS `BIGINT`s AS STRINGS, AND THIS IS MEASURED, NOT ASSUMED.
///
/// Found by the acceptance run of sub-block P3, on 19 August 2026: the `typeof` of a
/// reread `vu_a` is `number` under `node:sqlite` and `string` under `pg`. The
/// reason is that the PostgreSQL protocol returns an `int8` as text and that
/// `pg` refuses by default to convert it, since an `int8` can exceed
/// the JavaScript safe integer.
///
/// **Without this line, the defect is one of CLASS and not of instance**:
/// `LigneAgent.vu_a`, `LigneSession.ouverte_a` / `.fermee_a`,
/// `LigneUtilisateur.cree_a` and the two columns of `LigneJeton` all
/// declare as `number` a value that is a `string` on the
/// PRODUCTION engine. The typing does not see it — `interroger<T>` does an `as T[]`,
/// so the assertion is taken at face value.
///
/// ⚠️ WHAT THIS DEFECT DID NOT MAKE FAIL, and why that is the worst case:
/// `agents/fraicheur.ts::etatDe` survived BY ACCIDENT, its subtraction
/// converting the operand. `depot/jeton.ts` had got away with it through a local
/// `Number(...)` and a `number | string` type. Nothing went red, and
/// yet every `+`, every `===` and every `>` would have diverged by engine.
///
/// The conversion THROWS beyond `Number.MAX_SAFE_INTEGER` rather than
/// silently rounding: `Number('9007199254740993')` returns
/// `9007199254740992` without saying so, and a second lost on a timestamp
/// would be exactly the kind of fault no test would catch. The
/// service only writes `Date.now()` values (~1.8e12, that is four orders of
/// magnitude below the bound): this path is not reachable by it, and it
/// is guarded anyway.
///
/// ⚠️ `setTypeParser` is GLOBAL TO THE PROCESS, and that is declared: there is
/// no other consumer of `pg` here, and a per-`Pool` setting would
/// get lost for the administration pool that `base/harnais.ts` opens.
pg.types.setTypeParser(pg.types.builtins.INT8, (texte: string) => {
    const valeur = Number(texte);
    if (!Number.isSafeInteger(valeur)) {
        throw new Error(
            `BIGINT hors de l'entier sûr de JavaScript, converti nulle part : ${texte}`,
        );
    }
    return valeur;
});

/// `maxClients` bounds the number of connections that THIS driver keeps open.
///
/// 🔴 IT EXISTS FOR THE TESTS, AND THE DEFAULT IS THAT OF `pg` (ten), which is
/// the right one for a SERVICE: an instance opens exactly ONE driver, for
/// its whole life, and trimming its concurrency would make no sense.
///
/// ⚠️ MEASURED ON 20 AUGUST 2026, AND IT IS NOT A THEORETICAL PRECAUTION: the
/// suite opens ONE HUNDRED AND TWENTY-EIGHT databases (`grep -c 'baseNeuve('`), spread over
/// twenty-two files that vitest runs IN PARALLEL, and each `baseNeuve`
/// opens TWO drivers (one for administration, one for work). At ten clients
/// each, ten concurrent databases reach exactly the
/// `max_connections = 100` of the test instance. The day P5 added its
/// three test files, the instance returned
/// `FATAL: sorry, too many clients already` THEN a backend was
/// `terminated by signal 11: Segmentation fault` in the middle of a migration: the
/// whole suite went back to 181 failures, on perfectly healthy code.
/// The cause was the SUITE, not the service — but a suite that brings down
/// its instance no longer measures anything.
export function ouvrirPostgres(url: string, maxClients?: number): Pilote {
    const pool = new pg.Pool({
        connectionString: url,
        ...(maxClients === undefined ? {} : { max: maxClients }),
    });
    return {
        async executer(sql, params) {
            const r = await pool.query(rendreMarqueurs(sql), params);
            return { lignes: r.rowCount ?? 0 };
        },
        async interroger<T>(sql: string, params: unknown[]): Promise<T[]> {
            const r = await pool.query(rendreMarqueurs(sql), params);
            return r.rows as T[];
        },
        async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
            // 🔴 THE `pg` TRAP, named rather than suffered: a `BEGIN` issued on
            // the POOL and a `COMMIT` issued afterwards on the pool would take two
            // DIFFERENT clients, hence two different transactions — and all of it
            // SILENTLY, with no error, the first one staying open
            // until it times out. The client is therefore taken once and kept
            // for the whole duration of the body.
            const client = await pool.connect();
            try {
                await client.query('BEGIN');
                const valeur = await corps(surClient(client));
                await client.query('COMMIT');
                return valeur;
            } catch (cause) {
                await client.query('ROLLBACK');
                throw cause;
            } finally {
                client.release();
            }
        },
        async fermer() {
            await pool.end();
        },
    };
}

/// A `Pilote` restricted to the client already borrowed: that is what guarantees that
/// the whole body of a transaction really talks to the SAME connection.
function surClient(client: pg.PoolClient): Pilote {
    return {
        async executer(sql, params) {
            const r = await client.query(rendreMarqueurs(sql), params);
            return { lignes: r.rowCount ?? 0 };
        },
        async interroger<T>(sql: string, params: unknown[]): Promise<T[]> {
            const r = await client.query(rendreMarqueurs(sql), params);
            return r.rows as T[];
        },
        async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
            // No nested transaction in P1: `SAVEPOINT` would be one more
            // mechanism to test on both sides, and nothing
            // uses it. Explicit refusal rather than a nested `BEGIN` that
            // Postgres would accept with a warning, and SQLite by failing.
            throw new Error('transaction imbriquée non prise en charge');
        },
        async fermer() {
            throw new Error('un pilote de transaction ne se ferme pas : il est relâché');
        },
    };
}
