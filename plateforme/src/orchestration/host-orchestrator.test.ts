import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { SEUIL_INJOIGNABLE_MS } from '../agents/fraicheur';
import { enroler, marquerVu } from '../depot/agent';
import { hostOrchestrator } from './host-orchestrator';
import { Wake } from './wake';

const MS = 1_787_136_773_742;
let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function poserVm(p: Pilote, id: string, vuA: number | null): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [id, `n-${id}`, '192.168.3.2']);
    await enroler(p, id, 'empreinte-opaque', `PREFIXE${id}`);
    if (vuA !== null) await marquerVu(p, id, vuA);
}

function mount(instant = MS) {
    const send = vi.fn(async (_v: 'wake') => ({ ok: true }) as const);
    const wake = new Wake(send, () => instant);
    return { wake, send, orchestrateur: hostOrchestrator(base!, () => instant, wake) };
}

describe(`hostOrchestrator, engine=${MOTEUR}`, () => {
    it('`start` requests the wake of a known VM', async () => {
        base = await baseNeuve('ho-demarrer');
        await poserVm(base, 'v1', null);
        const { orchestrateur, send } = mount();
        expect(await orchestrateur.start('v1')).toEqual({ ok: true });
        expect(send).toHaveBeenCalledTimes(1);
    });

    it('🔴 `start` of an unknown VM is refused WITHOUT touching the host', async () => {
        base = await baseNeuve('ho-inconnue');
        const { orchestrateur, send } = mount();
        expect(await orchestrateur.start('fantome')).toEqual({
            ok: false,
            motif: 'vm-inconnue',
            operation: 'demarrer',
            backend: 'hote',
        });
        expect(send).not.toHaveBeenCalled();
    });

    it('`etat` returns `demarrage` for an unreachable VM whose wake is pending', async () => {
        base = await baseNeuve('ho-etat-demarrage');
        await poserVm(base, 'v1', MS - SEUIL_INJOIGNABLE_MS - 1);
        const { orchestrateur } = mount();
        expect(await orchestrateur.etat('v1')).toBe('injoignable');
        await orchestrateur.start('v1');
        expect(await orchestrateur.etat('v1')).toBe('demarrage');
    });

    it('🔴 `etat` returns `prete` for a live VM, even if a wake is marked pending', async () => {
        base = await baseNeuve('ho-etat-prete');
        await poserVm(base, 'v1', MS);
        const { orchestrateur } = mount();
        await orchestrateur.start('v1');
        expect(await orchestrateur.etat('v1')).toBe('prete');
    });

    it('the other verbs keep the static inventory behaviour', async () => {
        base = await baseNeuve('ho-autres');
        await poserVm(base, 'v1', null);
        const { orchestrateur } = mount();
        vi.spyOn(console, 'warn').mockImplementation(() => {});
        expect(await orchestrateur.arreter('v1')).toMatchObject({ ok: false, motif: 'non-supporte' });
        expect(await orchestrateur.instantane('v1', 'x')).toMatchObject({ ok: false, motif: 'non-supporte' });
        expect((await orchestrateur.lister()).map((v) => v.id)).toEqual(['v1']);
    });
});
