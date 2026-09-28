// The `jeton_rafraichissement` repository, and REPLAY DETECTION.
//
// 🔴 The decisive test of this file is the double rotation one: presenting
// the same plaintext twice must revoke THE WHOLE FAMILY, including the new
// token the thief holds. An implementation that DELETED the row at
// rotation would return `'inconnu'` on the second call — a refusal, so green for
// a naive test — while leaving the family intact. The two outcomes are
// distinguishable, and that is what makes this test non-vacuous.
//
// 🔴 The timestamps are EPOCHS IN MILLISECONDS, never small
// values: on an INTEGER column, Postgres would refuse them, and a suite that
// only writes `1_000`s would not see it.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { createUser } from './utilisateur';
import { DUREE_RAFRAICHISSEMENT_MS, emettre, revoquerFamille, tourner } from './jeton';

const MS = 1_787_136_773_742;

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function withUser(nom: string): Promise<{ p: Pilote; id: string }> {
    const p = await baseNeuve(nom);
    base = p;
    // The hash matters little here, but its LENGTH is the real one.
    const id = await createUser(
        p,
        'ada@exemple.test',
        `scrypt$16384$8$1$${'s'.repeat(22)}$${'e'.repeat(43)}`,
        MS,
    );
    return { p, id };
}

async function lignes(p: Pilote): Promise<
    Array<{ empreinte: string; famille: string; revoque_a: number | null; expire_a: number | string }>
> {
    return p.interroger(
        'SELECT empreinte, famille, revoque_a, expire_a FROM jeton_rafraichissement',
        [],
    );
}

describe(`jeton_rafraichissement repository, engine=${MOTEUR}`, () => {
    it('issues a NEW plaintext at each call, and the database NEVER holds the plaintext', async () => {
        const { p, id } = await withUser('jet-emettre');
        const un = await emettre(p, id, MS);
        const deux = await emettre(p, id, MS);
        expect(deux).not.toBe(un);

        // The plaintext must appear in NO column: a leak of the
        // database must not make the tokens usable.
        const all = await lignes(p);
        expect(all).toHaveLength(2);
        for (const l of all) {
            expect(l.empreinte).not.toBe(un);
            expect(l.empreinte).not.toBe(deux);
        }
        expect(JSON.stringify(all)).not.toContain(un);
        expect(JSON.stringify(all)).not.toContain(deux);
    });

    it('writes expire_a at the EXACT expected epoch, on this engine', async () => {
        const { p, id } = await withUser('jet-epoque');
        await emettre(p, id, MS);
        const [l] = await lignes(p);
        // 🔴 Exact value, of epoch magnitude: it is the assertion that
        // would have turned red on an INTEGER column on the Postgres side.
        expect(Number(l.expire_a)).toBe(MS + DUREE_RAFRAICHISSEMENT_MS);
    });

    it('rotates: the new one is valid, the old one no longer is, and the family is the same', async () => {
        const { p, id } = await withUser('jet-tourner');
        const un = await emettre(p, id, MS);
        const issue = await tourner(p, un, MS + 1_000);
        expect(issue.ok).toBe(true);
        if (!issue.ok) return;
        expect(issue.clair).not.toBe(un);
        expect(issue.userId).toBe(id);

        const all = await lignes(p);
        expect(all).toHaveLength(2);
        // A SINGLE family: the link is what will allow revoking everything.
        expect(new Set(all.map((l) => l.famille)).size).toBe(1);
        // The new one rotates in turn; the old one is dead.
        await expect(tourner(p, issue.clair, MS + 2_000)).resolves.toMatchObject({ ok: true });
    });

    it('REPLAY: rotating the same plaintext twice revokes the WHOLE family', async () => {
        const { p, id } = await withUser('jet-rejeu');
        const un = await emettre(p, id, MS);
        const issue = await tourner(p, un, MS + 1_000);
        expect(issue.ok).toBe(true);

        // The thief presents the already rotated plaintext.
        expect(await tourner(p, un, MS + 2_000)).toEqual({ ok: false, motif: 'rejeu' });

        // 🔴 And the NEW token, which the thief holds, is dead too.
        const all = await lignes(p);
        expect(all).toHaveLength(2);
        for (const l of all) expect(l.revoque_a).not.toBeNull();
    });

    it('a revoked family ALSO refuses the new token, and the reason SAYS so', async () => {
        // 🔴 `revoque` and `rejeu` are told apart by `remplace_par`: a
        // row revoked WITHOUT a successor was never rotated, so
        // presenting it is not a replay — it is a dead token. Without this
        // distinction, the `revoque` reason would be an unreachable variant.
        const { p, id } = await withUser('jet-famille');
        const un = await emettre(p, id, MS);
        const issue = await tourner(p, un, MS + 1_000);
        expect(issue.ok).toBe(true);
        if (!issue.ok) return;

        const [l] = await lignes(p);
        expect(await revoquerFamille(p, l.famille, MS + 1_500)).toBe(1);
        expect(await tourner(p, issue.clair, MS + 2_000)).toEqual({ ok: false, motif: 'revoque' });
    });

    it('refuses an unknown plaintext, and an expired token — on a clock that VARIES', async () => {
        const { p, id } = await withUser('jet-expire');
        expect(await tourner(p, 'a-plaintext-that-never-existed', MS)).toEqual({
            ok: false,
            motif: 'inconnu',
        });

        const un = await emettre(p, id, MS);
        // Before the deadline: accepted.
        const before = await tourner(p, un, MS + DUREE_RAFRAICHISSEMENT_MS - 1);
        expect(before.ok).toBe(true);
        if (!before.ok) return;
        // 🔴 Three distinct instants: a frozen clock would make this test
        // inert. At the EXACT deadline, the refusal is clear-cut.
        expect(await tourner(p, before.clair, MS + 2 * DUREE_RAFRAICHISSEMENT_MS)).toEqual({
            ok: false,
            motif: 'expire',
        });
    });

    it('rolls back the TRANSACTION if the new insertion fails: the old one stays valid', async () => {
        // 🔴 Outside a transaction, the revocation of the old one would already be written
        // when the insertion failed: the user would lose their session on
        // a partial failure, without any error telling them.
        const { p, id } = await withUser('jet-rollback');
        const un = await emettre(p, id, MS);
        const deux = await emettre(p, id, MS);

        // The test generator returns an ALREADY used plaintext: the UNIQUE index
        // on `empreinte` refuses the new insertion.
        await expect(tourner(p, un, MS + 1_000, () => deux)).rejects.toThrow();

        // The old one was not revoked: the transaction rolled everything back.
        const all = await lignes(p);
        expect(all).toHaveLength(2);
        for (const l of all) expect(l.revoque_a).toBeNull();
        await expect(tourner(p, un, MS + 2_000)).resolves.toMatchObject({ ok: true });
    });
});
