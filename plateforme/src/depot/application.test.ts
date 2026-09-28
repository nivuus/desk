// These tests run under `test:sqlite` AND under `test:postgres`, through the same
// harness as `pilotes.test.ts` and `agent.test.ts`.
//
// 🔴 THE VALUES ARE REALISTIC, NEVER CONVENIENT: the timestamps carry an
// EPOCH MAGNITUDE, the paths are Windows paths, and the keys have the
// length of a hash. It is the most expensive lesson of P1 — the double
// pass only wrote `1_000`s, and declared portable a schema that
// Postgres refused for every real write.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Application } from '../../../proto/ts/plateforme';
import type { Fusion } from '../apps/catalogue';
import {
    appliquer,
    lireConnues,
    lireParId,
    lireParVm,
    pxDepuisSourceMax,
    sourceMaxDepuis,
} from './application';

let base: Pilote | undefined;

/// The magnitude that really broke Postgres in P1.
const MS = 1_787_136_773_742;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function withVm(p: Pilote, id: string): Promise<void> {
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [id, `vm-${id}`, '192.168.3.2']);
}

function app(nom: string, cle: string): Application {
    return {
        cle,
        nom,
        chemin: `C:\\Users\\guacamole\\Desktop\\${nom}.lnk`,
        cible: `c:\\program files\\${nom}\\${nom}.exe`,
        arguments: '',
        repertoire: `c:\\program files\\${nom}`,
        icone: null,
        source_max: 'non-mesuree',
        accent: null,
        associations: [],
    };
}

/// A hash of the REAL length of a hexadecimal SHA-256.
function cle(n: number): string {
    return `${n}`.padStart(64, 'a');
}

const RIEN: Fusion = { aInserer: [], toUpdate: [], aMarquerDisparues: [], aRessusciter: [] };

