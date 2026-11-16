// `POST /installation` on an upload whose bytes the cleanup released — both engines.
//
// 🔴 Once `apps/nettoyage.ts` releases the chunks of an installer whose
// installations are all finished, the row stays (the history names it) but the
// bytes are gone. Ordering it again would create an order the agent can only
// fail to download, far from here and without saying why: the refusal belongs
// at the door, with its reason.

import { afterEach, describe, expect, it } from 'vitest';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { MOTEUR } from '../base/harnais';
import { ouvrirMagasinTranches } from '../apps/magasin-tranches';
import { RegistreAgents } from '../agents/registre';
import { enroler, marquerVu } from '../depot/agent';
import { create as createUpload, sceller } from '../depot/televersement';
import { attribuer, demonter, jetonDe, monterRoute, MS, poserVm, SECRET, withIt, type Montage } from './routes-harnais';
import { servirInstallation } from './routes-installation';

let m: Montage | undefined;
let racine: string | undefined;

afterEach(async () => {
    await demonter(m);
    m = undefined;
    if (racine !== undefined) rmSync(racine, { recursive: true, force: true });
    racine = undefined;
});

describe(`POST /installation on a purged upload, engine=${MOTEUR}`, () => {
    it('🔴 is refused with `contenu-purge`, and no installation row is created', async () => {
        racine = mkdtempSync(join(tmpdir(), 'inst-purge-'));
        const tranches = ouvrirMagasinTranches(join(racine, 't'), () => {});
        m = await monterRoute('inst-purge', (req, rep, base) =>
            servirInstallation(req, rep, {
                base, secretJeton: SECRET, tranches, registre: new RegistreAgents(), maintenant: () => MS,
            }),
        );
        const { base, url } = m;
        await poserVm(base, 'v1');
        const u = await attribuer(base, 'v1', 'p@exemple.test');
        await enroler(base, 'v1', 'empreinte-opaque', 'RhH1x2QmTz9kLpVbNc7dAw');
        await marquerVu(base, 'v1', MS);
        const tel = await createUpload(
            base,
            { userId: u, nom: 'setup.exe', taille: 3, sha256: 'a'.repeat(64), chunkSize: 4 },
            MS,
        );
        await sceller(base, tel.id, MS);

        const commander = () => fetch(`${url}/installation`, {
            method: 'POST',
            headers: withIt(jetonDe(u)),
            body: JSON.stringify({ vm: 'v1', televersement: tel.id }),
        });

        // Witness: the same order on the same upload is accepted while its bytes exist.
        expect((await commander()).status).toBe(201);

        await base.executer('UPDATE televersement SET contenu_purge_a = ? WHERE id = ?', [MS, tel.id]);
        const r = await commander();
        expect([r.status, await r.json()]).toEqual([409, { refus: 'contenu-purge' }]);
        const n = await base.interroger<{ n: number }>('SELECT COUNT(*) AS n FROM installation', []);
        expect(Number(n[0]?.n)).toBe(1);
    });
});
