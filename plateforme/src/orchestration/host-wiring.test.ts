import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { hostWiring } from './host-wiring';

let base: Pilote | undefined;
afterEach(async () => {
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

describe(`hostWiring, engine=${MOTEUR}`, () => {
    it('🔴 without a configured socket: the static orchestrator, `start` refuses `non-supporte`', async () => {
        base = await baseNeuve('hw-sans-socket');
        await base.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', ['v1', 'w1', '192.168.3.2']);
        vi.spyOn(console, 'warn').mockImplementation(() => {});
        const { orchestrateur } = hostWiring(base, Date.now, undefined);
        expect(await orchestrateur.start('v1')).toMatchObject({ ok: false, motif: 'non-supporte' });
    });

    it('with a configured socket: `start` goes through the host (absent here, so `hote-inaccessible`)', async () => {
        base = await baseNeuve('hw-avec-socket');
        await base.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', ['v1', 'w1', '192.168.3.2']);
        vi.spyOn(console, 'warn').mockImplementation(() => {});
        const { orchestrateur } = hostWiring(base, Date.now, '/tmp/host-wiring-absent.sock');
        expect(await orchestrateur.start('v1')).toMatchObject({ ok: false, motif: 'hote-inaccessible' });
    });

    it('`activity` only exists when the channel is configured', async () => {
        base = await baseNeuve('hw-activity');
        expect(hostWiring(base, Date.now, undefined).activity).toBeUndefined();
        const configured = hostWiring(base, Date.now, '/tmp/host-wiring-absent.sock');
        expect(configured.activity).toBeDefined();
        configured.activity!.stop();
    });
});
