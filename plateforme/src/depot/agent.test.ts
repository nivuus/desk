// These tests run under `test:sqlite` AND under `test:postgres`, without being
// written twice: they use the same harness as `pilotes.test.ts`.
//
// 🔴 THE VALUES ARE REALISTIC, NEVER CONVENIENT. It is the most expensive lesson
// of P1: the double pass only wrote `1_000`s, and declared portable a
// schema Postgres refused for every real write. `vu_a` therefore receives
// an EPOCH MAGNITUDE, and the hash a real `scrypt` length.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { hacher } from '../identite/mot-de-passe';
import { enroler, lireParPrefixe, lireParVm, marquerVu, remplacerEmpreinte } from './agent';

let base: Pilote | undefined;

/// The magnitude that really broke Postgres in P1 (`pilotes.test.ts:111`).
const MS = 1_787_136_773_742;
/// A prefix of the REAL length `agents/prefixe.ts` produces.
const PREFIXE = 'RhH1x2QmTz9kLpVbNc7dAw';

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

/// A VM to hang on to: `agent_enrole.vm_id` REFERENCES it, and
/// SQLite applies the foreign key (`PRAGMA foreign_keys=ON` at opening).
async function withVm(p: Pilote, id: string): Promise<void> {
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [id, `vm-${id}`, '192.168.3.2']);
}

describe(`agent_enrole repository, engine=${MOTEUR}`, () => {
    it('enrols, then re-reads by VM', async () => {
        base = await baseNeuve('agent-enrole');
        await withVm(base, 'v-1');
        // A REAL hash, produced by the same derivation as the
        // human accounts — not a short string that measures no
        // column length.
        const empreinte = await hacher('un-secret-d-enrolement-de-la-vraie-longueur');
        await enroler(base, 'v-1', empreinte, PREFIXE);

        const ligne = await lireParVm(base, 'v-1');
        expect(ligne).toBeDefined();
        expect(ligne!.vm_id).toBe('v-1');
        expect(ligne!.empreinte_secret).toBe(empreinte);
        expect(ligne!.prefixe_session).toBe(PREFIXE);
        // An enrolled VM that has never beaten: NULL, not zero.
        expect(ligne!.vu_a).toBeNull();
    });

    it('re-reads the SAME row by its PREFIX', async () => {
        // 🔴 The red: querying on `vm_id`. The prefix is what the session
        // name carries — it is the only key `signaling/trace.ts`
        // has to trace back to the VM.
        base = await baseNeuve('agent-prefixe');
        await withVm(base, 'v-1');
        await enroler(base, 'v-1', await hacher('secret-un'), PREFIXE);

        const parPrefixe = await lireParPrefixe(base, PREFIXE);
        const parVm = await lireParVm(base, 'v-1');
        expect(parPrefixe).toEqual(parVm);
        expect(parPrefixe!.vm_id).toBe('v-1');
    });

    it('returns undefined on an unknown one, WITHOUT THROWING, on both sides', async () => {
        // 🔴 Throwing would make the channel answer 500 where it must answer a
        // refusal — and the difference in behaviour would on its own be an enumeration
        // oracle. Precedent: `depot/utilisateur.ts::lireParEmail`.
        base = await baseNeuve('agent-inconnu');
        await expect(lireParVm(base, 'v-jamais-vue')).resolves.toBeUndefined();
        await expect(lireParPrefixe(base, 'PrefixeQuiNExistePas22')).resolves.toBeUndefined();
    });

    it('REFUSES a second enrolment on the SAME prefix', async () => {
        // 🔴 The red: removing UNIQUE from the migration. Two VMs with the same
        // prefix would make `lireParPrefixe` ambiguous, and the choice of VM
        // arbitrary — the exact problem the prefix exists to close.
        base = await baseNeuve('agent-unique');
        await withVm(base, 'v-1');
        await withVm(base, 'v-2');
        await enroler(base, 'v-1', await hacher('secret-un'), PREFIXE);
        await expect(enroler(base, 'v-2', await hacher('secret-deux'), PREFIXE)).rejects.toThrow();
    });

    it('marks vu_a at an EPOCH MAGNITUDE, and re-reads it AS `number`', async () => {
        // 🔴 Writing `1_000` would pass on both engines without proving anything:
        // it is the exact blind spot P1 paid for.
        //
        // ⚠️ THIS TEST WRAPPED ITS TWO READS IN `Number(...)`, and that
        // `Number` HID the defect instead of measuring it: `pg` returned this
        // `BIGINT` as a **string**, so that `LigneAgent.vu_a` declared
        // `number | null` a value that was a `string` in PRODUCTION.
        // Found by P3's acceptance run, fixed in the DRIVER
        // (`base/pilote-postgres.ts`, `setTypeParser`) because the defect
        // was one of class. The assertion is therefore now BARE — it is what
        // keeps the type declaration honest.
        base = await baseNeuve('agent-vu');
        await withVm(base, 'v-1');
        await enroler(base, 'v-1', await hacher('secret-un'), PREFIXE);

        await marquerVu(base, 'v-1', MS);
        expect((await lireParVm(base, 'v-1'))!.vu_a).toBe(MS);
        // The next heartbeat ADVANCES the value, it does not add it.
        await marquerVu(base, 'v-1', MS + 30_000);
        expect((await lireParVm(base, 'v-1'))!.vu_a).toBe(MS + 30_000);
    });
});

