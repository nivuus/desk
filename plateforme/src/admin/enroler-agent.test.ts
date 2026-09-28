// The PURE part of command-line enrolment, and the guard that matters:
// a secret NEVER goes through the argv.
//
// 🔴 The red of this file is `--secret`: accepting it "for convenience"
// would expose the enrolment secret to EVERY user of the machine, `ps`
// giving the argv of every process — then the shell history would keep it.
// It is the exact twin of `DRAPEAUX_INTERDITS` in `creer-utilisateur.ts`.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { lireParVm } from '../depot/agent';
import { verifyEnrolment } from '../agents/enrolement';
import { verify } from '../identite/mot-de-passe';
import { analyserArguments, enrolerLaVm, roterLeSecret } from './enroler-agent';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe('analyserArguments of admin:agent', () => {
    it('reads --vm and --adresse', () => {
        expect(analyserArguments(['--vm', 'w1', '--adresse', '192.168.3.2']))
            .toEqual({ mode: 'enroler', vm: 'w1', adresse: '192.168.3.2' });
    });

    it('--roter requires ONLY --vm: it creates no VM, it fixes one', () => {
        expect(analyserArguments(['--vm', 'w1', '--roter']))
            .toEqual({ mode: 'roter', vm: 'w1' });
        // And --adresse, if lying around, changes nothing: the rotation does not touch
        // the `vm` table.
        expect(analyserArguments(['--vm', 'w1', '--roter', '--adresse', '10.0.0.1']))
            .toEqual({ mode: 'roter', vm: 'w1' });
    });

    it('🔴 --roter without --vm is refused, rather than rotating at random', () => {
        // 🔴 THE RED: letting it through. A rotation with no named target cannot
        // guess WHICH of the enrolled VMs must change secret.
        expect('refus' in analyserArguments(['--roter'])).toBe(true);
    });

    it('🔴 REFUSES --secret EVEN together with --roter', () => {
        // 🔴 THE RED: checking the forbidden flags only on the enrolment
        // path. The secret of a ROTATION is just as sensitive as
        // that of an enrolment — `ps` would expose it the same way —, and
        // it is precisely the path taken when a secret has leaked.
        const r = analyserArguments(['--vm', 'w1', '--roter', '--secret', 'chut']);
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/drawn at random/i);
        expect(r.refus).not.toContain('chut');
    });

    it('🔴 REFUSES --secret on the command line, with its reason', () => {
        // 🔴 The most useful red of the file: accepting it. The reason does not
        // COPY the refused value — rewriting it into a log after
        // refusing it in an argv would make no sense.
        const r = analyserArguments(['--vm', 'w1', '--adresse', '10.0.0.1', '--secret', 'chut']);
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/drawn at random/i);
        expect(r.refus).not.toContain('chut');
    });

    it('also refuses the variants of the same flag', () => {
        for (const drapeau of ['--secret-enrolement', '--password', '--mdp', '-s']) {
            const r = analyserArguments(['--vm', 'w1', '--adresse', '10.0.0.1', drapeau, 'chut']);
            expect('refus' in r).toBe(true);
        }
    });

    it('refuses a missing --vm or --adresse, rather than returning undefined', () => {
        expect('refus' in analyserArguments([])).toBe(true);
        expect('refus' in analyserArguments(['--vm', 'w1'])).toBe(true);
        expect('refus' in analyserArguments(['--adresse', '10.0.0.1'])).toBe(true);
        expect('refus' in analyserArguments(['--vm', '', '--adresse', '10.0.0.1'])).toBe(true);
    });
});

