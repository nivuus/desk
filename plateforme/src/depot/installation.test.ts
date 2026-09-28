// Les deux dépôts du téléversement, sous `test:sqlite` ET sous `test:postgres`.
//
// 🔴 LES VALEURS SONT RÉALISTES, JAMAIS COMMODES : les horodatages portent une
// MAGNITUDE D'ÉPOQUE et les tailles celle d'un vrai installeur. C'est la leçon
// la plus chère de P1 — la double passe n'écrivait que des `1_000`, et
// déclarait portable un schéma que Postgres refusait pour toute écriture réelle.
//
// 🔴 CE FICHIER PORTE LES TROIS ROUGES DE LA MIGRATION `0006`, et sans lui elle
// n'en aurait aucune : une migration seule ne peut pas échouer autrement qu'en
// ne s'appliquant pas.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import {
    compterEnCours,
    create as createUpload,
    lireParId as lireTeleversement,
    lirePlusVieuxQue,
    sceller,
    remove,
} from './televersement';
import {
    avancer,
    create as createInstallation,
    lireEnAttentePourVm,
    lireParId as lireInstallation,
    terminer,
} from './installation';

let base: Pilote | undefined;

/// La magnitude qui a réellement cassé Postgres en P1.
const MS = 1_787_136_773_742;
/// Un installeur de 3 Go : au-delà de l'entier 32 bits de Postgres.
const TROIS_GO = 3_221_225_472;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function socle(p: Pilote): Promise<{ user: string; vm: string }> {
    await p.executer(
        'INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
        ['u-1', 'a@b.c', 'scrypt$1$1$1$x$y', MS],
    );
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [
        'v-1',
        'g3',
        '192.168.3.2',
    ]);
    return { user: 'u-1', vm: 'v-1' };
}

