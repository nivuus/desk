// These tests run under `test:sqlite` AND under `test:postgres`, without being
// written twice: they use the same harness as `pilotes.test.ts`.
//
// 🔴 THE CLOCK IS A PARAMETER, and it is what these exact values
// check. A `Date.now()` hidden in the repository layer would make them all fail.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import {
    balayerLesOuvertes,
    clore,
    compterOuvertesDe,
    lireParNom,
    ouvrirSession,
} from './session';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe(`session repository, engine=${MOTEUR}`, () => {
    it('opens a row with ouverte_a set and fermee_a null', async () => {
        base = await baseNeuve('dep-ouvre');
        const id = await ouvrirSession(base, 'bureau', 1_000_000);
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.id).toBe(id);
        expect(ligne.nom_session).toBe('bureau');
        // EXACT value: it is the assertion that forbids a hidden `Date.now()`.
        expect(Number(ligne.ouverte_a)).toBe(1_000_000);
        expect(ligne.fermee_a).toBeNull();
        expect(ligne.motif).toBeNull();
    });

    it('closes the row: fermee_a set, reason recorded', async () => {
        base = await baseNeuve('dep-clot');
        const id = await ouvrirSession(base, 'bureau', 1_000_000);
        await clore(base, id, 1_000_500, 'both peers left');
        const [ligne] = await lireParNom(base, 'bureau');
        expect(Number(ligne.fermee_a)).toBe(1_000_500);
        expect(ligne.motif).toBe('both peers left');
    });

    it('does not close twice: the second closing does not change fermee_a', async () => {
        base = await baseNeuve('dep-double');
        const id = await ouvrirSession(base, 'bureau', 1_000_000);
        await clore(base, id, 1_000_500, 'premier');
        await clore(base, id, 9_000_000, 'second');
        const [ligne] = await lireParNom(base, 'bureau');
        expect(Number(ligne.fermee_a)).toBe(1_000_500);
        expect(ligne.motif).toBe('premier');
    });

    it('sweeps at startup the rows left open, and returns their count', async () => {
        base = await baseNeuve('dep-balai');
        await ouvrirSession(base, 'a', 1_000);
        await ouvrirSession(base, 'b', 2_000);
        const close = await ouvrirSession(base, 'c', 3_000);
        await clore(base, close, 4_000, 'normale');

        expect(await balayerLesOuvertes(base, 5_000)).toBe(2);
        // Idempotent: a second sweep finds nothing any more.
        expect(await balayerLesOuvertes(base, 6_000)).toBe(0);

        const [a] = await lireParNom(base, 'a');
        expect(Number(a.fermee_a)).toBe(5_000);
        expect(a.motif).toBe('platform restarted');
        // The already closed row keeps ITS instant and ITS reason.
        const [c] = await lireParNom(base, 'c');
        expect(Number(c.fermee_a)).toBe(4_000);
        expect(c.motif).toBe('normale');
    });

    it('two successive sessions of the same name are two distinct rows', async () => {
        // The session name is NOT unique over time: `bureau` comes back at
        // every agent start (`agent/src/superviseur/protocole.rs`,
        // constant SESSION_DE_CONTROLE). The primary key is a UUID, never
        // the name.
        base = await baseNeuve('dep-homonymes');
        const un = await ouvrirSession(base, 'bureau', 1_000);
        await clore(base, un, 2_000, 'fin');
        const deux = await ouvrirSession(base, 'bureau', 3_000);
        expect(deux).not.toBe(un);
        expect(await lireParNom(base, 'bureau')).toHaveLength(2);
    });

    it('writes utilisateur_id NULL when none is given — P1 is intact', async () => {
        // 🔴 The parameter is OPTIONAL: making it required would break all of
        // P1's calls, and a `bureau` control session where the agent arrives
        // alone has nobody to record.
        base = await baseNeuve('dep-without-user');
        await ouvrirSession(base, 'bureau', 1_000_000);
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.utilisateur_id).toBeNull();
    });

    it('writes vm_id NULL when none is given — the local trial mode', async () => {
        // 🔴 The red: making the parameter MANDATORY. A `bureau` session
        // opened by a NON-enrolled agent — the local trial mode the spec
        // §10 sets as legitimate — has no honest VM to record, and the
        // column stays NULLABLE for this reason, not out of debt.
        base = await baseNeuve('dep-sans-vm');
        await ouvrirSession(base, 'bureau', 1_000_000);
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.vm_id).toBeNull();
    });

    it('writes vm_id when the trace resolved the prefix — legacy item no. 3 of P2', async () => {
        // `session.vm_id` stayed entirely NULL at the end of P2. It is the
        // trace that resolves it (prefix of the session name -> VM), and it is here
        // that it records it.
        base = await baseNeuve('dep-with-vm');
        await ouvrirSession(base, 'RhH1x2QmTz9kLpVbNc7dAw:bureau', 1_787_136_773_742, undefined, 'v-42');
        const [ligne] = await lireParNom(base, 'RhH1x2QmTz9kLpVbNc7dAw:bureau');
        expect(ligne.vm_id).toBe('v-42');
        // The user stays NULL: an agent alone has nobody to record.
        expect(ligne.utilisateur_id).toBeNull();
        // And nothing else moved — the epoch magnitude included.
        expect(Number(ligne.ouverte_a)).toBe(1_787_136_773_742);
        expect(ligne.fermee_a).toBeNull();
    });

    it('writes BOTH when the guard and the trace each established theirs', async () => {
        base = await baseNeuve('dep-with-both');
        await ouvrirSession(base, 'P:w-1', 1_000_000, 'u-42', 'v-42');
        const [ligne] = await lireParNom(base, 'P:w-1');
        expect(ligne.utilisateur_id).toBe('u-42');
        expect(ligne.vm_id).toBe('v-42');
    });

    it('writes utilisateur_id when the guard established one', async () => {
        // It is what makes the word "recorded" of criterion ③ literally
        // true, and it is what P4 will need to assign a VM.
        base = await baseNeuve('dep-with-user');
        await ouvrirSession(base, 'bureau', 1_000_000, 'u-42');
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.utilisateur_id).toBe('u-42');
        // And nothing else moved.
        expect(Number(ligne.ouverte_a)).toBe(1_000_000);
        expect(ligne.fermee_a).toBeNull();
    });
});

