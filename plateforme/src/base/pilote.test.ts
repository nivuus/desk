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
        // `SELECT '?' AS x` est du SQL VALIDE (mesuré sur SQLite 3.50.4) : une
        // conversion naïve en changerait le sens sans rien dire. Le refus est
        // ce qui rend mécanique la règle « toute valeur passe en paramètre ».
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
        // La spec §3.2 accepte `node:sqlite` PARCE QUE « l'échec d'une
        // évolution d'API est bruyant et immédiat » : masquer l'avertissement
        // retirerait exactement le bruit qui justifie la décision.
        expect(pkg).not.toMatch(/NODE_NO_WARNINGS|--no-warnings|--disable-warning/);
    });

    it("adds no production dependency outside the allow-list", () => {
        // Principe §4.2 du cadrage — « zéro dépendance native non maintenue »,
        // écrit après le naufrage de `fuse-native`. `ws` et `pg` sont purement
        // JavaScript ; `node:sqlite` est intégré à Node.
        //
        // ⚠️ Ce contrôle porte sur les `dependencies` DÉCLARÉES, jamais sur un
        // balayage de `node_modules` : `rollup` — dépendance TRANSITIVE de
        // vitest, donc de développement — installe déjà un `.node`. Un
        // `find node_modules -name '*.node'` serait rouge d'emblée, pour une
        // mauvaise raison.
        const deps = Object.keys(JSON.parse(pkg).dependencies).sort();
        expect(deps).toEqual(['pg', 'ws']);
    });
});
