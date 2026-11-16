// The per-user BYTE quota on uploads, through a real HTTP server — both engines.
//
// 🔴 THE DEFECT: the only bound was a COUNT of UNSEALED uploads (three). A
// sealed upload left that count, so an authenticated user could declare, fill
// and seal 4 GiB files in a loop; nothing removed them for thirty days, on the
// host root filesystem. The bound that protects a disk is a number of BYTES.

import { afterEach, describe, expect, it } from 'vitest';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { MOTEUR } from '../base/harnais';
import { ouvrirMagasinTranches } from '../apps/magasin-tranches';
import { lireConfig } from '../config';
import { createUser } from '../depot/utilisateur';
import { demonter, jetonDe, monterRoute, MS, SECRET, withIt, type Montage } from './routes-harnais';
import { QUOTA_TELEVERSEMENT_OCTETS_DEFAUT, servirTeleversement } from './routes-televersement';

let m: Montage | undefined;
let racine: string | undefined;

afterEach(async () => {
    await demonter(m);
    m = undefined;
    if (racine !== undefined) rmSync(racine, { recursive: true, force: true });
    racine = undefined;
});

const SHA = 'a'.repeat(64);

async function monter(nom: string, quotaOctets: number | undefined): Promise<Montage> {
    racine = mkdtempSync(join(tmpdir(), 'quota-tel-'));
    const tranches = ouvrirMagasinTranches(join(racine, 't'), () => {});
    m = await monterRoute(nom, (req, rep, base) =>
        servirTeleversement(req, rep, {
            base, secretJeton: SECRET, tranches, maintenant: () => MS, quotaOctets,
        }),
    );
    return m;
}

const declarer = (url: string, jeton: string, taille: number) =>
    fetch(`${url}/televersement`, {
        method: 'POST',
        headers: withIt(jeton, { 'content-type': 'application/json' }),
        body: JSON.stringify({ nom: 'setup.exe', taille, sha256: SHA }),
    });

describe(`upload byte quota, engine=${MOTEUR}`, () => {
    it('🔴 refuses a declaration that would hold more bytes than the quota, sealed uploads included', async () => {
        const { url, base } = await monter('quota-octets', 25);
        const u = await createUser(base, 'q@exemple.test', 'empreinte-opaque', MS);
        const jeton = jetonDe(u);

        const premier = await declarer(url, jeton, 10);
        expect(premier.status).toBe(201);
        const { id } = (await premier.json()) as { id: string };
        // Sealed: it leaves the count of uploads in progress, NOT the disk.
        await base.executer('UPDATE televersement SET scelle_a = ? WHERE id = ?', [MS, id]);
        expect((await declarer(url, jeton, 10)).status).toBe(201);

        const refus = await declarer(url, jeton, 10);
        expect(refus.status).toBe(507);
        expect(await refus.json()).toEqual({ refus: 'quota-octets', maximum: 25, occupe: 20 });

        // Bytes already released by the purge no longer count.
        await base.executer('UPDATE televersement SET contenu_purge_a = ? WHERE id = ?', [MS, id]);
        expect((await declarer(url, jeton, 10)).status).toBe(201);
    });

    it('applies the documented default when no quota is configured', async () => {
        const { url, base } = await monter('quota-defaut', undefined);
        const jeton = jetonDe(await createUser(base, 'd@exemple.test', 'empreinte-opaque', MS));
        const trop = await declarer(url, jeton, QUOTA_TELEVERSEMENT_OCTETS_DEFAUT + 1);
        // The 4 GiB per-file cap answers first for a size this large: the
        // default quota is never below one maximal file.
        expect(trop.status).toBe(413);
        expect(QUOTA_TELEVERSEMENT_OCTETS_DEFAUT).toBeGreaterThanOrEqual(4 * 1024 ** 3);
    });
});

describe('PLATEFORME_TELEVERSEMENT_QUOTA_OCTETS', () => {
    const BASE = {
        PLATEFORME_HOTE: '127.0.0.1',
        PLATEFORME_SECRET_JETON: SECRET,
        PLATEFORME_AUTH: 'motdepasse',
    };

    it('is optional, and parsed as a whole number of bytes', () => {
        expect(lireConfig(BASE).quotaTeleversementOctets).toBeUndefined();
        expect(lireConfig({ ...BASE, PLATEFORME_TELEVERSEMENT_QUOTA_OCTETS: '' }).quotaTeleversementOctets)
            .toBeUndefined();
        expect(lireConfig({ ...BASE, PLATEFORME_TELEVERSEMENT_QUOTA_OCTETS: '1073741824' })
            .quotaTeleversementOctets).toBe(1073741824);
    });

    it('throws on a value that is not a positive whole number, never falls back', () => {
        for (const v of ['abc', '-1', '0', '1.5', '1e9']) {
            expect(() => lireConfig({ ...BASE, PLATEFORME_TELEVERSEMENT_QUOTA_OCTETS: v }))
                .toThrow(/PLATEFORME_TELEVERSEMENT_QUOTA_OCTETS/);
        }
    });
});
