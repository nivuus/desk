// The background cleanup of the two stores, under `test:sqlite` AND under
// `test:postgres` — like everything that touches the database.
//
// 🔴 THIS FILE IS THE ANSWER TO CORRECTION ROUND 1: `evincer` of the two
// stores was a mechanism written, tested, documented — and called by
// NOBODY. Here, the set of references comes from the REAL DATABASE, never
// from a hand-built parameter: an `unTour` that received an empty
// set would evict everything old, including what is in use — the
// exact corruption the floor exists to prevent. The test that proves
// that NOBODY CALLS `evincer` ANY MORE — the precise defect of round 1 — lives
// in `http/serveur.test.ts`, because it is `startServer` that must
// do it, not this file.

import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, rmSync, utimesSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { appliquer } from '../depot/application';
import {
    create as createUpload,
    lireParId as lireTeleversement,
} from '../depot/televersement';
import { create as createInstallation } from '../depot/installation';
import type { Application } from '../../../proto/ts/plateforme-apps';
import { ouvrirMagasin, type Magasin } from './icones';
import { ouvrirMagasinTranches, type MagasinTranches } from './magasin-tranches';
import { startCleanup, referencesIcones, referencesTranches, unTour } from './nettoyage';

let base: Pilote | undefined;
let racines: string[] = [];

afterEach(async () => {
    await base?.fermer();
    base = undefined;
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
});

/// The magnitude that really broke Postgres in P1 — same value as
/// `depot/application.test.ts` and `depot/installation.test.ts`, to stay
/// in the same family of fixtures.
const MS = 1_787_136_773_742;
const JOUR_MS = 24 * 60 * 60_000;

function magasinIconesNeuf(): Magasin {
    const r = mkdtempSync(join(tmpdir(), 'nettoyage-icones-'));
    racines.push(r);
    return ouvrirMagasin(join(r, 'icones'), () => {});
}

function magasinTranchesNeuf(): MagasinTranches {
    const r = mkdtempSync(join(tmpdir(), 'nettoyage-tranches-'));
    racines.push(r);
    return ouvrirMagasinTranches(join(r, 'televersements'), () => {});
}

async function withVm(p: Pilote, id: string): Promise<void> {
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [id, `vm-${id}`, '192.168.3.2']);
}

async function withUser(p: Pilote, id: string): Promise<void> {
    await p.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)', [
        id,
        `${id}@exemple.test`,
        'scrypt$1$1$1$x$y',
        MS,
    ]);
}

function icone(texte: string): { empreinte: string; octets: Buffer } {
    const octets = Buffer.from(texte);
    return { empreinte: createHash('sha256').update(octets).digest('hex'), octets };
}

function appWithIcon(nom: string, cle: string, empreinteIcone: string | null): Application {
    return {
        cle,
        nom,
        chemin: `C:\\Users\\guacamole\\Desktop\\${nom}.lnk`,
        cible: `c:\\program files\\${nom}\\${nom}.exe`,
        arguments: '',
        repertoire: `c:\\program files\\${nom}`,
        icone: empreinteIcone,
        source_max: 'non-mesuree',
        accent: null,
        associations: [],
    };
}

/// Forces the last-modified date of a path — time is INJECTED
/// into the age measurement, never read from the clock: same rule as `icones.test.ts`
/// and `magasin-tranches.test.ts`.
function vieillir(chemin: string, quandMs: number): void {
    utimesSync(chemin, new Date(quandMs), new Date(quandMs));
}

/// A single-chunk stream — enough for this file, which does not test
/// the chunking itself (see `magasin-tranches.test.ts` for that).
function flux(s: string): AsyncIterable<Uint8Array> {
    return (async function* () {
        yield Buffer.from(s);
    })();
}