describe(`compterOuvertesDe, engine=${MOTEUR}`, () => {
    /// A real epoch, never a convenient small number: lesson of P1.
    const MS = 1_787_136_773_742;

    it('🔴 the count goes from 0 TO 1 — the TRANSITION is seen', async () => {
        // 🔴 The red: returning a constant. The test must see the count
        // MOVE, not read a number — it is the form of P3's criterion ④,
        // applied here. A test that only asserted `1` would be green on a
        // `return 1`.
        base = await baseNeuve('sess-compte-transition');
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(0);
        await ouvrirSession(base, 'PREFIXE:bureau', MS, 'u-ada');
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(1);
        // 🔴 AND IT IS A NUMBER. Without the `setTypeParser` of
        // `base/pilote-postgres.ts`, a `COUNT(*)` — an `int8` — would come back
        // as a STRING, and `'0' == 0` but `'0' !== 0`. This value
        // belonging to NO column, the column-by-column sweep of
        // `pilotes.test.ts` does not cover it.
        expect(typeof (await compterOuvertesDe(base, 'u-ada'))).toBe('number');
    });

    it('🔴 a CLOSED session is not counted', async () => {
        // 🔴 The red: omitting `AND fermee_a IS NULL`. The count would become
        // a history, and the hub would say "you have an open session" to
        // someone who has had none for weeks.
        base = await baseNeuve('sess-compte-close');
        const id = await ouvrirSession(base, 'PREFIXE:bureau', MS, 'u-ada');
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(1);
        await clore(base, id, MS + 60_000, 'both peers left');
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(0);
    });

    it('🔴 the session of ANOTHER user is not counted', async () => {
        // 🔴 The red: omitting the `WHERE utilisateur_id = ?`. The count
        // would become global, and everyone would see everyone's number of sessions.
        base = await baseNeuve('sess-compte-autrui');
        await ouvrirSession(base, 'PREFIXE:bureau', MS, 'u-bob');
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(0);
        expect(await compterOuvertesDe(base, 'u-bob')).toBe(1);
    });

    it('🔴 a session with a NULL `utilisateur_id` is counted for NOBODY', async () => {
        // 🔴 The red: treating `NULL` as belonging to the requester (for
        // example `utilisateur_id = ? OR utilisateur_id IS NULL`). It is the
        // NOMINAL case of a control session paired by the agent alone — everyone
        // would be assigned the sessions of every VM in the fleet.
        base = await baseNeuve('sess-compte-nul');
        await ouvrirSession(base, 'PREFIXE:bureau', MS);
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(0);
        expect(await compterOuvertesDe(base, '')).toBe(0);
    });
});
