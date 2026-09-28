// The catalogue merge, tested WITHOUT A DATABASE AND WITHOUT A CLOCK.
//
// 🔴 It is what the module buys: the three rules that decide what gets
// written — mark vanished, resurrect, update — turn red here,
// on an array and an object, without opening a database or mounting a server. A
// rule living in the repository layer or in the channel could only be tested
// by a test that crosses an SQL engine.

import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import type { Application, CatalogueMessage } from '../../../proto/ts/plateforme';
import { fusionner, type Connue } from './catalogue';

function app(cle: string, nom = cle): Application {
    return {
        cle,
        nom,
        chemin: `C:\\Bureau\\${nom}.lnk`,
        cible: `c:\\programmes\\${nom}.exe`,
        arguments: '',
        repertoire: 'c:\\programmes',
        icone: null,
        source_max: 'non-mesuree',
        accent: null,
        associations: [],
    };
}

function connue(id: string, cle: string, disparueA: number | null = null): Connue {
    return { id, cle, disparue_a: disparueA };
}

function message(
    complet: boolean,
    applications: Application[],
    disparues: string[] = [],
): CatalogueMessage {
    return { v: 2, type: 'catalogue', complet, applications, disparues };
}

describe('fusionner', () => {
    it('with `complet: true`, marks EVERY known row absent from the message as gone', () => {
        // 🔴 Treating a complete message as a delta would leave in the catalogue,
        // FOREVER, an application uninstalled while the channel
        // was down: its key would appear in no `disparues`, nobody
        // having seen it leave.
        const f = fusionner(
            [connue('id-a', 'a'), connue('id-b', 'b')],
            message(true, [app('a')]),
        );
        expect(f.aMarquerDisparues).toEqual(['id-b']);
        expect(f.toUpdate.map((m) => m.id)).toEqual(['id-a']);
    });

    it('does not RE-mark a row already gone', () => {
        // ⚠️ `disparue_a` is SET, NEVER REMOVED: rewriting it at each
        // reconciliation would advance the vanishing instant as long as
        // the agent runs, and the column would say "vanished 30 seconds ago"
        // of an application gone for a month.
        const f = fusionner([connue('id-b', 'b', 1_787_136_773_742)], message(true, []));
        expect(f.aMarquerDisparues).toEqual([]);
    });

    it("with `complet: false`, invents NO disappearance and honours only `disparues`", () => {
        // 🔴 The red opposite to the first one: treating a delta as a complete
        // state WOULD EMPTY the catalogue at each message carrying only one
        // appearance.
        const f = fusionner(
            [connue('id-a', 'a'), connue('id-b', 'b'), connue('id-c', 'c')],
            message(false, [app('a')], ['c']),
        );
        expect(f.aMarquerDisparues).toEqual(['id-c']);
        expect(f.aInserer).toEqual([]);
        expect(f.toUpdate.map((m) => m.id)).toEqual(['id-a']);
    });

    it('with `complet: true` and `applications: []`, EMPTIES the catalogue', () => {
        // The `catalogue_vide` case of the shared vectors: a VM on which
        // everything is uninstalled. Treating it as "nothing to do" would leave it
        // full.
        const f = fusionner([connue('id-a', 'a'), connue('id-b', 'b')], message(true, []));
        expect(f.aMarquerDisparues).toEqual(['id-a', 'id-b']);
        expect(f.aInserer).toEqual([]);
        expect(f.toUpdate).toEqual([]);
    });

    it("matches on the KEY: a known and present row goes into aMettreAJour, never into aInserer", () => {
        // 🔴 Pairing on the `id` would be impossible: the agent does not know them,
        // and has never seen them. It only knows the key.
        const f = fusionner([connue('id-a', 'a')], message(true, [app('a', 'renomme')]));
        expect(f.aInserer).toEqual([]);
        expect(f.toUpdate).toEqual([{ id: 'id-a', app: app('a', 'renomme') }]);
    });

    it('a known row that was GONE and is present again is RESURRECTED, and updated', () => {
        // 🔴 Inserting it anew would give it a NEW identifier, and a PWA
        // installed from the old one would point into the void. It is the same
        // loss of identifier as the one a DELETE would cause, through another
        // door.
        //
        // ⚠️ IT IS IN BOTH LISTS, and that is deliberate: resurrecting
        // sets `disparue_a` back to NULL, updating refreshes the fields. A
        // resurrection alone would make visible a row with stale fields.
        const f = fusionner(
            [connue('id-a', 'a', 1_787_136_773_742)],
            message(true, [app('a', 'revenu')]),
        );
        expect(f.aRessusciter).toEqual(['id-a']);
        expect(f.toUpdate).toEqual([{ id: 'id-a', app: app('a', 'revenu') }]);
        expect(f.aInserer).toEqual([]);
        expect(f.aMarquerDisparues).toEqual([]);
    });

    it('inserts an unknown key, and ignores a `disparues` that designates nobody', () => {
        // ⚠️ An unknown key in `disparues` is not an error: the agent
        // can announce the disappearance of an application the platform
        // never recorded — one lost upstream message is enough. Throwing
        // would take down the channel of an agent that is perfectly fine.
        const f = fusionner([], message(false, [app('neuve')], ['jamais-vue']));
        expect(f.aInserer).toEqual([app('neuve')]);
        expect(f.aMarquerDisparues).toEqual([]);
        expect(f.aRessusciter).toEqual([]);
    });

    it('is PURE: no clock read, no database driver', () => {
        // 🔴 THIS CHECK BLANKS THE COMMENTS BEFORE SEARCHING, and it is
        // not a matter of style: this very file NAMES `Date.now`
        // and `Pilote` in its own prose to explain why they are
        // absent. A bare `grep` would therefore be satisfied — or turned red — by the
        // documentation rather than by the code, and would discriminate nothing.
        // This repository has paid three times for this exact form of check.
        const source = readFileSync(
            path.join(path.dirname(fileURLToPath(import.meta.url)), 'catalogue.ts'),
            'utf8',
        );
        const code = source
            .replace(/\/\*[\s\S]*?\*\//g, '')
            .replace(/\/\/.*$/gm, '');
        expect([...code.matchAll(/\bDate\.now\b|\bPilote\b/g)].map((m) => m[0])).toEqual([]);
    });
});