describe(`application repository, engine=${MOTEUR}`, () => {
    it('lireParVm returns ONLY the applications of this VM', async () => {
        // 🔴 Omitting the `WHERE vm_id = ?` would show a user the
        // catalogue of every VM of the service.
        base = await baseNeuve('app-par-vm');
        await withVm(base, 'v-1');
        await withVm(base, 'v-2');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Firefox', cle(1))] }, MS);
        await appliquer(base, 'v-2', { ...RIEN, aInserer: [app('Excel', cle(2))] }, MS);

        expect((await lireParVm(base, 'v-1')).map((l) => l.nom)).toEqual(['Firefox']);
        expect((await lireParVm(base, 'v-2')).map((l) => l.nom)).toEqual(['Excel']);
    });

    it('lireParVm EXCLUDES the gone and the hidden ones', async () => {
        // 🔴 Including them would make the displayed catalogue carry applications
        // that no longer exist on the VM — and the hub would offer to launch a
        // deleted shortcut.
        base = await baseNeuve('app-exclusions');
        await withVm(base, 'v-1');
        await appliquer(
            base,
            'v-1',
            { ...RIEN, aInserer: [app('Vivante', cle(1)), app('Partie', cle(2)), app('Masquee', cle(3))] },
            MS,
        );
        const partie = (await lireParVm(base, 'v-1')).find((l) => l.nom === 'Partie')!;
        const masquee = (await lireParVm(base, 'v-1')).find((l) => l.nom === 'Masquee')!;
        await appliquer(base, 'v-1', { ...RIEN, aMarquerDisparues: [partie.id] }, MS + 10);
        // `masquee_a` has no writer in G1: the hiding gesture does not exist
        // yet. The column is set by hand, exactly as a P4 test
        // sets `vm.utilisateur_id` that nothing fills before it.
        await base.executer('UPDATE application SET masquee_a = ? WHERE id = ?', [MS + 20, masquee.id]);

        expect((await lireParVm(base, 'v-1')).map((l) => l.nom)).toEqual(['Vivante']);
        // ⚠️ But they are STILL THERE: it is the half that decides, and without
        // it a `DELETE` would pass this test.
        expect(await lireParId(base, partie.id)).toBeDefined();
        expect(await lireParId(base, masquee.id)).toBeDefined();
    });

    it('lireParId returns `undefined` on an unknown identifier, NEVER an exception', async () => {
        // Precedent: `depot/agent.ts::lireParVm` and `depot/vm.ts::lireParId`.
        // An exception bubbling up as a 500 would be an enumeration oracle.
        base = await baseNeuve('app-inconnue');
        await expect(lireParId(base, 'jamais-vu')).resolves.toBeUndefined();
    });

    it("generates the identifier IN THE REPOSITORY, and sets apparue_a", async () => {
        // 🔴 Letting it come from the agent would mean two VMs could
        // produce the same one — the key, for its part, is the hash of a triple of
        // paths, and two VMs carrying the same application share it.
        base = await baseNeuve('app-identifiant');
        await withVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Firefox', cle(1))] }, MS);
        const [ligne] = await lireParVm(base, 'v-1');

        // The shape of a `randomUUID()`: 36 characters, five groups, version 4.
        expect(ligne.id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
        expect([ligne.apparue_a, ligne.vue_a, ligne.disparue_a, ligne.masquee_a])
            .toEqual([MS, MS, null, null]);
    });

    it("updates the fields and advances vue_a, touching neither the id nor apparue_a", async () => {
        base = await baseNeuve('app-maj');
        await withVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Ancien', cle(1))] }, MS);
        const before = (await lireParVm(base, 'v-1'))[0];

        await appliquer(
            base,
            'v-1',
            { ...RIEN, toUpdate: [{ id: before.id, app: app('Neuf', cle(1)) }] },
            MS + 30,
        );
        const apres = (await lireParVm(base, 'v-1'))[0];
        expect(apres.id).toBe(before.id);
        expect(apres.nom).toBe('Neuf');
        expect(apres.cible).toBe('c:\\program files\\Neuf\\Neuf.exe');
        expect(apres.vue_a).toBe(MS + 30);
        // 🔴 `apparue_a` is the FIRST sighting: advancing it at each update
        // would make it a duplicate of `vue_a`, and the installation verdict that
        // will read it one day would never again see an appearance.
        expect(apres.apparue_a).toBe(MS);
    });

    it('sets disparue_a WITHOUT DELETING THE ROW', async () => {
        // 🔴 THE TEST READS BOTH, and that is what makes it discriminating: a
        // `DELETE` would indeed give "no longer in the catalogue", and would make an
        // application installed on the browser side lose its identifier.
        base = await baseNeuve('app-disparue');
        await withVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Partante', cle(1))] }, MS);
        const before = (await lireParVm(base, 'v-1'))[0];

        await appliquer(base, 'v-1', { ...RIEN, aMarquerDisparues: [before.id] }, MS + 40);

        expect(await lireParVm(base, 'v-1')).toEqual([]);
        const ligne = await lireParId(base, before.id);
        expect(ligne).toBeDefined();
        expect(ligne!.disparue_a).toBe(MS + 40);
        expect(ligne!.id).toBe(before.id);
    });

    it("resurrects: disparue_a goes back to NULL, and the identifier DOES NOT CHANGE", async () => {
        // 🔴 Inserting a new row would be the same loss of identifier, through
        // another door.
        base = await baseNeuve('app-resurrection');
        await withVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Revenante', cle(1))] }, MS);
        const before = (await lireParVm(base, 'v-1'))[0];
        await appliquer(base, 'v-1', { ...RIEN, aMarquerDisparues: [before.id] }, MS + 50);

        await appliquer(
            base,
            'v-1',
            {
                ...RIEN,
                aRessusciter: [before.id],
                toUpdate: [{ id: before.id, app: app('Revenante', cle(1)) }],
            },
            MS + 60,
        );

        const [apres] = await lireParVm(base, 'v-1');
        expect(apres.id).toBe(before.id);
        expect(apres.disparue_a).toBeNull();
        expect(apres.apparue_a).toBe(MS);
    });

    it('lireConnues ALSO returns the gone ones, with their instant of disappearance', async () => {
        // 🔴 It is what distinguishes this reader from `lireParVm`, and the omission
        // would be serious: a merge that did not see the vanished ones would
        // REINSERT them on their return, with a new identifier. The reader of the
        // displayed catalogue and the reader of the merge can therefore NOT
        // be the same.
        base = await baseNeuve('app-connues');
        await withVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('A', cle(1)), app('B', cle(2))] }, MS);
        const b = (await lireParVm(base, 'v-1')).find((l) => l.nom === 'B')!;
        await appliquer(base, 'v-1', { ...RIEN, aMarquerDisparues: [b.id] }, MS + 70);

        const connues = await lireConnues(base, 'v-1');
        expect(connues.map((c) => c.cle).sort()).toEqual([cle(1), cle(2)].sort());
        expect(connues.find((c) => c.cle === cle(2))!.disparue_a).toBe(MS + 70);
        expect(connues.find((c) => c.cle === cle(1))!.disparue_a).toBeNull();
    });

    it("writes NOTHING when a single write of the merge fails", async () => {
        // 🔴 A MERGE APPLIES WHOLLY OR NOT AT ALL. A half-written
        // catalogue is indistinguishable from a correct catalogue at the next
        // round: the next reconciliation would take it for the state of the
        // VM, and the missing rows would only come back at the next complete
        // send — or never, if the agent no longer emits one.
        //
        // 🔴 THE FAILURE IS PLACED AFTER A WRITE THAT SUCCEEDS, and it is
        // not a staging detail: a first draft made the VERY FIRST
        // write fail, and the mutation "remove the
        // transaction" SURVIVED it — nothing had been written before the failure,
        // so both versions returned the same state. The check
        // could not fail.
        //
        // Here, the merge inserts two applications with the SAME key for the SAME
        // VM: the first passes, the second is refused by the unique index
        // `application_cle`. Without a transaction, the first would remain.
        base = await baseNeuve('app-transaction');
        await withVm(base, 'v-1');

        await expect(
            appliquer(
                base,
                'v-1',
                { ...RIEN, aInserer: [app('Premiere', cle(1)), app('Doublon', cle(1))] },
                MS + 80,
            ),
        ).rejects.toThrow();

        // NEITHER ONE NOR THE OTHER: the first was indeed written, then rolled back.
        expect(await lireParVm(base, 'v-1')).toEqual([]);
        expect(await lireConnues(base, 'v-1')).toEqual([]);
    });

    it('carries NO literal value in its queries', async () => {
        // 🔴 THIS CHECK ONLY TURNS RED UNDER `test:postgres`, and it is measured:
        // `rendreMarqueurs` (`base/pilote.ts`) is only called by
        // `pilote-postgres.ts`, and it alone THROWS on an
        // apostrophe. Under SQLite the offending query would pass without a word.
        // The red therefore plays out on BOTH engines, and is only visible on
        // one — it is exactly the blind spot the double pass exists to
        // cover, and the static lint of `sous-ensemble.test.ts` only sweeps
        // the `.sql` files.
        //
        // This case exercises the FOUR writes and the THREE reads of the module
        // at once: it is the only way to make each query go through
        // the marker converter.
        base = await baseNeuve('app-marqueurs');
        await withVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Une', cle(1))] }, MS);
        const une = (await lireParVm(base, 'v-1'))[0];
        await appliquer(
            base,
            'v-1',
            {
                aInserer: [],
                toUpdate: [{ id: une.id, app: app('Une', cle(1)) }],
                aMarquerDisparues: [une.id],
                aRessusciter: [une.id],
            },
            MS + 90,
        );
        await lireConnues(base, 'v-1');
        await expect(lireParId(base, une.id)).resolves.toBeDefined();
    });

    it('🔴 `NULL` is re-read as `non-mesuree`, NEVER `{pixels:0}` — criterion ④', async () => {
        // 🔴 REPRESENTING `NonMesuree` BY A NUMBER WOULD MAKE AN UNKNOWN
        // PROVENANCE CLAIM TO BE WORTH SOMETHING, and that is all
        // sub-block G2 exists to prevent. The rule is written ONLY
        // ONCE, in the repository layer, precisely so that it cannot diverge.
        expect(sourceMaxDepuis(null)).toBe('non-mesuree');
        expect(sourceMaxDepuis(null)).not.toEqual({ pixels: 0 });
        expect(sourceMaxDepuis(256)).toEqual({ pixels: 256 });
        expect(sourceMaxDepuis(48)).toEqual({ pixels: 48 });
        // And the reverse, which must close the round trip.
        expect(pxDepuisSourceMax('non-mesuree')).toBeNull();
        expect(pxDepuisSourceMax({ pixels: 256 })).toBe(256);
    });

    it('🔴 both icon fields make the ROUND TRIP through the database', async () => {
        base = await baseNeuve('app-icones');
        const p = base;
        await withVm(p, 'v-ico');
        await appliquer(p, 'v-ico', {
            aInserer: [
                { ...app('Avec', 'k-avec'), icone: 'f'.repeat(64), source_max: { pixels: 256 } },
                { ...app('Sans', 'k-sans'), icone: null, source_max: 'non-mesuree' },
            ],
            toUpdate: [],
            aMarquerDisparues: [],
            aRessusciter: [],
        }, 1_700_000_000_000);
        const lignes = await lireParVm(p, 'v-ico');
        const withIt = lignes.find((l) => l.nom === 'Avec')!;
        const sans = lignes.find((l) => l.nom === 'Sans')!;
        expect(withIt.icone).toBe('f'.repeat(64));
        expect(sourceMaxDepuis(withIt.source_max_px)).toEqual({ pixels: 256 });
        // 🔴 THE FORBIDDEN COMBINATION — a null `icone` and a measured size —
        // IS WRITTEN BY NO PATH. The test NAMES it so that it is not
        // born of carelessness.
        expect(sans.icone).toBeNull();
        expect(sans.source_max_px).toBeNull();
        expect(sourceMaxDepuis(sans.source_max_px)).toBe('non-mesuree');

        // An icon that CHANGES does reach the database: it is the nominal case
        // of an application that updates itself, not the exception.
        const id = withIt.id;
        await appliquer(p, 'v-ico', {
            aInserer: [],
            toUpdate: [
                { id, app: { ...app('Avec', 'k-avec'), icone: 'e'.repeat(64), source_max: { pixels: 48 } } },
            ],
            aMarquerDisparues: [],
            aRessusciter: [],
        }, 1_700_000_001_000);
        const relu = (await lireParVm(p, 'v-ico')).find((l) => l.id === id)!;
        expect(relu.icone).toBe('e'.repeat(64));
        expect(sourceMaxDepuis(relu.source_max_px)).toEqual({ pixels: 48 });
    });
});