describe(`upload repository, engine=${MOTEUR}`, () => {
    it('creates, re-reads, and seals', async () => {
        base = await baseNeuve('tel-cree');
        const { user } = await socle(base);
        const ligne = await createUpload(
            base,
            {
                userId: user,
                nom: 'Firefox Setup 130.0.exe',
                taille: TROIS_GO,
                sha256: 'a'.repeat(64),
                chunkSize: 8 * 1024 * 1024,
            },
            MS,
        );
        expect(ligne.scelle_a).toBeNull();

        const relu = await lireTeleversement(base, ligne.id);
        expect(relu?.nom).toBe('Firefox Setup 130.0.exe');
        expect(relu?.taille).toBe(TROIS_GO);
        expect(relu?.cree_a).toBe(MS);
        expect(relu?.scelle_a).toBeNull();

        await sceller(base, ligne.id, MS + 5_000);
        expect((await lireTeleversement(base, ligne.id))?.scelle_a).toBe(MS + 5_000);
    });

    // 🔴 LA ROUGE DU `BIGINT`, ET ELLE NE SE VOIT QUE SUR LA PASSE POSTGRES.
    // `INTEGER` vaut 8 octets sur SQLite et EXACTEMENT 4 sur Postgres : une
    // taille de 3 Go et un `Date.now()` y débordent tous les deux. C'est le
    // défaut que P1 a trouvé en recette, sur ses PROPRES migrations, et dont
    // les deux gardes existants — le lint lexical et la double passe — ne
    // pouvaient rien voir : `INTEGER` est un type licite, et la suite
    // n'écrivait que des petites valeurs.
    //
    // ⚠️ ET LE TEST COMPARE `typeof`, PAS SEULEMENT LA VALEUR : `pg` rend tout
    // `int8` en TEXTE, et `interroger<T>` fait un `as T[]` — aucun typage ne
    // l'attraperait. C'est le second défaut que P3 a trouvé, de classe, et le
    // remède vit AU PILOTE (`setTypeParser`), jamais dans une rustine locale.
    it('🔴 returns NUMBERS, not strings, on real magnitudes', async () => {
        base = await baseNeuve('tel-nombres');
        const { user } = await socle(base);
        const ligne = await createUpload(
            base,
            {
                userId: user,
                nom: 'gros.msi',
                taille: TROIS_GO,
                sha256: 'b'.repeat(64),
                chunkSize: 8 * 1024 * 1024,
            },
            MS,
        );
        const relu = await lireTeleversement(base, ligne.id);
        expect([typeof relu?.taille, typeof relu?.cree_a, typeof relu?.taille_tranche]).toEqual([
            'number',
            'number',
            'number',
        ]);
        expect(relu?.taille).toBe(TROIS_GO);
    });

    it('counts the IN PROGRESS ones, and a sealed one no longer is', async () => {
        base = await baseNeuve('tel-quota');
        const { user } = await socle(base);
        const a = await createUpload(
            base,
            { userId: user, nom: 'a.exe', taille: 1, sha256: 'c'.repeat(64), chunkSize: 8 },
            MS,
        );
        await createUpload(
            base,
            { userId: user, nom: 'b.exe', taille: 1, sha256: 'd'.repeat(64), chunkSize: 8 },
            MS,
        );
        expect(await compterEnCours(base, user)).toBe(2);
        await sceller(base, a.id, MS + 1);
        expect(await compterEnCours(base, user)).toBe(1);
        // Un autre utilisateur n'entre pas dans le quota.
        expect(await compterEnCours(base, 'u-inconnu')).toBe(0);
    });

    it('the age sweep returns what is older than the bound', async () => {
        base = await baseNeuve('tel-age');
        const { user } = await socle(base);
        const vieux = await createUpload(
            base,
            { userId: user, nom: 'v.exe', taille: 1, sha256: 'e'.repeat(64), chunkSize: 8 },
            MS - 100_000,
        );
        await createUpload(
            base,
            { userId: user, nom: 'n.exe', taille: 1, sha256: 'f'.repeat(64), chunkSize: 8 },
            MS,
        );
        const a_purger = await lirePlusVieuxQue(base, MS - 1);
        expect(a_purger.map((l) => l.id)).toEqual([vieux.id]);
        await remove(base, vieux.id);
        expect(await lireTeleversement(base, vieux.id)).toBeUndefined();
    });

    // 🔴 LA ROUGE DE LA CLÉ ÉTRANGÈRE, PREMIÈRE MOITIÉ. Sans
    // `REFERENCES user(id)`, cette insertion PASSERAIT — et un
    // téléversement orphelin n'appartiendrait à personne, donc échapperait à
    // toute vérification de propriétaire. Les clés étrangères sont APPLIQUÉES
    // des deux côtés : `pilote-sqlite.ts` pose `PRAGMA foreign_keys = ON`.
    it('🔴 REFUSES an upload whose user does not exist', async () => {
        base = await baseNeuve('tel-orphelin');
        await expect(
            createUpload(
                base,
                { userId: 'u-fantome', nom: 'x.exe', taille: 1, sha256: 'g'.repeat(64), chunkSize: 8 },
                MS,
            ),
        ).rejects.toThrow();
    });
});

