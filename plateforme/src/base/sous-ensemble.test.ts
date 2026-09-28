// The STATIC LINT of the portable SQL subset.
//
// 🔴 It is NOT redundant with the double execution pass, and the fact is
// MEASURED. On 19 August 2026, on SQLite 3.50.4:
//     AUTOINCREMENT sqlite : ACCEPTE
//     SERIAL sqlite        : ACCEPTE (type libre)
// SQLite accepts any type name by affinity. `SERIAL` therefore passes
// there without noise — and it also passes on Postgres, where it MEANS SOMETHING
// ELSE. Two green passes, two different schemas: the execution test
// cannot catch `SERIAL`. Only this lint can.
//
// Conversely, this lint can do nothing against a construct syntactically
// legal on both sides but with diverging semantics — that is the role of the
// double pass. Each covers the other's blind spot.

import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const REPERTOIRE = path.join(path.dirname(fileURLToPath(import.meta.url)), 'migrations');

const INTERDITS: Array<[RegExp, string]> = [
    [/\bSERIAL\b/i, 'SERIAL: accepted by affinity on SQLite, something else on Postgres'],
    [/\bAUTOINCREMENT\b/i, 'AUTOINCREMENT: specific to SQLite'],
    [/\bdatetime\s*\(/i, 'datetime(): timestamps are written by the application'],
    [/\bnow\s*\(/i, 'now(): timestamps are written by the application'],
    [/\bCURRENT_TIMESTAMP\b/i, 'CURRENT_TIMESTAMP: likewise'],
    [/\bUUID\b/i, 'UUID: Postgres type, absent from SQLite — identifiers are TEXT'],
    [/\bTIMESTAMPTZ\b/i, 'TIMESTAMPTZ : type Postgres'],
    [/\bJSONB\b/i, 'JSONB : type Postgres'],
    [/\bBOOLEAN\b/i, 'BOOLEAN: booleans are INTEGER 0/1'],
    // 🔴 MEASURED, not precautionary. `INTEGER` is up to 8 bytes on SQLite and
    // exactly 4 on Postgres: on 19 August 2026, on PostgreSQL 16.15, a
    // `Date.now()` in an INTEGER column returned
    //     value "1787136773742" is out of range for type integer
    // and the service could not apply its OWN migrations, while
    // SQLite accepted it without a word.
    //
    // What this pattern assumes, and one must know it: the NAMING CONVENTION
    // `_a` for a timestamp (cree_a, vue_a, ouverte_a, fermee_a,
    // applique_a). A timestamp column named otherwise would escape this
    // lint — it therefore does not replace the magnitude test of
    // `pilotes.test.ts`, it is its lexical counterpart.
    [
        /\b\w+_a\s+INTEGER\b/i,
        'an `_a` timestamp as INTEGER: 4 bytes on Postgres, where a Date.now() overflows — BIGINT',
    ],
];

/// Strips `--` comments BEFORE any check.
///
/// ⚠️ Without this stripping, the lint would be red on ITS OWN DOCUMENTATION: the
/// SQL comments are in French and carry apostrophes, which the engine
/// never sees. They also name the forbidden tokens to explain
/// why they are forbidden.
function corps(texte: string): string {
    return texte.replace(/--.*$/gm, '');
}

const files = readdirSync(REPERTOIRE).filter((f) => f.endsWith('.sql')).sort();

describe('portable SQL subset', () => {
    it('reads at least one migration file', () => {
        // ⚠️ Without this assertion, a lint that reads NO file would pass
        // trivially — and would be green the day the directory was
        // renamed. It is the pattern of the "check that cannot fail",
        // which this repository has paid for four times.
        expect(files.length).toBeGreaterThan(0);
    });

    for (const file of files) {
        const sql = corps(readFileSync(path.join(REPERTOIRE, file), 'utf8'));

        it(`${file} uses no token outside the subset`, () => {
            const trouves = INTERDITS.filter(([motif]) => motif.test(sql)).map(([, raison]) => raison);
            // Compared as a STRING and not as an array: vitest truncates
            // an array to `[ Array(1) ]`, a message that does not name the offending
            // token — a diagnostic that helps whoever reads it in no way.
            expect(trouves.join(' | ')).toBe('');
        });

        it(`${file} carries no string literal`, () => {
            // Constraint of `rendreMarqueurs`: every value goes as a
            // parameter, not even a literal DEFAULT. An apostrophe here
            // would make the marker conversion throw on the Postgres side.
            expect(sql).not.toMatch(/['"]/);
        });
    }
});

describe('the duplicated definition of schema_migration', () => {
    // ⚠️ `migrations.ts` recreates `schema_migration` hardcoded, because the
    // first migration needs the table to record itself. The
    // comment of that file claims the two definitions are
    // "identical" — a claim NOTHING checked, and which would have
    // diverged silently: a database created by the hardcoded line would no longer
    // have looked like the schema the base describes.
    const source = readFileSync(
        path.join(path.dirname(fileURLToPath(import.meta.url)), 'migrations.ts'),
        'utf8',
    );
    const socle = readFileSync(path.join(REPERTOIRE, '0001-socle.sql'), 'utf8');

    /// Reduces a table definition to `column type` separated by
    /// commas, to compare the STRUCTURE and not the layout.
    function columns(ddl: string): string {
        const corps = /schema_migration\s*\(([^)]*)\)/i.exec(ddl);
        if (!corps) throw new Error(`no definition of schema_migration in this text`);
        return corps[1]
            .split(',')
            .map((c) => c.replace(/\s+/g, ' ').trim().toUpperCase())
            .join(', ');
    }

    it('is the same in migrations.ts and in 0001-socle.sql', () => {
        expect(columns(source)).toBe(columns(corps(socle)));
    });

    it('does carry the timestamp as BIGINT', () => {
        // Without this second assertion, two WRONG and identical definitions
        // would pass the first — it is the check that cannot fail.
        expect(columns(source)).toContain('APPLIQUE_A BIGINT');
    });
});
