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
import { creerUtilisateur } from './utilisateur';
import { DUREE_RAFRAICHISSEMENT_MS, emettre, revoquerFamille, tourner } from './jeton';

const MS = 1_787_136_773_742;

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function avecUtilisateur(nom: string): Promise<{ p: Pilote; id: string }> {
    const p = await baseNeuve(nom);
    base = p;
    // L'empreinte importe peu ici, mais sa LONGUEUR est celle du réel.
    const id = await creerUtilisateur(
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

describe(`dépôt jeton_rafraichissement, moteur=${MOTEUR}`, () => {
    it('émet un clair NEUF à chaque appel, et la base ne porte JAMAIS le clair', async () => {
        const { p, id } = await avecUtilisateur('jet-emettre');
        const un = await emettre(p, id, MS);
        const deux = await emettre(p, id, MS);
        expect(deux).not.toBe(un);

        // Le clair ne doit apparaître dans AUCUNE colonne : une fuite de la
        // base ne doit pas rendre les jetons utilisables.
        const toutes = await lignes(p);
        expect(toutes).toHaveLength(2);
        for (const l of toutes) {
            expect(l.empreinte).not.toBe(un);
            expect(l.empreinte).not.toBe(deux);
        }
        expect(JSON.stringify(toutes)).not.toContain(un);
        expect(JSON.stringify(toutes)).not.toContain(deux);
    });

    it('écrit expire_a à l’époque EXACTE attendue, sur ce moteur', async () => {
        const { p, id } = await avecUtilisateur('jet-epoque');
        await emettre(p, id, MS);
        const [l] = await lignes(p);
        // 🔴 Valeur exacte, de magnitude d'époque : c'est l'assertion qui
        // aurait rougi sur une colonne INTEGER côté Postgres.
        expect(Number(l.expire_a)).toBe(MS + DUREE_RAFRAICHISSEMENT_MS);
    });

    it('tourne : le neuf vaut, l’ancien ne vaut plus, et la famille est la même', async () => {
        const { p, id } = await avecUtilisateur('jet-tourner');
        const un = await emettre(p, id, MS);
        const issue = await tourner(p, un, MS + 1_000);
        expect(issue.ok).toBe(true);
        if (!issue.ok) return;
        expect(issue.clair).not.toBe(un);
        expect(issue.utilisateurId).toBe(id);

        const toutes = await lignes(p);
        expect(toutes).toHaveLength(2);
        // Une SEULE famille : le lien est ce qui permettra de tout révoquer.
        expect(new Set(toutes.map((l) => l.famille)).size).toBe(1);
        // Le neuf tourne à son tour ; l'ancien est mort.
        await expect(tourner(p, issue.clair, MS + 2_000)).resolves.toMatchObject({ ok: true });
    });

    it('REJEU : tourner deux fois le même clair révoque TOUTE la famille', async () => {
        const { p, id } = await avecUtilisateur('jet-rejeu');
        const un = await emettre(p, id, MS);
        const issue = await tourner(p, un, MS + 1_000);
        expect(issue.ok).toBe(true);

        // Le voleur présente le clair déjà tourné.
        expect(await tourner(p, un, MS + 2_000)).toEqual({ ok: false, motif: 'rejeu' });

        // 🔴 Et le jeton NEUF, que le voleur détient, est mort lui aussi.
        const toutes = await lignes(p);
        expect(toutes).toHaveLength(2);
        for (const l of toutes) expect(l.revoque_a).not.toBeNull();
    });

    it('une famille révoquée refuse AUSSI le jeton neuf, et le motif le DIT', async () => {
        // 🔴 `revoque` et `rejeu` sont distingués par `remplace_par` : une
        // ligne révoquée SANS successeur n'a jamais été tournée, donc la
        // présenter n'est pas un rejeu — c'est un jeton mort. Sans cette
        // distinction, le motif `revoque` serait une variante inatteignable.
        const { p, id } = await avecUtilisateur('jet-famille');
        const un = await emettre(p, id, MS);
        const issue = await tourner(p, un, MS + 1_000);
        expect(issue.ok).toBe(true);
        if (!issue.ok) return;

        const [l] = await lignes(p);
        expect(await revoquerFamille(p, l.famille, MS + 1_500)).toBe(1);
        expect(await tourner(p, issue.clair, MS + 2_000)).toEqual({ ok: false, motif: 'revoque' });
    });

    it('refuse un clair inconnu, et un jeton expiré — sur une horloge qui VARIE', async () => {
        const { p, id } = await avecUtilisateur('jet-expire');
        expect(await tourner(p, 'un-clair-qui-n-a-jamais-existe', MS)).toEqual({
            ok: false,
            motif: 'inconnu',
        });

        const un = await emettre(p, id, MS);
        // Avant l'échéance : accepté.
        const avant = await tourner(p, un, MS + DUREE_RAFRAICHISSEMENT_MS - 1);
        expect(avant.ok).toBe(true);
        if (!avant.ok) return;
        // 🔴 Trois instants distincts : une horloge figée rendrait ce test
        // inerte. À l'échéance EXACTE, le refus est franc.
        expect(await tourner(p, avant.clair, MS + 2 * DUREE_RAFRAICHISSEMENT_MS)).toEqual({
            ok: false,
            motif: 'expire',
        });
    });

    it('annule la TRANSACTION si l’insertion neuve échoue : l’ancien reste valide', async () => {
        // 🔴 Hors transaction, la révocation de l'ancien serait déjà écrite
        // quand l'insertion échouerait : l'utilisateur perdrait sa session sur
        // une panne partielle, sans qu'aucune erreur ne le lui dise.
        const { p, id } = await avecUtilisateur('jet-rollback');
        const un = await emettre(p, id, MS);
        const deux = await emettre(p, id, MS);

        // Le générateur de test rend un clair DÉJÀ employé : l'index UNIQUE
        // sur `empreinte` refuse l'insertion neuve.
        await expect(tourner(p, un, MS + 1_000, () => deux)).rejects.toThrow();

        // L'ancien n'a pas été révoqué : la transaction a tout annulé.
        const toutes = await lignes(p);
        expect(toutes).toHaveLength(2);
        for (const l of toutes) expect(l.revoque_a).toBeNull();
        await expect(tourner(p, un, MS + 2_000)).resolves.toMatchObject({ ok: true });
    });
});
