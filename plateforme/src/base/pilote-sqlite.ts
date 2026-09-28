// The `node:sqlite` driver — the one for development and for the test suite.
//
// 🔴 The `ExperimentalWarning` of `node:sqlite` IS NOT MASKED, anywhere:
// no `NODE_NO_WARNINGS`, no `--no-warnings`, no `--disable-warning`, neither in
// `package.json`, nor in the vitest configuration, nor in `verify-all.sh`.
// Spec §3.2 accepts this module BECAUSE "the failure of an API change is
// loud and immediate": silencing it would remove exactly the noise that
// justifies the decision. What makes the output readable is not
// suppression, it is RARITY — `node:sqlite` is imported ONLY by this
// file, so the warning shows up once per process, on two
// lines. A check of `pilote.test.ts` forbids masking it later.
//
// ⚠️ `DatabaseSync` is SYNCHRONOUS; the `Pilote` interface is asynchronous. The
// methods below therefore return ALREADY RESOLVED promises. There is here
// no concurrency, no parallelism, no real waiting: a
// successor who believed they could fire two queries side by side on this driver
// would be wrong. The asynchrony is that of the interface, not that of the engine.

import { createRequire } from 'node:module';
import type { DatabaseSync as TypeDatabaseSync } from 'node:sqlite';
import type { Pilote } from './pilote';

// 🔴 `node:sqlite` is loaded through `createRequire`, and NEVER through a static
// `import`. The cause is MEASURED, not guessed (19 August 2026, vitest 2.1.9,
// vite 5.4.21, Node v24.9.0):
//
//     node --input-type=module -e "
//       import { isNodeBuiltin } from './node_modules/vite-node/dist/utils.mjs';
//       console.log(isNodeBuiltin('node:sqlite'), isNodeBuiltin('node:http'));"
//     -> false true
//
// `vite-node/dist/utils.mjs` — the VITEST resolver, distinct from that of
// vite, whose `isNodeBuiltin` nonetheless accepts any `node:*` — strips the
// `node:` prefix then looks for `sqlite` in its own list of builtins, which
// does not contain it. It concludes that it is an npm package and fails on
// "Failed to load url sqlite (resolved id: sqlite). Does the file exist?".
// `test.server.deps.external` changes nothing: the failure happens at
// RESOLUTION, before any externalisation decision.
//
// ⚠️ This masks NOTHING: the module loaded is the real one, and
// the `ExperimentalWarning` still shows up. The day vite-node knows about
// `sqlite`, the static `import` will become possible again — and this detour can
// be undone.
const { DatabaseSync } = createRequire(import.meta.url)('node:sqlite') as {
    DatabaseSync: typeof TypeDatabaseSync;
};

export function ouvrirSqlite(cheminOuMemoire: string): Pilote {
    const base = new DatabaseSync(cheminOuMemoire);
    // Without this pragma, SQLite DOES NOT ENFORCE foreign keys: they are
    // accepted at declaration and ignored at execution. The portability of the
    // schema would then be tested on one side only — Postgres enforces them,
    // for its part, without anything to ask.
    base.exec('PRAGMA foreign_keys = ON');
    return pilote(base);
}

function pilote(base: TypeDatabaseSync): Pilote {
    return {
        async executer(sql, params) {
            const r = base.prepare(sql).run(...(params as never[]));
            return { lignes: Number(r.changes) };
        },
        async interroger<T>(sql: string, params: unknown[]): Promise<T[]> {
            return base.prepare(sql).all(...(params as never[])) as T[];
        },
        async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
            base.exec('BEGIN');
            try {
                const value = await corps(pilote(base));
                base.exec('COMMIT');
                return value;
            } catch (cause) {
                base.exec('ROLLBACK');
                throw cause;
            }
        },
        async fermer() {
            base.close();
        },
    };
}