describe(`enrolerLaVm, engine=${MOTEUR}`, () => {
    it('🔴 draws the secret AT RANDOM: two calls with the SAME ARGUMENTS differ', async () => {
        // 🔴 The red: deriving it from the VM name. It would become guessable by
        // anyone who knows that name, and enrolment would authenticate nothing any more.
        //
        // ⚠️ BOTH CALLS CARRY THE SAME ARGUMENTS, AND THAT IS THE WHOLE
        // TEST. A first draft used two distinct VM names
        // (`w1`, `w2`): a secret derived from the name would then have differed
        // too, and the test would have stayed GREEN under the mutation — MEASURED, it
        // did. A check never seen red is not a
        // check, and this one could not be.
        //
        // `vm.nom` is not UNIQUE (`0001-socle.sql`): two enrolments of the
        // same name are therefore possible, and that is what makes the call repeatable.
        base = await baseNeuve('admin-agent-alea');
        const un = await enrolerLaVm(base, 'w1', '192.168.3.2', 1_787_136_773_742);
        const deux = await enrolerLaVm(base, 'w1', '192.168.3.2', 1_787_136_773_742);
        expect(un.secret).not.toBe(deux.secret);
        // Nor are the prefixes: two VMs do not fight over a
        // namespace, even under the same display name.
        expect(un.prefixe).not.toBe(deux.prefixe);
        expect(un.prefixe).toHaveLength(22);
        // Two distinct rows, hence two distinct identifiers.
        expect(un.vmId).not.toBe(deux.vmId);
    });

    it('🔴 writes the FINGERPRINT to the database, NEVER the plaintext secret', async () => {
        // 🔴 The red: writing the plaintext. The test reads the column back and would
        // find it there — a stolen database would then hand over every VM.
        base = await baseNeuve('admin-agent-empreinte');
        const { secret, prefixe } = await enrolerLaVm(base, 'w1', '192.168.3.2', 1_787_136_773_742);

        const [vm] = await base.interroger<{ id: string }>(
            'SELECT id FROM vm WHERE nom = ?', ['w1']);
        const ligne = await lireParVm(base, vm.id);
        expect(ligne).toBeDefined();
        expect(ligne!.empreinte_secret).not.toContain(secret);
        expect(ligne!.prefixe_session).toBe(prefixe);
        // And the hash does VERIFY the returned secret: without this assertion,
        // writing anything would pass the previous one.
        expect(await verify(secret, ligne!.empreinte_secret)).toBe(true);
        // An enrolled VM has not beaten yet.
        expect(ligne!.vu_a).toBeNull();
    });
});

describe(`roterLeSecret, engine=${MOTEUR}`, () => {
    it("🔴 (a) the OLD secret is refused and the NEW one accepted, end to end", async () => {
        // 🔴 THE RED: writing the secret in plaintext instead of its hash, or
        // writing nothing at all. The judge is not the column but
        // `verifyEnrolment`, that is the REAL path of the /agent channel:
        // it is the only way to know that the rotation produced a
        // hash the service can verify.
        base = await baseNeuve('admin-roter-bout-en-bout');
        const { vmId, secret: ancien } = await enrolerLaVm(
            base, 'w1', '192.168.3.2', 1_787_136_773_742);

        const r = await roterLeSecret(base, vmId);
        expect('refus' in r).toBe(false);
        if ('refus' in r) return;

        expect((await verifyEnrolment(base, vmId, ancien, () => {})).ok).toBe(false);
        expect((await verifyEnrolment(base, vmId, r.secret, () => {})).ok).toBe(true);
    });

    it('🔴 (b) the SESSION PREFIX is unchanged — re-read on both sides', async () => {
        // 🔴 THE RED: rotating the prefix too. It makes up the name of the VM's
        // LIVE sessions: changing it would cut them all. The
        // test reads the column BEFORE and AFTER the call, otherwise it would
        // measure nothing.
        base = await baseNeuve('admin-roter-prefixe');
        const { vmId, prefixe: before } = await enrolerLaVm(
            base, 'w1', '192.168.3.2', 1_787_136_773_742);

        const r = await roterLeSecret(base, vmId);
        expect('refus' in r).toBe(false);

        const apres = (await lireParVm(base, vmId))!.prefixe_session;
        expect(apres).toBe(before);
        expect(apres).toHaveLength(22);
    });

    it('🔴 (b bis) the new secret is DRAWN AT RANDOM: two rotations differ', async () => {
        // 🔴 THE RED: deriving it from the VM identifier. It would be guessable
        // by anyone who knows it, and the rotation would repair nothing.
        base = await baseNeuve('admin-roter-alea');
        const { vmId } = await enrolerLaVm(base, 'w1', '192.168.3.2', 1_787_136_773_742);
        const un = await roterLeSecret(base, vmId);
        const deux = await roterLeSecret(base, vmId);
        expect('refus' in un).toBe(false);
        expect('refus' in deux).toBe(false);
        if ('refus' in un || 'refus' in deux) return;
        expect(un.secret).not.toBe(deux.secret);
    });

    it('🔴 (c) an UNKNOWN VM gives a REASONED refusal, never an exception', async () => {
        // 🔴 THE RED: letting the UPDATE touch zero rows silently and
        // return a success. The administrator would believe they rotated a
        // compromised secret, and the old one would stay valid — the worst outcome
        // possible for this command, since it is used ONLY when a
        // secret has leaked.
        base = await baseNeuve('admin-roter-inconnue');
        const r = await roterLeSecret(base, 'no-vm-by-this-name');
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/enrolled/i);
    });
});
