import { describe, expect, it, vi } from 'vitest';
import type { Fetch } from './catalogue';
import { LAUNCH_RETRY_MS, LAUNCH_STARTUP_MAX_MS, launchWhenReady } from './lancement';

/// A service that answers the given responses in order, then the last one again.
function service(reponses: Array<{ status: number; corps: unknown }>) {
    let i = 0;
    const fetch = vi.fn<Fetch>(async () => {
        const r = reponses[Math.min(i++, reponses.length - 1)];
        return {
            ok: r.status < 400,
            status: r.status,
            json: async () => r.corps,
            arrayBuffer: async () => new ArrayBuffer(0),
        };
    });
    return { fetch, deps: { base: 'http://p', jeton: 't', fetch } };
}

function horloge() {
    let t = 0;
    const attentes: number[] = [];
    return {
        clock: {
            now: () => t,
            sleep: async (ms: number) => {
                attentes.push(ms);
                t += ms;
            },
        },
        attentes,
    };
}

const DEMARRAGE = { status: 503, corps: { refus: 'agent-injoignable', etat: 'demarrage' } };
const SUCCES = { status: 200, corps: { issue: 'raccourci' } };

describe('launchWhenReady', () => {
    it('a launch that succeeds first time announces nothing and waits for nothing', async () => {
        const { deps, fetch } = service([SUCCES]);
        const { clock, attentes } = horloge();
        const onStarting = vi.fn();
        const r = await launchWhenReady('app1', deps, clock, onStarting);
        expect(r).toEqual({ kind: 'done', issue: { etat: 'ok', value: null } });
        expect(fetch).toHaveBeenCalledTimes(1);
        expect(onStarting).not.toHaveBeenCalled();
        expect(attentes).toEqual([]);
    });

    it('a starting VM: announces ONCE, replays every 3 s, then succeeds', async () => {
        const { deps, fetch } = service([DEMARRAGE, DEMARRAGE, DEMARRAGE, SUCCES]);
        const { clock, attentes } = horloge();
        const onStarting = vi.fn();
        const r = await launchWhenReady('app1', deps, clock, onStarting);
        expect(r.kind).toBe('done');
        expect(fetch).toHaveBeenCalledTimes(4);
        expect(onStarting).toHaveBeenCalledTimes(1);
        expect(attentes).toEqual([LAUNCH_RETRY_MS, LAUNCH_RETRY_MS, LAUNCH_RETRY_MS]);
    });

    it('a VM that never comes back: `vm-timeout` after LAUNCH_STARTUP_MAX_MS, no infinite loop', async () => {
        const { deps, fetch } = service([DEMARRAGE]);
        const { clock } = horloge();
        const r = await launchWhenReady('app1', deps, clock, () => {});
        expect(r).toEqual({ kind: 'vm-timeout' });
        expect(fetch.mock.calls.length).toBe(LAUNCH_STARTUP_MAX_MS / LAUNCH_RETRY_MS + 1);
    });

    it('`504 delai` is NEVER replayed (the order may have gone out: duplicate risk)', async () => {
        const { deps, fetch } = service([{ status: 504, corps: { refus: 'delai' } }, SUCCES]);
        const { clock } = horloge();
        const r = await launchWhenReady('app1', deps, clock, () => {});
        expect(fetch).toHaveBeenCalledTimes(1);
        expect(r).toMatchObject({ kind: 'done', issue: { etat: 'refus', refus: { statut: 504, motif: 'delai' } } });
    });

    it('a 503 `injoignable` WITHOUT `demarrage` (wake impossible) is not replayed', async () => {
        const refus = {
            status: 503,
            corps: { refus: 'agent-injoignable', etat: 'injoignable', reveil: 'hote-inaccessible' },
        };
        const { deps, fetch } = service([refus, SUCCES]);
        const { clock } = horloge();
        const r = await launchWhenReady('app1', deps, clock, () => {});
        expect(fetch).toHaveBeenCalledTimes(1);
        expect(r.kind).toBe('done');
    });

    it('an authorisation refusal (401/404) is not replayed', async () => {
        const { deps, fetch } = service([{ status: 404, corps: { refus: 'vm-inconnue' } }, SUCCES]);
        const { clock } = horloge();
        await launchWhenReady('app1', deps, clock, () => {});
        expect(fetch).toHaveBeenCalledTimes(1);
    });
});
