// `POST /application/:id/lancer` on a VM that does not answer: the launch asks
// the orchestrator to wake it, and the 503 body says whether the wake was
// accepted.
//
// 🔴 THE ORCHESTRATOR IS A DOUBLE, THE REGISTRY IS REAL (no agent registered:
// `lancer` answers `agent-injoignable` without sending anything). What the real
// host orchestrator does is tested by `host-orchestrator.test.ts`.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { RegistreAgents, type SocketAgent } from '../agents/registre';
import { DELAI_LANCEMENT_MS } from '../agents/registre';
import type { Orchestrateur } from '../orchestration/interface';
import { inventaireStatique } from '../orchestration/inventaire-statique';
import { BACKEND_HOTE, refuser, type Outcome } from '../orchestration/refus';
import { enroler, marquerVu } from '../depot/agent';
import { createUser } from '../depot/utilisateur';
import {
    attribuer,
    demonter,
    jetonDe,
    MS,
    monterRoute,
    poserApp,
    poserVm,
    SECRET,
    withIt,
    type Montage,
} from './routes-harnais';
import { servirApplications } from './routes-applications';

let montage: Montage | undefined;
let registre = new RegistreAgents();

afterEach(async () => {
    await demonter(montage);
    montage = undefined;
});

/// Mounts the launch route with the given `start` in place of the static one.
async function servir(nom: string, start: Orchestrateur['start']): Promise<Montage> {
    registre = new RegistreAgents();
    montage = await monterRoute(nom, (req, rep, base) =>
        servirApplications(req, rep, {
            base,
            secretJeton: SECRET,
            registre,
            maintenant: () => MS,
            orchestrateur: { ...inventaireStatique(base, () => MS), start },
        }),
    );
    return montage;
}

const lancer = (m: Montage, id: string, jeton: string) =>
    fetch(`${m.url}/application/${id}/lancer`, { method: 'POST', headers: withIt(jeton) });

describe('launching an application on a VM that does not answer', () => {
    it('🔴 asks for the wake and answers 503 `etat: demarrage` when it is accepted', async () => {
        const start = vi.fn(async (): Promise<Outcome> => ({ ok: true }));
        const m = await servir('ra-reveil', start);
        await poserVm(m.base, 'v-1');
        const u = await attribuer(m.base, 'v-1', 'u@exemple.test');
        const id = await poserApp(m.base, 'v-1', 'Firefox', 'c-1');

        const r = await lancer(m, id, jetonDe(u));
        expect(r.status).toBe(503);
        expect(await r.json()).toEqual({ refus: 'agent-injoignable', etat: 'demarrage' });
        expect(start).toHaveBeenCalledWith('v-1');
    });

    it('🔴 answers 503 `etat: injoignable` with the reason when the wake is refused', async () => {
        const start = vi.fn(async (): Promise<Outcome> =>
            refuser('hote-inaccessible', 'demarrer', BACKEND_HOTE),
        );
        const m = await servir('ra-reveil-refuse', start);
        await poserVm(m.base, 'v-1');
        const u = await attribuer(m.base, 'v-1', 'u@exemple.test');
        const id = await poserApp(m.base, 'v-1', 'Firefox', 'c-1');

        const r = await lancer(m, id, jetonDe(u));
        expect(r.status).toBe(503);
        expect(await r.json()).toEqual({
            refus: 'agent-injoignable',
            etat: 'injoignable',
            reveil: 'hote-inaccessible',
        });
    });

    it("🔴 does NOT ask for the wake of someone else's VM", async () => {
        const start = vi.fn(async (): Promise<Outcome> => ({ ok: true }));
        const m = await servir('ra-reveil-autre', start);
        await poserVm(m.base, 'v-1');
        await attribuer(m.base, 'v-1', 'alice@exemple.test');
        const bob = await createUser(m.base, 'bob@exemple.test', 'x', MS);
        const id = await poserApp(m.base, 'v-1', 'Firefox', 'c-1');

        const r = await lancer(m, id, jetonDe(bob));
        expect(r.status).toBe(404);
        expect(start).not.toHaveBeenCalled();
    });

    it('🔴 a `504 delai` asks for no wake', async () => {
        const start = vi.fn(async (): Promise<Outcome> => ({ ok: true }));
        const m = await servir('ra-reveil-delai', start);
        await poserVm(m.base, 'v-1');
        const u = await attribuer(m.base, 'v-1', 'u@exemple.test');
        const id = await poserApp(m.base, 'v-1', 'Firefox', 'c-1');
        // An agent registered and FRESH (it beat a second ago), but SILENT: the
        // order goes out, nobody answers.
        await enroler(m.base, 'v-1', 'empreinte', 'prefixe-1');
        await marquerVu(m.base, 'v-1', MS - 1_000);
        const muet: SocketAgent = { readyState: 1, send() {}, close() {} };
        registre.inscrire('v-1', muet);

        const r = await lancer(m, id, jetonDe(u));
        expect(r.status).toBe(504);
        expect(await r.json()).toEqual({ refus: 'delai' });
        expect(start).not.toHaveBeenCalled();
    }, DELAI_LANCEMENT_MS + 10_000);

    it('🔴 wakes a VM whose socket is still held but whose heartbeat is stale, and sends no order', async () => {
        const start = vi.fn(async (): Promise<Outcome> => ({ ok: true }));
        const m = await servir('ra-reveil-hiberne', start);
        await poserVm(m.base, 'v-1');
        const u = await attribuer(m.base, 'v-1', 'u@exemple.test');
        const id = await poserApp(m.base, 'v-1', 'Firefox', 'c-1');
        // A hibernated VM: the agent beat ten minutes ago, and its socket is
        // still ESTABLISHED on our side (no FIN ever came).
        await enroler(m.base, 'v-1', 'empreinte', 'prefixe-1');
        await marquerVu(m.base, 'v-1', MS - 600_000);
        const send = vi.fn();
        registre.inscrire('v-1', { readyState: 1, send, close() {} });

        const r = await lancer(m, id, jetonDe(u));
        expect(r.status).toBe(503);
        expect(await r.json()).toEqual({ refus: 'agent-injoignable', etat: 'demarrage' });
        expect(start).toHaveBeenCalledWith('v-1');
        expect(send).not.toHaveBeenCalled();
    });
});