describe(`background clean-up, engine=${MOTEUR}`, () => {
    it('🔴 referencesIcones COMES FROM THE DATABASE: a live entry returns a NON-EMPTY set', async () => {
        // The guard of the danger named by round 2: an EMPTY set while
        // a live application exists would evict this icon — it is
        // exactly the corruption the floor exists to prevent.
        base = await baseNeuve('nettoyage-refs-icones-non-vide');
        await withVm(base, 'v1');
        const { empreinte } = icone('vivante');
        await appliquer(
            base,
            'v1',
            { aInserer: [appWithIcon('X', 'a'.repeat(64), empreinte)], toUpdate: [], aMarquerDisparues: [], aRessusciter: [] },
            MS,
        );
        const refs = await referencesIcones(base);
        expect(refs.size).toBeGreaterThan(0);
        expect(refs).toEqual(new Set([empreinte]));
    });

    it('a GONE entry no longer references its icon', async () => {
        base = await baseNeuve('nettoyage-refs-icones-disparue');
        await withVm(base, 'v1');
        const { empreinte } = icone('feu-vivante');
        const cle = 'b'.repeat(64);
        await appliquer(
            base,
            'v1',
            { aInserer: [appWithIcon('X', cle, empreinte)], toUpdate: [], aMarquerDisparues: [], aRessusciter: [] },
            MS,
        );
        const [{ id }] = await base.interroger<{ id: string }>(
            'SELECT id FROM application WHERE cle = ?',
            [cle],
        );
        await appliquer(base, 'v1', { aInserer: [], toUpdate: [], aMarquerDisparues: [id], aRessusciter: [] }, MS + 1);
        expect(await referencesIcones(base)).toEqual(new Set());
    });

    it('a HIDDEN entry STILL references its icon — hiding is not disappearing', async () => {
        base = await baseNeuve('nettoyage-refs-icones-masquee');
        await withVm(base, 'v1');
        const { empreinte } = icone('masquable');
        const cle = 'c'.repeat(64);
        await appliquer(
            base,
            'v1',
            { aInserer: [appWithIcon('X', cle, empreinte)], toUpdate: [], aMarquerDisparues: [], aRessusciter: [] },
            MS,
        );
        const [{ id }] = await base.interroger<{ id: string }>(
            'SELECT id FROM application WHERE cle = ?',
            [cle],
        );
        await base.executer('UPDATE application SET masquee_a = ? WHERE id = ?', [MS + 1, id]);
        // ⚠️ IF THIS TEST TURNS RED BECAUSE THE SET IS EMPTY, THE RULE IS
        // WRONG IN THE DANGEROUS DIRECTION: it would evict the icon of an
        // application the user can still unhide.
        expect(await referencesIcones(base)).toEqual(new Set([empreinte]));
    });

    it('a REAL round, against the database: old+referenced stays, old+orphan goes, young stays', async () => {
        base = await baseNeuve('nettoyage-tour-icones');
        await withVm(base, 'v1');
        const enService = icone('en-service-tour');
        await appliquer(
            base,
            'v1',
            {
                aInserer: [appWithIcon('X', 'd'.repeat(64), enService.empreinte)],
                toUpdate: [],
                aMarquerDisparues: [],
                aRessusciter: [],
            },
            MS,
        );

        const magasin = magasinIconesNeuf();
        const orpheline = icone('orpheline-tour');
        const recente = icone('recente-tour');
        magasin.write(enService.empreinte, enService.octets);
        magasin.write(orpheline.empreinte, orpheline.octets);
        magasin.write(recente.empreinte, recente.octets);
        vieillir(join(magasin.repertoire, enService.empreinte), 0);
        vieillir(join(magasin.repertoire, orpheline.empreinte), 0);
        // `recente` keeps its real write date (now): young by
        // construction, with no need to force it.

        const tranches = magasinTranchesNeuf();
        await unTour({ base, magasin, tranches, maintenant: 400 * JOUR_MS });

        expect(magasin.possede(enService.empreinte)).toBe(true);
        expect(magasin.possede(orpheline.empreinte)).toBe(false);
        expect(magasin.possede(recente.empreinte)).toBe(true);
    });

    it('🔴 referencesTranches COMES FROM THE DATABASE: an existing row returns a NON-EMPTY set', async () => {
        base = await baseNeuve('nettoyage-refs-tranches-non-vide');
        await withUser(base, 'u1');
        const t = await createUpload(
            base,
            { userId: 'u1', nom: 'x.exe', taille: 1, sha256: 'e'.repeat(64), chunkSize: 8 },
            MS,
        );
        const refs = await referencesTranches(base);
        expect(refs.size).toBeGreaterThan(0);
        expect(refs).toEqual(new Set([t.id]));
    });

    it(
        '🔴 a REAL round, against the database: the installation foreign key PROTECTS ' +
            'the row — and so the disk —, a truly orphan upload loses BOTH',
        async () => {
            base = await baseNeuve('nettoyage-tour-tranches');
            await withUser(base, 'u1');
            await withVm(base, 'v1');

            // A: old, but an installation still references it.
            const a = await createUpload(
                base,
                { userId: 'u1', nom: 'a.exe', taille: 1, sha256: 'f'.repeat(64), chunkSize: 8 },
                MS - 400 * JOUR_MS,
            );
            await createInstallation(base, { vmId: 'v1', televersementId: a.id }, MS);

            // B: old, and nobody references it — the ORPHAN case.
            const b = await createUpload(
                base,
                { userId: 'u1', nom: 'b.exe', taille: 1, sha256: 'a'.repeat(64), chunkSize: 8 },
                MS - 400 * JOUR_MS,
            );

            // C: young — protected by its age, regardless of any reference.
            const c = await createUpload(
                base,
                { userId: 'u1', nom: 'c.exe', taille: 1, sha256: 'b'.repeat(64), chunkSize: 8 },
                MS,
            );

            const tranches = magasinTranchesNeuf();
            await tranches.write(a.id, 0, flux('a'), 1024);
            await tranches.write(b.id, 0, flux('b'), 1024);
            await tranches.write(c.id, 0, flux('c'), 1024);
            vieillir(join(tranches.racine, a.id), 0);
            vieillir(join(tranches.racine, b.id), 0);
            // `c` keeps its real write date: young by construction.

            const magasin = magasinIconesNeuf();
            await unTour({ base, magasin, tranches, maintenant: MS });

            // A: the row SURVIVES (foreign key refusal), so does the disk.
            expect(await lireTeleversement(base, a.id)).toBeDefined();
            expect(existsSync(join(tranches.racine, a.id))).toBe(true);

            // B: row AND disk disappear — the real orphan.
            expect(await lireTeleversement(base, b.id)).toBeUndefined();
            expect(existsSync(join(tranches.racine, b.id))).toBe(false);

            // C: too young to even be a candidate for the row purge.
            expect(await lireTeleversement(base, c.id)).toBeDefined();
            expect(existsSync(join(tranches.racine, c.id))).toBe(true);
        },
    );

    // 🔴 THE CRITICAL OF CORRECTION ROUND 2 — THE TWO FLOORS DID NOT
    // MEASURE THE SAME AGE. The ROW purge filtered on `cree_a`;
    // the DISK eviction filters on `mtime`, the LAST ACTIVITY — it
    // is NOT the same thing, and the gap is DETERMINISTIC, not a race:
    // an upload created 31 days ago with a chunk that just
    // arrived AT THIS INSTANT saw its row deleted (candidate by
    // `cree_a`, no installation referencing it) while its
    // directory stayed — the resume dies, the disk is not even
    // freed. This test replays EXACTLY this scenario.
    it(
        '🔴 CRITICAL: an upload CREATED 31 days ago, one chunk of which ' +
            'has just ARRIVED, keeps ITS ROW AND ITS DISK',
        async () => {
            base = await baseNeuve('nettoyage-critique-planchers');
            await withUser(base, 'u1');

            const maintenant = MS;
            const t = await createUpload(
                base,
                {
                    userId: 'u1',
                    nom: 'actif.exe',
                    taille: 1,
                    sha256: 'c'.repeat(64),
                    chunkSize: 8,
                },
                maintenant - 31 * JOUR_MS,
            );

            const tranches = magasinTranchesNeuf();
            await tranches.write(t.id, 0, flux('x'), 1024);
            // The chunk that "just arrived": its activity date is FORCED
            // to `maintenant`, never left to age — it is what
            // distinguishes this scenario from the real orphan case (the test above).
            vieillir(join(tranches.racine, t.id), maintenant);

            const magasin = magasinIconesNeuf();
            await unTour({ base, magasin, tranches, maintenant });

            // BEFORE THE FIX: the row would have disappeared (31 days > the 30
            // of the ROW floor) while the disk stayed intact (the
            // DISK floor, for its part, sees it as young) — the deterministic
            // corruption. AFTER: the two floors agree, and
            // BOTH survive.
            expect(await lireTeleversement(base, t.id)).toBeDefined();
            expect(existsSync(join(tranches.racine, t.id))).toBe(true);
        },
    );

    // ⚠️ Important ① (correction round 2): the foreign key refusal —
    // the ONLY EXPECTED failure of the row purge — stays SILENT; everything
    // else is logged. The two arms of the same mechanism, in two
    // separate tests so they are not confused.
    it('a foreign key refusal (EXPECTED) logs NOTHING', async () => {
        base = await baseNeuve('nettoyage-fk-muet');
        await withUser(base, 'u1');
        await withVm(base, 'v1');
        const t = await createUpload(
            base,
            { userId: 'u1', nom: 'ref.exe', taille: 1, sha256: 'e'.repeat(64), chunkSize: 8 },
            MS - 400 * JOUR_MS,
        );
        await createInstallation(base, { vmId: 'v1', televersementId: t.id }, MS);

        const tranches = magasinTranchesNeuf();
        await tranches.write(t.id, 0, flux('x'), 1024);
        vieillir(join(tranches.racine, t.id), 0);

        const espion = vi.spyOn(console, 'error').mockImplementation(() => {});
        try {
            const magasin = magasinIconesNeuf();
            await unTour({ base, magasin, tranches, maintenant: MS });
            expect(espion).not.toHaveBeenCalled();
        } finally {
            espion.mockRestore();
        }
    });

    it('🔴 a purge failure that IS NOT a foreign key GETS LOGGED', async () => {
        base = await baseNeuve('nettoyage-fk-pas-muet');
        await withUser(base, 'u1');
        // Really orphaned — nothing references it, the deletion of its
        // row WOULD normally SUCCEED (see the test "a REAL ROUND…" further
        // up): it is what makes the fault injected below attributable
        // to the `catch`, never to the foreign key.
        const t = await createUpload(
            base,
            { userId: 'u1', nom: 'orph.exe', taille: 1, sha256: 'f'.repeat(64), chunkSize: 8 },
            MS - 400 * JOUR_MS,
        );
        const tranches = magasinTranchesNeuf();
        await tranches.write(t.id, 0, flux('x'), 1024);
        vieillir(join(tranches.racine, t.id), 0);

        // A driver that makes the DELETE SPECIFICALLY fail, for a cause
        // UNRELATED to a foreign key — the mutation played by the
        // review itself ("database cut"), reproduced here as a fixture.
        const basePannee: Pilote = {
            ...base,
            async executer(sql, params) {
                if (sql.startsWith('DELETE FROM televersement')) {
                    throw new Error('database cut — nothing to do with a foreign key');
                }
                return base!.executer(sql, params);
            },
        };

        const espion = vi.spyOn(console, 'error').mockImplementation(() => {});
        try {
            const magasin = magasinIconesNeuf();
            await unTour({ base: basePannee, magasin, tranches, maintenant: MS });
            expect(espion).toHaveBeenCalledTimes(1);
            expect(espion.mock.calls[0][0]).toContain(t.id);
            expect(espion.mock.calls[0][0]).toContain('database cut');
            // The row was NOT deleted: the failure did prevent the
            // deletion, it did not merely make it silent.
            expect(await lireTeleversement(base, t.id)).toBeDefined();
        } finally {
            espion.mockRestore();
        }
    });

    // ⚠️ Important ③ (correction round 2): `arreter()` really
    // prevents the NEXT rounds. `clearInterval` does not interrupt a
    // round already in flight — this test proves ONLY the absence of rounds after
    // the call, not the interruption of a round in progress, exactly what the
    // corrected comment of `arreter()` promises and nothing more.
    it('🔴 arreter() REALLY prevents the NEXT rounds', async () => {
        base = await baseNeuve('nettoyage-arret-reel');
        let tours = 0;
        const magasinReel = magasinIconesNeuf();
        const magasin: typeof magasinReel = {
            ...magasinReel,
            async evincer(opts) {
                tours += 1;
                return magasinReel.evincer(opts);
            },
        };
        const tranches = magasinTranchesNeuf();

        const nettoyage = await startCleanup(
            { base, magasin, tranches, maintenant: () => MS },
            20, // tiny periodeMs — a test, never a shipped value.
        );
        expect(tours).toBe(1); // le premier tour, ATTENDU.

        nettoyage.arreter();
        // Much longer than `periodeMs` (20 ms): if `arreter()`
        // prevented nothing, several more rounds would have had time to
        // run in this window.
        await new Promise((resolve) => setTimeout(resolve, 150));
        expect(tours).toBe(1);
    });

    // 🔴 HARDENING (correction round 3): a MALFORMED row (id that
    // is not a UUID — NOT REACHABLE today, `create` being the only
    // `INSERT`, but provoked here directly in SQL) must NOT make the
    // whole round give up. The return order of `lirePlusVieuxQue`
    // not being guaranteed, this test assumes NO order: whether the faulty
    // row is seen before or after the legitimate orphan, the latter must,
    // in every case, end up purged — row AND disk.
    it('a malformed row does not abandon the round: the neighbouring orphan is evicted anyway', async () => {
        base = await baseNeuve('nettoyage-durcissement-id-malforme');
        await withUser(base, 'u1');

        const vieux = MS - 400 * JOUR_MS;
        // The FAULTY row, set directly in SQL — never through `create`.
        await base.executer(
            'INSERT INTO televersement(id,utilisateur_id,nom,taille,sha256,taille_tranche,cree_a,scelle_a)'
                + ' VALUES(?,?,?,?,?,?,?,?)',
            ['not-a-uuid', 'u1', 'fautif.exe', 1, 'a'.repeat(64), 8, vieux, null],
        );
        // The LEGITIMATE orphan, through the normal path.
        const orpheline = await createUpload(
            base,
            { userId: 'u1', nom: 'orph.exe', taille: 1, sha256: 'b'.repeat(64), chunkSize: 8 },
            vieux,
        );

        const tranches = magasinTranchesNeuf();
        await tranches.write(orpheline.id, 0, flux('x'), 1024);
        vieillir(join(tranches.racine, orpheline.id), 0);

        const magasin = magasinIconesNeuf();
        await unTour({ base, magasin, tranches, maintenant: MS });

        expect(await lireTeleversement(base, orpheline.id)).toBeUndefined();
        expect(existsSync(join(tranches.racine, orpheline.id))).toBe(false);
    });
});
