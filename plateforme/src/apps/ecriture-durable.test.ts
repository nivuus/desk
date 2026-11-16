// The durability of the two "temporary file then rename" writes: chunks and icons.
//
// 🔴 THE DEFECT: both stores wrote a temporary file and renamed it onto its
// final name WITHOUT `fsync`. After a host freeze (this host has frozen nine
// times) the rename can be on disk while the data is not: ext4's
// `auto_da_alloc` only covers renaming over an EXISTING file, and a chunk or an
// icon always lands on a NEW name. The chunk then exists with 0 bytes, `lister`
// counts it, sealing calls it `incoherentes` — the verdict that cannot be repaired.
//
// A power cut cannot be staged in a test. What CAN be checked is the order of
// the system calls the durability depends on, observed on the real `node:fs`:
// the data is flushed BEFORE the rename, and the directory AFTER it. The calls
// are recorded, never replaced — every write below really lands on disk.

import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { createHash } from 'node:crypto';
import { afterEach, describe, expect, it, vi } from 'vitest';

type Evenement =
    | { quoi: 'open'; chemin: string; fd: number }
    | { quoi: 'fsync'; fd: number }
    | { quoi: 'rename'; de: string; vers: string };

const journal: Evenement[] = [];

vi.mock('node:fs', async (importOriginal) => {
    const reel = await importOriginal<typeof import('node:fs')>();
    return {
        ...reel,
        openSync: (chemin: string, ...reste: unknown[]) => {
            const fd = (reel.openSync as (...a: unknown[]) => number)(chemin, ...reste);
            journal.push({ quoi: 'open', chemin: String(chemin), fd });
            return fd;
        },
        fsyncSync: (fd: number) => {
            journal.push({ quoi: 'fsync', fd });
            reel.fsyncSync(fd);
        },
        renameSync: (de: string, vers: string) => {
            journal.push({ quoi: 'rename', de: String(de), vers: String(vers) });
            reel.renameSync(de, vers);
        },
    };
});

const { ouvrirMagasinTranches } = await import('./magasin-tranches');
const { ouvrirMagasin } = await import('./icones');

let racines: string[] = [];

afterEach(() => {
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
    journal.length = 0;
});

function dossier(): string {
    const r = mkdtempSync(join(tmpdir(), 'ecriture-durable-'));
    racines.push(r);
    return r;
}

/// The index of the `fsync` of the descriptor opened on `chemin`, or -1.
function fsyncDe(chemin: string, apres = 0): number {
    const ouvertures = journal.filter(
        (e): e is Extract<Evenement, { quoi: 'open' }> => e.quoi === 'open' && e.chemin === chemin,
    );
    for (const o of ouvertures) {
        const i = journal.findIndex((e, k) => k >= apres && e.quoi === 'fsync' && e.fd === o.fd);
        if (i >= 0) return i;
    }
    return -1;
}

function renommage(vers: string): { index: number; de: string } {
    const index = journal.findIndex((e) => e.quoi === 'rename' && e.vers === vers);
    const e = journal[index];
    return { index, de: e?.quoi === 'rename' ? e.de : '' };
}

describe('durable writes', () => {
    it('🔴 a chunk is flushed before its rename, and its directory after it', async () => {
        const racine = join(dossier(), 'televersements');
        const magasin = ouvrirMagasinTranches(racine, () => {});
        const id = '11111111-2222-4333-8444-555555555555';
        const r = await magasin.write(
            id,
            0,
            (async function* () {
                yield Buffer.from('octets');
            })(),
            1024,
        );
        expect(r).toEqual({ ok: true, octets: 6 });

        const cible = join(racine, id, '0');
        const { index: iRename, de: provisoire } = renommage(cible);
        expect(iRename).toBeGreaterThanOrEqual(0);
        const iDonnees = fsyncDe(provisoire);
        expect(iDonnees).toBeGreaterThanOrEqual(0);
        expect(iDonnees).toBeLessThan(iRename);
        expect(fsyncDe(dirname(cible), iRename)).toBeGreaterThan(iRename);
        // The upload directory was NEW: its own entry in the root must be flushed too.
        expect(fsyncDe(racine)).toBeGreaterThanOrEqual(0);
        expect(readFileSync(cible, 'utf8')).toBe('octets');
    });

    it('🔴 an icon is flushed before its rename, and its directory after it', () => {
        const repertoire = join(dossier(), 'icones');
        const magasin = ouvrirMagasin(repertoire, () => {});
        const octets = Buffer.from('une icone');
        const empreinte = createHash('sha256').update(octets).digest('hex');
        magasin.write(empreinte, octets);

        const cible = join(repertoire, empreinte);
        const { index: iRename, de: provisoire } = renommage(cible);
        expect(iRename).toBeGreaterThanOrEqual(0);
        const iDonnees = fsyncDe(provisoire);
        expect(iDonnees).toBeGreaterThanOrEqual(0);
        expect(iDonnees).toBeLessThan(iRename);
        expect(fsyncDe(repertoire, iRename)).toBeGreaterThan(iRename);
        expect(readFileSync(cible)).toEqual(octets);
    });
});
