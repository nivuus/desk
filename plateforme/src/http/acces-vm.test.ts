// VM ownership on the three human routes that act on a VM, tested THROUGH
// real HTTP servers: `GET /applications?vm=`, `POST /application/:id/lancer`
// and `POST /installation`.
//
// An UNASSIGNED VM (`utilisateur_id` NULL: after a detach, a failed
// assignment, or an orphan row) belongs to nobody. It must answer every
// authenticated requester exactly like a VM owned by someone else — the
// verdict `GET /vm` (`orchestration/selection.ts::vmsDe`) already gives.
// Each case carries its witness: the same request from the OWNER succeeds,
// otherwise a route that refuses everything would pass.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { MOTEUR } from '../base/harnais';
import { RegistreAgents } from '../agents/registre';
import { ouvrirMagasinTranches } from '../apps/magasin-tranches';
import { enroler, marquerVu } from '../depot/agent';
import { create as createUpload, sceller } from '../depot/televersement';
import { createUser } from '../depot/utilisateur';
import { inventaireStatique } from '../orchestration/inventaire-statique';
import { accesVm } from './acces-vm';
import {
    attribuer, demonter, jetonDe, monterRoute, MS, poserApp, poserVm, SECRET, withIt, type Montage,
} from './routes-harnais';
import { servirApplications } from './routes-applications';
import { servirInstallation } from './routes-installation';

let m: Montage | undefined;
let racine: string | undefined;

afterEach(async () => {
    await demonter(m);
    m = undefined;
    if (racine !== undefined) rmSync(racine, { recursive: true, force: true });
    racine = undefined;
    vi.restoreAllMocks();
});

const REFUS_VM = JSON.stringify({ refus: 'vm-inconnue' });

async function servirApps(nom: string): Promise<Montage> {
    return monterRoute(nom, (req, rep, base) =>
        servirApplications(req, rep, {
            base,
            secretJeton: SECRET,
            registre: new RegistreAgents(),
            maintenant: () => MS,
            orchestrateur: inventaireStatique(base, () => MS),
        }));
}

async function servirInstall(nom: string): Promise<Montage> {
    racine = mkdtempSync(join(tmpdir(), `acces-${nom}-`));
    const tranches = ouvrirMagasinTranches(racine, () => {});
    return monterRoute(nom, (req, rep, base) =>
        servirInstallation(req, rep, {
            base,
            secretJeton: SECRET,
            tranches,
            registre: new RegistreAgents(),
            maintenant: () => MS,
        }));
}

describe(`unassigned VMs are foreign to everyone, engine=${MOTEUR}`, () => {
    it('`GET /applications?vm=` refuses an unassigned VM like a foreign one', async () => {
        m = await servirApps('acces-liste');
        await poserVm(m.base, 'v-1');
        await poserApp(m.base, 'v-1', 'Firefox', 'c-1');
        const u = await createUser(m.base, 'anyone@example.test', 'x', MS);

        const r = await fetch(`${m.url}/applications?vm=v-1`, { headers: withIt(jetonDe(u)) });
        expect([r.status, await r.text()]).toEqual([404, REFUS_VM]);

        // Witness: once the VM is assigned to this user, the same request is served.
        await m.base.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', [u, 'v-1']);
        const ok = await fetch(`${m.url}/applications?vm=v-1`, { headers: withIt(jetonDe(u)) });
        expect(ok.status).toBe(200);
    });

    it('`POST /application/:id/lancer` refuses an unassigned VM like a foreign one', async () => {
        m = await servirApps('acces-lancer');
        await poserVm(m.base, 'v-1');
        const id = await poserApp(m.base, 'v-1', 'Firefox', 'c-1');
        const u = await createUser(m.base, 'anyone@example.test', 'x', MS);

        const r = await fetch(`${m.url}/application/${id}/lancer`, {
            method: 'POST', headers: withIt(jetonDe(u)),
        });
        expect([r.status, await r.text()]).toEqual([404, REFUS_VM]);
    });

    it('`POST /installation` refuses an unassigned VM like a foreign one, and writes no row', async () => {
        m = await servirInstall('acces-installation');
        await poserVm(m.base, 'v-1');
        await enroler(m.base, 'v-1', 'opaque-fingerprint', 'PREFIX-A');
        await marquerVu(m.base, 'v-1', MS);
        const u = await createUser(m.base, 'anyone@example.test', 'x', MS);
        const tel = await createUpload(m.base, {
            userId: u, nom: 'setup.exe', taille: 3, sha256: '0'.repeat(64), chunkSize: 7,
        }, MS);
        await sceller(m.base, tel.id, MS);
        const corps = { method: 'POST', headers: withIt(jetonDe(u)), body: JSON.stringify({ vm: 'v-1', televersement: tel.id }) };

        const r = await fetch(`${m.url}/installation`, corps);
        expect([r.status, await r.text()]).toEqual([404, REFUS_VM]);
        expect(await m.base.interroger('SELECT id FROM installation', [])).toEqual([]);

        // Witness: the owner of the VM gets the order through.
        await m.base.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', [u, 'v-1']);
        expect((await fetch(`${m.url}/installation`, corps)).status).toBe(201);
    });

    it('`accesVm` gives one verdict per case, and logs the real case without serving it', async () => {
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        m = await servirApps('acces-verdicts');
        await poserVm(m.base, 'v-libre');
        await poserVm(m.base, 'v-prise');
        const owner = await attribuer(m.base, 'v-prise', 'owner@example.test');
        const other = await createUser(m.base, 'other@example.test', 'x', MS);

        expect(await accesVm(m.base, 'v-prise', owner)).toBe('ok');
        expect(await accesVm(m.base, 'v-prise', other)).toBe('etrangere');
        expect(await accesVm(m.base, 'v-libre', other)).toBe('etrangere');
        expect(await accesVm(m.base, 'never-seen', other)).toBe('inconnue');
        const tout = traces.join(' | ');
        expect(tout).toContain('cas=unassigned');
        expect(tout).toContain('cas=etrangere');
        expect(tout).toContain('cas=inconnue');
    });
});
