// The migration runner: transactional, NUMERICALLY ordered, idempotent.
//
// Spec §6: a failing migration rolls back its transaction, stops the service and
// leaves the version unchanged. Never a half-applied schema — a partial
// schema would be indistinguishable from a correct one at the next startup, and
// that is the class of silent failure this whole repository is written against.

import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import type { Pilote } from './pilote';

interface Migration {
    version: number;
    fichier: string;
    sql: string;
}

/// 🔴 The sort is NUMERIC, never lexicographic.
///
/// A string sort puts `0010` before `0002`: the tenth migration
/// would apply before the second, and the schema would be wrong without any
/// error saying so — each file would still be run, and
/// `schema_migration` properly filled in. It is a silent failure deferred by nine
/// migrations, and that is why the sort is explicit here.
function lire(repertoire: string): Migration[] {
    return readdirSync(repertoire)
        .filter((f) => f.endsWith('.sql'))
        .map((fichier) => {
            const tete = /^(\d+)/.exec(fichier);
            if (!tete) {
                throw new Error(
                    `migration sans numéro de version en tête : ${fichier} — le nom doit commencer par des chiffres`,
                );
            }
            return {
                version: Number(tete[1]),
                fichier,
                sql: readFileSync(path.join(repertoire, fichier), 'utf8'),
            };
        })
        .sort((a, b) => a.version - b.version);
}

/// Splits a file into statements.
///
/// The `--` comments are removed first: they carry French
/// apostrophes, and an empty statement would make some engines fail.
function instructions(sql: string): string[] {
    return sql
        .replace(/--.*$/gm, '')
        .split(';')
        .map((s) => s.trim())
        .filter((s) => s.length > 0);
}

/// Applies the migrations not yet applied and returns their COUNT.
///
/// `maintenant` is a parameter, never `Date.now()` read here: that is the rule
/// of the subset (timestamps are always written by the application),
/// and it is what makes the result checkable against an exact value.
export async function appliquerMigrations(
    p: Pilote,
    repertoire: string,
    maintenant: number,
): Promise<number> {
    // The tracking table must exist before it can be read. `IF NOT
    // EXISTS` makes it idempotent; its definition is repeated here and in
    // `0001-socle.sql`, identically, because neither of the two can
    // lean on the other: the first migration needs the table to
    // record itself.
    //
    // ⚠️ This duplication is DELIBERATE, so it can diverge — and a
    // TYPE divergence would be silent: the database created by this line would
    // no longer look like the one the base schema describes, without any error
    // saying so. `sous-ensemble.test.ts` compares the two definitions; do not
    // touch one without the other.
    //
    // `applique_a` is BIGINT and not INTEGER: see the header of
    // `0001-socle.sql` -- on Postgres, INTEGER is 4 bytes and a
    // `Date.now()` does not fit.
    await p.executer(
        'CREATE TABLE IF NOT EXISTS schema_migration (version INTEGER PRIMARY KEY, applique_a BIGINT NOT NULL)',
        [],
    );

    const deja = await p.interroger<{ version: number }>(
        'SELECT version FROM schema_migration',
        [],
    );
    const appliquees = new Set(deja.map((l) => Number(l.version)));

    let compte = 0;
    for (const migration of lire(repertoire)) {
        if (appliquees.has(migration.version)) continue;
        await p.transaction(async (tx) => {
            for (const instruction of instructions(migration.sql)) {
                // `schema_migration` is already created above: a second
                // creation in the same transaction would fail. The base schema
                // declares it for a reader of the schema, not for execution.
                if (/^CREATE\s+TABLE\s+schema_migration\b/i.test(instruction)) continue;
                await tx.executer(instruction, []);
            }
            await tx.executer(
                'INSERT INTO schema_migration(version, applique_a) VALUES(?, ?)',
                [migration.version, maintenant],
            );
        });
        compte += 1;
    }
    return compte;
}

/// The migrations directory, resolved from this module — so that no
/// caller has to know the tree layout.
export const REPERTOIRE_MIGRATIONS = path.join(
    path.dirname(new URL(import.meta.url).pathname),
    'migrations',
);
