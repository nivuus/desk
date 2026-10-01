import { describe, expect, it } from 'vitest';
import { createAdapter, type FileHandle, type PoigneeRepertoire } from './adaptateur';
import { MAX_ENTREES, TTL_MS, createMemo } from './memo';

/* A tree whose directories COUNT their enumerations: it is the cost `memo.ts`
   removes, and a fake that did not count it could not see it come back. */

type Noeud = { [nom: string]: Noeud | Uint8Array };

function monter(arbre: Noeud) {
    const compte = { values: 0 };
    /** Names whose NEXT `getFile()` fails once, as a file held by another program would. */
    const pannes = new Set<string>();
    /** Runs at the start of the next enumeration only: "something happens mid-resolution". */
    const pendant: { values?: () => void } = {};
    function dossier(nom: string, n: Noeud): PoigneeRepertoire {
        const fichier = (enfant: string): FileHandle => ({
            kind: 'file',
            name: enfant,
            getFile: async () => {
                if (pannes.delete(enfant)) throw new DOMException(enfant, 'NotReadableError');
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
                const crochet = pendant.values;
                pendant.values = undefined;
                crochet?.();
                for (const [enfant, e] of Object.entries(n)) {
                    yield e instanceof Uint8Array ? fichier(enfant) : dossier(enfant, e);
                }
            },
        };
    }
    let temps = 0;
    const horloge = { avancer: (ms: number) => (temps += ms) };
    const adaptateur = createAdapter(dossier('', arbre), false, () => temps);
    return { adaptateur, compte, horloge, pannes, pendant };
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

describe("lire's memory under concurrency", () => {
    it('shares ONE resolution between chunks asked for at the same time', async () => {
        const { adaptateur, compte } = monter({ 'f.bin': OCTETS });
        // The agent's window: four chunks requested before any answer.
        const lus = await Promise.all([0, 10, 20, 30].map((p) => adaptateur.lire('f.bin', p, 10)));
        expect(lus.flatMap((l) => [...l])).toEqual([...OCTETS.slice(0, 40)]);
        expect(compte.values).toBe(1);
    });

    it('does not keep a resolution that was under way when the memory was emptied', async () => {
        const { adaptateur, compte, pendant } = monter({ 'f.bin': OCTETS });
        // A rename lands WHILE the read enumerates the parent.
        pendant.values = () => adaptateur.forget?.();
        await adaptateur.lire('f.bin', 0, 1);
        await adaptateur.lire('f.bin', 1, 1);
        expect(compte.values).toBe(2);
    });

    it('does not keep a freshly resolved handle whose first read fails', async () => {
        const { adaptateur, compte, pannes } = monter({ 'f.bin': OCTETS });
        pannes.add('f.bin');
        await expect(adaptateur.lire('f.bin', 0, 1)).rejects.toBeDefined();
        expect([...(await adaptateur.lire('f.bin', 1, 1))]).toEqual([1]);
        // Resolved again, rather than the failed handle retried first.
        expect(compte.values).toBe(2);
    });
});

describe('createMemo', () => {
    it('refuses a value resolved under a generation a clear() made stale', () => {
        const memo = createMemo<number>(() => 0);
        const avant = memo.generation();
        memo.clear();
        memo.set('k', 1, avant);
        expect(memo.get('k')).toBeUndefined();
    });

    it('forgets the oldest entry beyond MAX_ENTREES', () => {
        const memo = createMemo<number>(() => 0);
        for (let i = 0; i <= MAX_ENTREES; i += 1) memo.set(`k${i}`, i, memo.generation());
        expect(memo.get('k0')).toBeUndefined();
        expect(memo.get(`k${MAX_ENTREES}`)).toBe(MAX_ENTREES);
    });
});