describe(`remplacerEmpreinte, engine=${MOTEUR}`, () => {
    it("replaces the fingerprint of the named VM, and returns the number of rows touched", async () => {
        base = await baseNeuve('agent-rotation');
        await withVm(base, 'v-1');
        const ancienne = await hacher('le-secret-d-origine-de-la-vraie-longueur');
        await enroler(base, 'v-1', ancienne, PREFIXE);

        const neuve = await hacher('le-secret-de-remplacement-tout-aussi-long');
        expect(await remplacerEmpreinte(base, 'v-1', neuve)).toBe(1);

        const ligne = await lireParVm(base, 'v-1');
        expect(ligne!.empreinte_secret).toBe(neuve);
    });

    it("🔴 DOES NOT TOUCH the session prefix — re-read on BOTH sides of the call", async () => {
        // 🔴 THE RED: rotating the prefix together with the secret.
        // It makes up the name of this VM's LIVE sessions
        // (`agents/prefixe.ts`): changing it would cut every ongoing session.
        // Rotating the secret is not rotating the identity.
        base = await baseNeuve('agent-rotation-prefixe');
        await withVm(base, 'v-1');
        await enroler(base, 'v-1', await hacher('le-secret-d-origine-tres-long'), PREFIXE);

        const before = (await lireParVm(base, 'v-1'))!.prefixe_session;
        await remplacerEmpreinte(base, 'v-1', await hacher('un-tout-autre-secret-aussi-long'));
        const apres = (await lireParVm(base, 'v-1'))!.prefixe_session;

        expect(apres).toBe(before);
        expect(apres).toBe(PREFIXE);
    });

    it("🔴 touches NO other VM, and does not throw on an unknown VM", async () => {
        // 🔴 THE RED: forgetting the WHERE clause. All VMs would then share
        // the same secret, which no single-VM test would see.
        base = await baseNeuve('agent-rotation-portee');
        await withVm(base, 'v-1');
        await withVm(base, 'v-2');
        const gardee = await hacher('le-secret-de-la-vm-voisine-bien-long');
        await enroler(base, 'v-1', await hacher('le-secret-a-remplacer-bien-long'), PREFIXE);
        await enroler(base, 'v-2', gardee, 'Zk4pQ7mNr2xTvB9wLcHd1s');

        await remplacerEmpreinte(base, 'v-1', await hacher('le-secret-neuf-tout-aussi-long'));
        expect((await lireParVm(base, 'v-2'))!.empreinte_secret).toBe(gardee);

        // An unknown VM: zero rows touched, and above all NO exception —
        // it is what lets the caller return a reasoned refusal.
        expect(await remplacerEmpreinte(base, 'v-jamais-enrolee', 'peu-importe')).toBe(0);
    });
});
