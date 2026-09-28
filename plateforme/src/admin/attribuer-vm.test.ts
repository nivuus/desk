// Assigning a VM through the administration command line.
//
// 🔴 HERE WE NAME THE CAUSE, AND IT IS THE OPPOSITE OF THE ROUTES. `http/routes-vm.ts`
// returns the SAME refusal for "unknown VM" and "someone else's VM", because a
// public route that told them apart would be an enumeration oracle. This
// command is an ADMINISTRATION command: enumeration is not a risk
// there — the caller already has access to the database and the config secret —
// and hiding the cause from them would send them looking elsewhere (D8, E8).
//
// ⚠️ SEVEN TESTS, as the plan announces.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { enroler } from '../depot/agent';
import { lireParId } from '../depot/vm';
import { createUser } from '../depot/utilisateur';
import { analyserArguments, appliquer } from './attribuer-vm';

const MS = 1_787_136_773_742;

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function poserVm(p: Pilote, id: string, nom: string): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [id, nom, '192.168.3.2']);
    await enroler(p, id, 'empreinte-opaque', `PREFIXE${id}`);
}

describe('analyserArguments of admin:attribuer', () => {
    it('🔴 requires --email AND --vm, with NO DEFAULT for either', () => {
        // 🔴 The red: giving one of the two a default. A default on
        // `--email` would assign the VM to an account the operator did not
        // name; a default on `--vm` would pick one at random. Both
        // are irreversible gestures with no confirmation.
        expect(analyserArguments(['--email', 'ada@exemple.test', '--vm', 'w1'])).toEqual({
            action: 'attribuer',
            email: 'ada@exemple.test',
            vm: 'w1',
        });
        expect('refus' in analyserArguments([])).toBe(true);
        expect('refus' in analyserArguments(['--email', 'ada@exemple.test'])).toBe(true);
        expect('refus' in analyserArguments(['--vm', 'w1'])).toBe(true);
        // An EMPTY value is not a value.
        expect('refus' in analyserArguments(['--email', '', '--vm', 'w1'])).toBe(true);
        expect('refus' in analyserArguments(['--email', 'ada@exemple.test', '--vm', ''])).toBe(true);
    });

    it('reuses AS IS the list of refused flags of the two other commands', () => {
        // ⚠️ NO SECRET IS AT STAKE IN THIS COMMAND, and the list is
        // reused anyway: an administration command that accepted
        // `--mot-de-passe` without using it would still leave the string
        // in `ps`, where any user of the machine would read it. The refusal is
        // explicit and carries its reason.
        for (const drapeau of ['--mot-de-passe', '--motdepasse', '--password', '--mdp', '-p',
                               '--secret', '--secret-enrolement', '-s']) {
            const r = analyserArguments(['--email', 'ada@exemple.test', '--vm', 'w1', drapeau, 'chut']);
            expect('refus' in r).toBe(true);
            if (!('refus' in r)) return;
            // The reason does NOT COPY the refused value.
            expect(r.refus).not.toContain('chut');
        }
    });

    it('🔴 `--detacher` does NOT require an email, and says so', () => {
        // 🔴 The red: ignoring the flag, or requiring `--email` with it. One
        // detaches a VM FROM someone; requiring that someone to be named
        // would force the operator to know in advance what the command will
        // tell them.
        expect(analyserArguments(['--detacher', '--vm', 'w1'])).toEqual({
            action: 'detacher',
            vm: 'w1',
        });
        expect('refus' in analyserArguments(['--detacher'])).toBe(true);
    });
});