describe(`installation repository, engine=${MOTEUR}`, () => {
    async function withUpload(p: Pilote): Promise<{ vm: string; tel: string }> {
        const { user, vm } = await socle(p);
        const tel = await createUpload(
            p,
            {
                userId: user,
                nom: 'setup.exe',
                taille: TROIS_GO,
                sha256: 'a'.repeat(64),
                chunkSize: 8 * 1024 * 1024,
            },
            MS,
        );
        await sceller(p, tel.id, MS + 1);
        return { vm, tel: tel.id };
    }

    it('is born pending, advances, then finishes', async () => {
        base = await baseNeuve('inst-cycle');
        const { vm, tel } = await withUpload(base);
        const inst = await createInstallation(base, { vmId: vm, televersementId: tel }, MS);
        expect(inst.etat).toBe('en_attente');
        expect((await lireEnAttentePourVm(base, vm)).map((l) => l.id)).toEqual([inst.id]);

        await avancer(
            base,
            inst.id,
            { phase: 'transfert', octetsFaits: 8_388_608, octetsTotal: TROIS_GO, ecouleMs: 1_200 },
            MS + 2_000,
        );
        const enCours = await lireInstallation(base, inst.id);
        expect(enCours?.etat).toBe('en_cours');
        expect(enCours?.octets_total).toBe(TROIS_GO);
        // 🔴 ET LA RÉÉMISSION S'ARRÊTE : c'est la PREMIÈRE des deux ceintures
        // contre une double exécution.
        expect(await lireEnAttentePourVm(base, vm)).toEqual([]);

        await terminer(
            base,
            inst.id,
            { issue: 'reussie', motif: null, codeSortie: 3010, journal: '', journalTronque: false },
            MS + 9_000,
        );
        const fini = await lireInstallation(base, inst.id);
        expect(fini?.etat).toBe('terminee');
        expect(fini?.issue).toBe('reussie');
        // ⚠️ 3010 EST UN SUCCÈS QUI DEMANDE UN REDÉMARRAGE, et la base le
        // RAPPORTE à côté de l'issue sans en rien déduire.
        expect(fini?.code_sortie).toBe(3010);
        expect(fini?.terminee_a).toBe(MS + 9_000);
    });

    // 🔴 LA ROUGE QUI COMPTE POUR LA RÉÉMISSION. La plateforme RÉÉMET, donc un
    // agent peut rapporter deux fois — et une progression tardive arrivant
    // après l'issue effacerait celle-ci ET remettrait l'état à `en_cours`,
    // c'est-à-dire hors de `terminee`. Le garde est `AND etat <> 'terminee'`
    // sur les DEUX écritures ; sans lui, ce test voit l'issue disparaître.
    it('🔴 a LATE progress does not erase an outcome already set', async () => {
        base = await baseNeuve('inst-tardive');
        const { vm, tel } = await withUpload(base);
        const inst = await createInstallation(base, { vmId: vm, televersementId: tel }, MS);
        await terminer(
            base,
            inst.id,
            { issue: 'sans-effet', motif: null, codeSortie: 0, journal: '', journalTronque: false },
            MS + 1_000,
        );
        await avancer(
            base,
            inst.id,
            { phase: 'execution', octetsFaits: 0, octetsTotal: 0, ecouleMs: 60_000 },
            MS + 2_000,
        );
        const relu = await lireInstallation(base, inst.id);
        expect(relu?.etat).toBe('terminee');
        expect(relu?.issue).toBe('sans-effet');
        expect(relu?.phase).toBe('');
    });

    it('an exit code NOT COLLECTED stays null, never a sentinel', async () => {
        base = await baseNeuve('inst-sans-code');
        const { vm, tel } = await withUpload(base);
        const inst = await createInstallation(base, { vmId: vm, televersementId: tel }, MS);
        await terminer(
            base,
            inst.id,
            {
                issue: 'issue-inconnue',
                motif: null,
                codeSortie: null,
                journal: 'last line',
                journalTronque: true,
            },
            MS + 1,
        );
        const relu = await lireInstallation(base, inst.id);
        expect(relu?.code_sortie).toBeNull();
        expect(relu?.journal_tronque).toBeTruthy();
    });

    // 🔴 LA ROUGE DE LA CLÉ ÉTRANGÈRE, SECONDE MOITIÉ — et c'est celle que le
    // plan nomme : « l'insertion d'une installation pour une VM inexistante
    // PASSE au lieu d'échouer ».
    it('🔴 REFUSES an installation for a non-existent VM', async () => {
        base = await baseNeuve('inst-vm-fantome');
        const { tel } = await withUpload(base);
        await expect(
            createInstallation(base, { vmId: 'v-fantome', televersementId: tel }, MS),
        ).rejects.toThrow();
    });

    it('🔴 REFUSES an installation for a non-existent upload', async () => {
        base = await baseNeuve('inst-tel-fantome');
        const { vm } = await withUpload(base);
        await expect(
            createInstallation(base, { vmId: vm, televersementId: 't-fantome' }, MS),
        ).rejects.toThrow();
    });

    // ⚠️ CE REFUS EST VOULU, et il est le pendant de l'absence d'`ON DELETE` :
    // l'historique d'une installation doit rester lisible. Les TRANCHES du
    // disque, elles, sont balayées par ailleurs — ce sont elles qui coûtent de
    // la place, pas la ligne.
    it('🔴 REFUSES to delete an upload that an installation references', async () => {
        base = await baseNeuve('inst-fk-refus');
        const { vm, tel } = await withUpload(base);
        await createInstallation(base, { vmId: vm, televersementId: tel }, MS);
        await expect(remove(base, tel)).rejects.toThrow();
    });
});
