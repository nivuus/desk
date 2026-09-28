import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { rendreMarqueurs } from './pilote';

describe('rendreMarqueurs', () => {
    it('numbers the placeholders in order', () => {
        expect(rendreMarqueurs('INSERT INTO t(a,b) VALUES(?,?)'))
            .toBe('INSERT INTO t(a,b) VALUES($1,$2)');
    });

    it('leaves a SQL without placeholder intact', () => {
        expect(rendreMarqueurs('SELECT 1')).toBe('SELECT 1');
    });

    it('REFUSES a SQL carrying a string literal', () => {
        // `SELECT '?' AS x` is VALID SQL (measured on SQLite 3.50.4): a
        // naive conversion would change its meaning without saying anything. The refusal is
        // what makes the rule "every value goes as a parameter" mechanical.
        expect(() => rendreMarqueurs("SELECT '?' AS x")).toThrow(/literal/);
        expect(() => rendreMarqueurs("SELECT * FROM t WHERE a = 'x'")).toThrow(/literal/);
    });

    it('also refuses the double quote, which hides a delimited identifier', () => {
        expect(() => rendreMarqueurs('SELECT "a b" FROM t')).toThrow(/literal/);
    });
});

describe('package discipline', () => {
    const pkg = readFileSync(new URL('../../package.json', import.meta.url), 'utf8');

    it("masks the experimental warning of node:sqlite nowhere", () => {
        // Spec §3.2 accepts `node:sqlite` BECAUSE "the failure of an
        // API change is loud and immediate": hiding the warning
        // would remove exactly the noise that justifies the decision.
        expect(pkg).not.toMatch(/NODE_NO_WARNINGS|--no-warnings|--disable-warning/);
    });

    it("adds no production dependency outside the allow-list", () => {
        // Principle §4.2 of the framing — "zero unmaintained native dependency",
        // written after the wreck of `fuse-native`. `ws` and `pg` are pure
        // JavaScript; `node:sqlite` is built into Node.
        //
        // ⚠️ This check is on the DECLARED `dependencies`, never on a
        // sweep of `node_modules`: `rollup` — a TRANSITIVE dependency of
        // vitest, hence a dev one — already installs a `.node`. A
        // `find node_modules -name '*.node'` would be red from the start, for a
        // wrong reason.
        const deps = Object.keys(JSON.parse(pkg).dependencies).sort();
        expect(deps).toEqual(['pg', 'ws']);
    });
});
