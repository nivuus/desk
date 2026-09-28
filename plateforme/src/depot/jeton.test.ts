// Le dépôt `jeton_rafraichissement`, et la DÉTECTION DE REJEU.
//
// 🔴 Le test décisif de ce fichier est celui de la double rotation : présenter
// deux fois le même clair doit révoquer TOUTE LA FAMILLE, y compris le jeton
// neuf que le voleur détient. Une implémentation qui SUPPRIMERAIT la ligne à
// la rotation rendrait `'inconnu'` au second appel — un refus, donc vert pour
// un test naïf — en laissant la famille intacte. Les deux issues sont
// distinguables, et c'est ce qui rend ce test non vacueux.
//
// 🔴 Les horodatages sont des ÉPOQUES EN MILLISECONDES, jamais de petites
// valeurs : sur une colonne INTEGER, Postgres les refuserait, et une suite qui
// n'écrit que des `1_000` ne le verrait pas.

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
    // L'empreinte importe peu ici, mais sa LONGUEUR est celle du réel.
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

        // Le clair ne doit apparaître dans AUCUNE colonne : une fuite de la
        // base ne doit pas rendre les jetons utilisables.
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
        // 🔴 Valeur exacte, de magnitude d'époque : c'est l'assertion qui
        // aurait rougi sur une colonne INTEGER côté Postgres.
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
        // Une SEULE famille : le lien est ce qui permettra de tout révoquer.
        expect(new Set(all.map((l) => l.famille)).size).toBe(1);
        // Le neuf tourne à son tour ; l'ancien est mort.
        await expect(tourner(p, issue.clair, MS + 2_000)).resolves.toMatchObject({ ok: true });
    });

    it('REPLAY: rotating the same plaintext twice revokes the WHOLE family', async () => {
        const { p, id } = await withUser('jet-rejeu');
        const un = await emettre(p, id, MS);
        const issue = await tourner(p, un, MS + 1_000);
        expect(issue.ok).toBe(true);

        // Le voleur présente le clair déjà tourné.
        expect(await tourner(p, un, MS + 2_000)).toEqual({ ok: false, motif: 'rejeu' });

        // 🔴 Et le jeton NEUF, que le voleur détient, est mort lui aussi.
        const all = await lignes(p);
        expect(all).toHaveLength(2);
        for (const l of all) expect(l.revoque_a).not.toBeNull();
    });

    it('a revoked family ALSO refuses the new token, and the reason SAYS so', async () => {
        // 🔴 `revoque` et `rejeu` sont distingués par `remplace_par` : une
        // ligne révoquée SANS successeur n'a jamais été tournée, donc la
        // présenter n'est pas un rejeu — c'est un jeton mort. Sans cette
        // distinction, le motif `revoque` serait une variante inatteignable.
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
        // Avant l'échéance : accepté.
        const before = await tourner(p, un, MS + DUREE_RAFRAICHISSEMENT_MS - 1);
        expect(before.ok).toBe(true);
        if (!before.ok) return;
        // 🔴 Trois instants distincts : une horloge figée rendrait ce test
        // inerte. À l'échéance EXACTE, le refus est franc.
        expect(await tourner(p, before.clair, MS + 2 * DUREE_RAFRAICHISSEMENT_MS)).toEqual({
            ok: false,
            motif: 'expire',
        });
    });

    it('rolls back the TRANSACTION if the new insertion fails: the old one stays valid', async () => {
        // 🔴 Hors transaction, la révocation de l'ancien serait déjà écrite
        // quand l'insertion échouerait : l'utilisateur perdrait sa session sur
        // une panne partielle, sans qu'aucune erreur ne le lui dise.
        const { p, id } = await withUser('jet-rollback');
        const un = await emettre(p, id, MS);
        const deux = await emettre(p, id, MS);

        // Le générateur de test rend un clair DÉJÀ employé : l'index UNIQUE
        // sur `empreinte` refuse l'insertion neuve.
        await expect(tourner(p, un, MS + 1_000, () => deux)).rejects.toThrow();

        // L'ancien n'a pas été révoqué : la transaction a tout annulé.
        const all = await lignes(p);
        expect(all).toHaveLength(2);
        for (const l of all) expect(l.revoque_a).toBeNull();
        await expect(tourner(p, un, MS + 2_000)).resolves.toMatchObject({ ok: true });
    });
});