describe(`appliquer of admin:attribuer, engine=${MOTEUR}`, () => {
    it('🔴 success → code 0, and the RE-READ row carries the owner', async () => {
        base = await baseNeuve('adm-attrib-ok');
        await poserVm(base, 'v1', 'w1');
        const ada = await createUser(base, 'ada@exemple.test', 'empreinte', MS);
        const issue = await appliquer(base, {
            action: 'attribuer',
            email: 'ada@exemple.test',
            vm: 'w1',
        });
        expect(issue.code).toBe(0);
        // The STATE read back, never the exit code alone: a `return 0` that
        // wrote nothing would pass the first assertion.
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(ada);
        // `--vm` accepts a NAME or an IDENTIFIER, the name first:
        // the administrator knows the one they gave.
        expect(issue.sortie).toContain('v1');
    });

    it('🔴 `--detacher` returns the VM to the pool, and a NEW assignment becomes possible again', async () => {
        // 🔴 The red: ignoring the flag. The VM would stay taken for life, and
        // there would be no path to give it back — `depot/vm.ts::detacher`
        // has no other caller.
        base = await baseNeuve('adm-detacher');
        await poserVm(base, 'v1', 'w1');
        await createUser(base, 'ada@exemple.test', 'empreinte', MS);
        const bob = await createUser(base, 'bob@exemple.test', 'empreinte', MS);
        await appliquer(base, { action: 'attribuer', email: 'ada@exemple.test', vm: 'w1' });

        expect((await appliquer(base, { action: 'detacher', vm: 'w1' })).code).toBe(0);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBeNull();

        const encore = await appliquer(base, {
            action: 'attribuer',
            email: 'bob@exemple.test',
            vm: 'w1',
        });
        expect(encore.code).toBe(0);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(bob);
    });

    it('UNKNOWN email → code 2, and the message NAMES the cause', async () => {
        // ⚠️ We name, here. See the header: this is not a public route.
        base = await baseNeuve('adm-courriel-inconnu');
        await poserVm(base, 'v1', 'w1');
        const issue = await appliquer(base, {
            action: 'attribuer',
            email: 'personne@exemple.test',
            vm: 'w1',
        });
        expect(issue.code).toBe(2);
        expect(issue.error).toMatch(/personne@exemple\.test/);
        expect(issue.error).toMatch(/account|email/i);
        // And nothing was written.
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBeNull();
    });

    it('UNKNOWN VM → code 2, message naming the VM', async () => {
        base = await baseNeuve('adm-vm-inconnue');
        await createUser(base, 'ada@exemple.test', 'empreinte', MS);
        const issue = await appliquer(base, {
            action: 'attribuer',
            email: 'ada@exemple.test',
            vm: 'w-jamais-creee',
        });
        expect(issue.code).toBe(2);
        expect(issue.error).toMatch(/w-jamais-creee/);
    });

    it('🔴 VM ALREADY TAKEN → code 2, message naming the OWNER', async () => {
        // 🔴 The red: not naming the owner. The operator would know that
        // it failed without knowing whom to detach — they would have to open the database
        // by hand, which this command exists to avoid.
        base = await baseNeuve('adm-vm-prise');
        await poserVm(base, 'v1', 'w1');
        const ada = await createUser(base, 'ada@exemple.test', 'empreinte', MS);
        await createUser(base, 'bob@exemple.test', 'empreinte', MS);
        await appliquer(base, { action: 'attribuer', email: 'ada@exemple.test', vm: 'w1' });

        const issue = await appliquer(base, {
            action: 'attribuer',
            email: 'bob@exemple.test',
            vm: 'w1',
        });
        expect(issue.code).toBe(2);
        expect(issue.error).toContain(ada);
        // And the VM did NOT change hands.
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(ada);
    });

    it('🔴 user who ALREADY has a VM → code 2 and a message, NEVER a stack trace', async () => {
        // 🔴 The red: letting the uniqueness exception bubble up. The administrator
        // would see `duplicate key value violates unique constraint
        // "vm_un_utilisateur"` — or its SQLite twin, which does not say the same
        // thing —, which does not tell them what to do. The orchestrator's typed
        // refusal is translated into a sentence.
        base = await baseNeuve('adm-deja-servi');
        await poserVm(base, 'v1', 'w1');
        await poserVm(base, 'v2', 'w2');
        await createUser(base, 'ada@exemple.test', 'empreinte', MS);
        await appliquer(base, { action: 'attribuer', email: 'ada@exemple.test', vm: 'w1' });

        const issue = await appliquer(base, {
            action: 'attribuer',
            email: 'ada@exemple.test',
            vm: 'w2',
        });
        expect(issue.code).toBe(2);
        expect(issue.error).toMatch(/already/i);
        // No stack trace, and no engine text: the two engines
        // do not write the same one, so one of the two would be wrong.
        expect(issue.error).not.toMatch(/UNIQUE constraint|duplicate key|at Object|\bat \w+\./);
        expect((await lireParId(base, 'v2'))?.utilisateur_id).toBeNull();
    });
});
