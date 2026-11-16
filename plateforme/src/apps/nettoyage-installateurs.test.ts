// The purge of installer BYTES once every installation that used them is
// finished — against BOTH engines, with a real database and a real disk store.
//
// 🔴 THE DEFECT THIS FILE PINS: an `installation` row references its upload
// by a foreign key, and nothing ever deletes an `installation` row (its history
// must stay readable). The upload row could therefore never be deleted, so it
// stayed "referenced", so its chunks — up to 4 GiB per installer — stayed on
// the host root filesystem FOREVER, finished installation or not.
//
// The cure keeps the row (the history still names the file, its size and its
// fingerprint) and releases the BYTES: `contenu_purge_a` records when.

import { existsSync, mkdtempSync, rmSync, utimesSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import {
    create as createInstallation,
    lireParId as lireInstallation,
} from '../depot/installation';
import { create as createUpload, lireParId as lireTeleversement, sceller } from '../depot/televersement';
import { ouvrirMagasin } from './icones';
import { ouvrirMagasinTranches, type MagasinTranches } from './magasin-tranches';
import { referencesTranches, unTour } from './nettoyage';

let base: Pilote | undefined;
let racines: string[] = [];

afterEach(async () => {
    await base?.fermer();
    base = undefined;
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
});

const MS = 1_787_136_773_742;
const JOUR_MS = 24 * 60 * 60_000;

function dossier(prefixe: string): string {
    const r = mkdtempSync(join(tmpdir(), prefixe));
    racines.push(r);
    return r;
}

function flux(s: string): AsyncIterable<Uint8Array> {
    return (async function* () {
        yield Buffer.from(s);
    })();
}

async function socle(p: Pilote): Promise<void> {
    await p.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)', [
        'u1', 'u1@exemple.test', 'scrypt$1$1$1$x$y', MS,
    ]);
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v1', 'vm-1', '192.168.3.2']);
}

/// A sealed upload, created long ago, whose single chunk is on disk and OLD.
async function vieuxTeleversement(p: Pilote, tranches: MagasinTranches, sha: string): Promise<string> {
    const t = await createUpload(
        p,
        { userId: 'u1', nom: 'setup.exe', taille: 1, sha256: sha.repeat(64), chunkSize: 8 },
        MS - 400 * JOUR_MS,
    );
    await sceller(p, t.id, MS - 400 * JOUR_MS);
    await tranches.write(t.id, 0, flux('x'), 1024);
    utimesSync(join(tranches.racine, t.id), new Date(0), new Date(0));
    return t.id;
}

async function marquer(p: Pilote, id: string, etat: string): Promise<void> {
    await p.executer('UPDATE installation SET etat = ? WHERE id = ?', [etat, id]);
}

describe(`purge of finished installers, engine=${MOTEUR}`, () => {
    it('🔴 frees the bytes of an upload whose installations are ALL finished, and keeps the history', async () => {
        base = await baseNeuve('purge-installateur-fini');
        await socle(base);
        const tranches = ouvrirMagasinTranches(join(dossier('purge-tr-'), 't'), () => {});
        const fini = await vieuxTeleversement(base, tranches, 'a');
        const inst = await createInstallation(base, { vmId: 'v1', televersementId: fini }, MS - 300 * JOUR_MS);
        await marquer(base, inst.id, 'terminee');

        // Witness in the SAME round: an upload whose installation is still
        // running must keep its bytes, otherwise "freed" could mean "everything goes".
        const enCours = await vieuxTeleversement(base, tranches, 'b');
        const instEnCours = await createInstallation(base, { vmId: 'v1', televersementId: enCours }, MS - 300 * JOUR_MS);
        await marquer(base, instEnCours.id, 'en_cours');

        const magasin = ouvrirMagasin(join(dossier('purge-ic-'), 'i'), () => {});
        await unTour({ base, magasin, tranches, maintenant: MS });

        expect(existsSync(join(tranches.racine, fini))).toBe(false);
        const ligne = await lireTeleversement(base, fini);
        expect(ligne?.nom).toBe('setup.exe');
        expect(ligne?.contenu_purge_a).toBe(MS);
        expect((await lireInstallation(base, inst.id))?.televersement_id).toBe(fini);
        expect(await referencesTranches(base)).toEqual(new Set([enCours]));

        expect(existsSync(join(tranches.racine, enCours))).toBe(true);
        expect((await lireTeleversement(base, enCours))?.contenu_purge_a).toBeNull();
    });

    it('keeps the bytes of a finished installation while the directory is still recent', async () => {
        base = await baseNeuve('purge-installateur-recent');
        await socle(base);
        const tranches = ouvrirMagasinTranches(join(dossier('purge-tr-'), 't'), () => {});
        const t = await createUpload(
            base,
            { userId: 'u1', nom: 'r.exe', taille: 1, sha256: 'c'.repeat(64), chunkSize: 8 },
            MS - 400 * JOUR_MS,
        );
        await sceller(base, t.id, MS);
        await tranches.write(t.id, 0, flux('x'), 1024);
        utimesSync(join(tranches.racine, t.id), new Date(MS), new Date(MS));
        const inst = await createInstallation(base, { vmId: 'v1', televersementId: t.id }, MS);
        await marquer(base, inst.id, 'terminee');

        const magasin = ouvrirMagasin(join(dossier('purge-ic-'), 'i'), () => {});
        await unTour({ base, magasin, tranches, maintenant: MS });

        expect(existsSync(join(tranches.racine, t.id))).toBe(true);
        expect((await lireTeleversement(base, t.id))?.contenu_purge_a).toBeNull();
    });
});
