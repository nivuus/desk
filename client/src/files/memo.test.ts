import { describe, expect, it } from 'vitest';
import { createAdapter, type FileHandle, type PoigneeRepertoire } from './adaptateur';
import { MAX_ENTREES, TTL_MS, createMemo } from './memo';

/* A tree whose directories COUNT their enumerations: it is the cost `memo.ts`
   removes, and a fake that did not count it could not see it come back. */

type Noeud = { [nom: string]: Noeud | Uint8Array };

function monter(arbre: Noeud) {
    const compte = { values: 0 };
    function dossier(nom: string, n: Noeud): PoigneeRepertoire {
        const fichier = (enfant: string): FileHandle => ({
            kind: 'file',
            name: enfant,
            getFile: async () => {
                const actuel = n[enfant];
                if (!(actuel instanceof Uint8Array)) {
                    throw new DOMException(enfant, 'NotFoundError');
                }
                return {
                    size: actuel.length,
                    lastModified: 0,
                    slice: (d: number, f: number) => ({
                        arrayBuffer: async () => actuel.slice(d, f).buffer as ArrayBuffer,
                    }),
                    arrayBuffer: async () => actuel.buffer as ArrayBuffer,
                };
            },
        });
        return {
            kind: 'directory',
            name: nom,
            async getDirectoryHandle(enfant) {
                const e = n[enfant];
                if (e === undefined || e instanceof Uint8Array) {
                    throw new DOMException(enfant, 'NotFoundError');
                }
                return dossier(enfant, e);
            },
            async getFileHandle(enfant) {
                const e = n[enfant];
                if (!(e instanceof Uint8Array)) throw new DOMException(enfant, 'NotFoundError');
                return fichier(enfant);
            },
            async *values() {
                compte.values += 1;
                for (const [enfant, e] of Object.entries(n)) {
                    yield e instanceof Uint8Array ? fichier(enfant) : dossier(enfant, e);
                }
            },
        };
    }
    let temps = 0;
    const horloge = { avancer: (ms: number) => (temps += ms) };
    const adaptateur = createAdapter(dossier('', arbre), false, () => temps);
    return { adaptateur, compte, horloge };
}

const OCTETS = new Uint8Array(Array.from({ length: 100 }, (_, i) => i));

describe("lire's memory of resolved handles", () => {
    it('reads the chunks of one file with ONE resolution, not one per chunk', async () => {
        const { adaptateur, compte } = monter({ a: { b: { 'F.bin': OCTETS } } });
        const lus: number[] = [];
        for (let position = 0; position < 100; position += 10) {
            lus.push(...(await adaptateur.lire('a/b/f.bin', position, 10)));
        }
        expect(lus).toEqual([...OCTETS]);
        // a, a/b, a/b/F.bin: three enumerations — ten chunks would have cost thirty.
        expect(compte.values).toBe(3);
    });

    it('reuses the directories already resolved for a sibling file', async () => {
        const { adaptateur, compte } = monter({ a: { 'x.bin': OCTETS, 'y.bin': OCTETS } });
        await adaptateur.lire('a/x.bin', 0, 1);
        const avant = compte.values;
        await adaptateur.lire('a/y.bin', 0, 1);
        // Only `a` is enumerated again, to canonicalise `y.bin`.
        expect(compte.values - avant).toBe(1);
    });

    it('resolves again once TTL_MS has elapsed, even under continuous reads', async () => {
        const { adaptateur, compte, horloge } = monter({ 'f.bin': OCTETS });
        await adaptateur.lire('f.bin', 0, 1);
        horloge.avancer(TTL_MS - 1);
        await adaptateur.lire('f.bin', 1, 1);
        expect(compte.values).toBe(1);
        horloge.avancer(1);
        await adaptateur.lire('f.bin', 2, 1);
        expect(compte.values).toBe(2);
    });

    it('is emptied by forget()', async () => {
        const { adaptateur, compte } = monter({ 'f.bin': OCTETS });
        await adaptateur.lire('f.bin', 0, 1);
        adaptateur.forget?.();
        await adaptateur.lire('f.bin', 1, 1);
        expect(compte.values).toBe(2);
    });

    it('never retries a handle that no longer opens, and reports a fresh resolution', async () => {
        const arbre: Noeud = { 'f.bin': OCTETS };
        const { adaptateur } = monter(arbre);
        await adaptateur.lire('f.bin', 0, 1);
        delete arbre['f.bin'];
        await expect(adaptateur.lire('f.bin', 1, 1)).rejects.toMatchObject({ code: 'introuvable' });
        // Put back under ANOTHER case: only a fresh resolution finds it.
        arbre['F.BIN'] = OCTETS;
        expect([...(await adaptateur.lire('f.bin', 5, 2))]).toEqual([5, 6]);
    });
});

describe('createMemo', () => {
    it('forgets the oldest entry beyond MAX_ENTREES', () => {
        const memo = createMemo<number>(() => 0);
        for (let i = 0; i <= MAX_ENTREES; i += 1) memo.set(`k${i}`, i);
        expect(memo.get('k0')).toBeUndefined();
        expect(memo.get(`k${MAX_ENTREES}`)).toBe(MAX_ENTREES);
    });
});
