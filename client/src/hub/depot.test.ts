import { describe, expect, it } from 'vitest';
import { deposer, resumer } from './depot';
import type { DepsTeleversement, Issue } from './televersement';

const FILE = new File([new Uint8Array([1, 2, 3])], 'app.msi');

describe('resumer', () => {
    it('returns a SUCCESS tone and names the file when sealing passes', () => {
        const issue: Issue = { etat: 'scelle', id: 't-1', size: 3, sha256: 'ab', deposees: [0] };
        expect(resumer(FILE, issue)).toEqual({
            ton: 'succes',
            texte: 'app.msi was uploaded and sealed (1 chunk(s) deposited).',
            id: 't-1',
        });
    });

    it('returns the SERVICE REASON as is, never rewritten', () => {
        const issue: Issue = {
            etat: 'refus',
            id: 't-2',
            refus: { source: 'service', etape: 'scellement', statut: 409, motif: 'empreinte-divergente' },
        };
        const r = resumer(FILE, issue);
        expect(r.ton).toBe('danger');
        // 🔴 THE REASON MUST APPEAR WORD FOR WORD: translating it would make it a
        //    copy no type confronts with its source.
        expect(r.texte).toContain('empreinte-divergente');
        expect(r.id).toBe('t-2');
    });

    it('tells a LOCAL refusal from a SERVICE refusal', () => {
        const issue: Issue = {
            etat: 'refus',
            refus: { source: 'client', motif: 'fichier-different', detail: 'the size changed' },
        };
        const r = resumer(FILE, issue);
        expect(r.texte).toContain('fichier-different');
        expect(r.texte).toContain('the size changed');
        expect(r.texte).not.toContain('service');
        expect(r.id).toBeUndefined();
    });
});

describe('deposer — the convergence point', () => {
    /// 🔴 THIS TEST IS THE ONE THAT MAKES THE RED OF CRITERION ② HONEST: it exercises
    ///    that `deposer` REALLY leads to the upload, and not only
    ///    that it returns an object. Both paths of the hub — drag and drop and
    ///    `launchQueue` — call THIS function, and no other.
    it('takes a file all the way to sealing, and reports it', async () => {
        const vus: string[] = [];
        const deps: DepsTeleversement = {
            base: 'https://x',
            jeton: 'J',
            maintenant: () => 0,
            fetch: async (url, init) => {
                vus.push(`${init?.method ?? 'GET'} ${url.replace('https://x', '')}`);
                if (url.endsWith('/televersement')) {
                    // ⚠️ `taille_tranche` IS A CONTRACT, not an ornament:
                    //    `televerser` refuses with `etat-illisible` without it. A
                    //    first draft of this fake omitted it, and the test went
                    //    red — so it does exercise the REAL sequence.
                    //    At 2 bytes per chunk, a file of 3 makes TWO.
                    return {
                        ok: true,
                        status: 201,
                        json: async () => ({ id: 't-9', taille_tranche: 2, tranches_presentes: [] }),
                    };
                }
                return { ok: true, status: 200, json: async () => ({}) };
            },
        };
        const resume = await deposer(FILE, deps);
        expect(resume.ton).toBe('succes');
        // The REAL sequence: creation, one chunk, sealing.
        expect(vus[0]).toBe('POST /televersement');
        expect(vus.filter((v) => v.startsWith('PUT /televersement/t-9/tranche/'))).toEqual([
            'PUT /televersement/t-9/tranche/0',
            'PUT /televersement/t-9/tranche/1',
        ]);
        expect(vus.at(-1)).toBe('POST /televersement/t-9/sceller');
    });

    it('returns a service refusal WITHOUT throwing', async () => {
        const deps: DepsTeleversement = {
            base: 'https://x',
            jeton: 'J',
            maintenant: () => 0,
            fetch: async () => ({ ok: false, status: 413, json: async () => ({ refus: 'trop-gros' }) }),
        };
        const resume = await deposer(FILE, deps);
        expect(resume.ton).toBe('danger');
        expect(resume.texte).toContain('trop-gros');
    });
});
