import { describe, expect, it } from 'vitest';
import {
    createAdapter,
    FilesError,
    type ReadableFile,
    type FileHandle,
    type PoigneeRepertoire,
    type TrancheLisible,
} from './adaptateur';

/* ── A FAKE IN-MEMORY FILE SYSTEM ─────────────────────────────────────────
   The File System Access API does not exist under Node: without injecting the
   root (spec §4.4), NONE of these tests would exist.

   🔴 THE FAKE `File` COUNTS ITS CALLS, and that is what gives the range test
   its value. A fake that simply returned the right bytes would pass just as
   well with `slice(o, o+n).arrayBuffer()` as with `arrayBuffer()` followed by a
   cut on the caller's side — that is, it would be VACUOUS. The targeted defect is
   observed, not imagined: `web/index.js:562-564` read the WHOLE file to
   return a range of it. */

interface Compteurs {
    arrayBufferEntier: number;
    slice: number;
}

function fakeFile(octets: Uint8Array, modified: number, compteurs: Compteurs): ReadableFile {
    return {
        size: octets.length,
        lastModified: modified,
        slice(debut: number, fin: number): TrancheLisible {
            compteurs.slice += 1;
            const tranche = octets.slice(debut, fin);
            return { arrayBuffer: async () => tranche.buffer.slice(tranche.byteOffset, tranche.byteOffset + tranche.byteLength) as ArrayBuffer };
        },
        arrayBuffer: async () => {
            compteurs.arrayBufferEntier += 1;
            return octets.buffer.slice(octets.byteOffset, octets.byteOffset + octets.byteLength) as ArrayBuffer;
        },
    };
}

type Arbre = { [nom: string]: Arbre | { octets: Uint8Array; modifie: number } };

function isFile(n: Arbre[string]): n is { octets: Uint8Array; modifie: number } {
    return 'octets' in n && n.octets instanceof Uint8Array;
}

/** `DOMException` is available under Node ≥ 17; we use it as is. */
function absent(nom: string): never {
    throw new DOMException(`« ${nom} » cannot be found`, 'NotFoundError');
}

function mauvaisType(nom: string): never {
    throw new DOMException(`« ${nom} » is not of the requested type`, 'TypeMismatchError');
}

function repertoire(nom: string, arbre: Arbre, compteurs: Compteurs): PoigneeRepertoire {
    return {
        kind: 'directory',
        name: nom,
        async getDirectoryHandle(enfant: string): Promise<PoigneeRepertoire> {
            const n = arbre[enfant];
            if (n === undefined) absent(enfant);
            if (isFile(n)) mauvaisType(enfant);
            return repertoire(enfant, n, compteurs);
        },
        async getFileHandle(enfant: string): Promise<FileHandle> {
            const n = arbre[enfant];
            if (n === undefined) absent(enfant);
            if (!isFile(n)) mauvaisType(enfant);
            return {
                kind: 'file',
                name: enfant,
                getFile: async () => fakeFile(n.octets, n.modifie, compteurs),
            };
        },
        async *values() {
            for (const [enfant, n] of Object.entries(arbre)) {
                yield isFile(n)
                    ? {
                          kind: 'file' as const,
                          name: enfant,
                          getFile: async () => fakeFile(n.octets, n.modifie, compteurs),
                      }
                    : repertoire(enfant, n, compteurs);
            }
        },
    };
}

const CONTENU = new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

function monter() {
    const compteurs: Compteurs = { arrayBufferEntier: 0, slice: 0 };
    const arbre: Arbre = {
        'note.txt': { octets: new Uint8Array([65, 66, 67]), modifie: 1_690_000_000_000 },
        'gros.bin': { octets: CONTENU, modifie: 42 },
        dossier: {
            'dedans.txt': { octets: new Uint8Array([88]), modifie: -86_400_000 },
        },
    };
    return { compteurs, adaptateur: createAdapter(repertoire('', arbre, compteurs)) };
}

/** The same tree, with fault injection ARMED. */
function monterArme() {
    const compteurs: Compteurs = { arrayBufferEntier: 0, slice: 0 };
    const arbre: Arbre = { 'note.txt': { octets: new Uint8Array([1]), modifie: 0 } };
    return createAdapter(repertoire('', arbre, compteurs), true);
}

describe('File System Access API adapter', () => {
    it('listing returns the entries with their kind', async () => {
        const { adaptateur } = monter();
        const entrees = await adaptateur.lister('');
        const parNom = new Map(entrees.map((e) => [e.nom, e]));
        expect(parNom.get('dossier')?.repertoire).toBe(true);
        expect(parNom.get('note.txt')?.repertoire).toBe(false);
        expect(parNom.get('note.txt')?.taille).toBe(3);
        expect(parNom.get('note.txt')?.modifie).toBe(1_690_000_000_000);
    });

    it('listing the root takes the empty path', async () => {
        const { adaptateur } = monter();
        // The root has no name: the empty logical path designates it, and
        // that is what `pont::chemins` normalises on the agent side.
        expect((await adaptateur.lister('')).length).toBe(3);
        expect((await adaptateur.lister('dossier')).map((e) => e.nom)).toEqual(['dedans.txt']);
    });

    it('attributs returns the size, the timestamp and the CANONICAL NAME', async () => {
        const { adaptateur } = monter();
        expect(await adaptateur.attributs('note.txt')).toEqual({
            nom: 'note.txt',
            repertoire: false,
            taille: 3,
            modifie: 1_690_000_000_000,
        });
        expect(await adaptateur.attributs('dossier')).toEqual({
            nom: 'dossier',
            repertoire: true,
            taille: 0,
            modifie: 0,
        });
        // ⚠️ The ROOT has no name.
        expect(await adaptateur.attributs('')).toEqual({
            nom: '',
            repertoire: true,
            taille: 0,
            modifie: 0,
        });
    });

    it('🔴 reading a range reads ONLY that range', async () => {
        const { adaptateur, compteurs } = monter();
        const octets = await adaptateur.lire('gros.bin', 4, 3);
        expect([...octets]).toEqual([4, 5, 6]);
        // 🔴 THE HEART OF THE TEST: the whole file is NEVER materialised.
        expect(compteurs.arrayBufferEntier).toBe(0);
        expect(compteurs.slice).toBe(1);
    });

    it('reading past the end returns fewer bytes, without throwing', async () => {
        const { adaptateur } = monter();
        const octets = await adaptateur.lire('gros.bin', 8, 100);
        expect([...octets]).toEqual([8, 9]);
        // Entirely beyond: zero bytes, still without throwing.
        expect((await adaptateur.lire('gros.bin', 50, 10)).length).toBe(0);
    });

    it('🔴 a non-existent path returns the Introuvable code', async () => {
        const { adaptateur } = monter();
        await expect(adaptateur.attributs('absent.txt')).rejects.toBeInstanceOf(FilesError);
        await expect(adaptateur.attributs('absent.txt')).rejects.toMatchObject({
            code: 'introuvable',
        });
        await expect(adaptateur.lire('absent.txt', 0, 1)).rejects.toMatchObject({
            code: 'introuvable',
        });
    });

    it('tells a missing final component from a missing PARENT', async () => {
        const { adaptateur } = monter();
        // ProjFS tells ERROR_FILE_NOT_FOUND from ERROR_PATH_NOT_FOUND, and
        // Explorer does not say the same thing about the two.
        await expect(adaptateur.attributs('nulle-part/note.txt')).rejects.toMatchObject({
            code: 'chemin-introuvable',
        });
        await expect(adaptateur.lister('note.txt/encore')).rejects.toMatchObject({
            code: 'chemin-introuvable',
        });
    });

    it('🔴 an FSA error becomes a CODE, never a string', async () => {
        const { adaptateur } = monter();
        // 🔴 THE EXACT DEFECT OF THE OLD BRIDGE, exercised right here:
        // `web/index.js:669` did `JSON.stringify(e)` of an `Error`, which
        // returns `"{}"`, and `src/file.js:127` rebuilt it as
        // `new Error("{}")`. The whole cause was destroyed AT EMISSION.
        expect(JSON.stringify(new Error('permission refused'))).toBe('{}');

        const echec = await adaptateur.attributs('absent.txt').catch((e: unknown) => e);
        expect(echec).toBeInstanceOf(FilesError);
        // What the adapter produces instead: a member of the SHARED
        // enumeration, which crosses the wire without losing anything.
        expect((echec as FilesError).code).toBe('introuvable');
        // And the message stays readable for a human, on the browser side — it does
        // not cross the wire, but it is what one reads in the console.
        expect((echec as FilesError).message).toMatch(/absent\.txt/);
    });

    it('a permission refusal becomes acces-refuse, not internal', async () => {
        const refusante: PoigneeRepertoire = {
            kind: 'directory',
            name: '',
            async getDirectoryHandle() {
                throw new DOMException('permission revoked', 'NotAllowedError');
            },
            async getFileHandle() {
                throw new DOMException('permission revoked', 'NotAllowedError');
            },
            async *values(): AsyncGenerator<FileHandle | PoigneeRepertoire> {
                throw new DOMException('permission revoked', 'NotAllowedError');
            },
        };
        const adaptateur = createAdapter(refusante);
        await expect(adaptateur.lister('')).rejects.toMatchObject({ code: 'acces-refuse' });
        await expect(adaptateur.attributs('x')).rejects.toMatchObject({ code: 'acces-refuse' });
    });

    it('an unexpected failure becomes internal, never an invented code', async () => {
        const cassee: PoigneeRepertoire = {
            kind: 'directory',
            name: '',
            async getDirectoryHandle() {
                throw new Error('something blew up');
            },
            async getFileHandle() {
                throw new Error('something blew up');
            },
            async *values(): AsyncGenerator<FileHandle | PoigneeRepertoire> {
                throw new Error('something blew up');
            },
        };
        await expect(createAdapter(cassee).lister('')).rejects.toMatchObject({
            code: 'interne',
        });
    });
});

describe('case on READ, fixed by F3', () => {
    // 🔴 THIS FAKE IS CASE-SENSITIVE — like OPFS, hence like the acceptance
    // INSTRUMENT. That is precisely where F1 measured its inconsistency:
    // `casse.txt` passed (resolved by NTFS without us) and `GROS.BIN` failed
    // (it reached the bridge and hit OPFS), IN THE SAME RUN.

    it('🔴 `GROS.BIN` returns `gros.bin`, and the F1 inconsistency DISAPPEARS', async () => {
        // Red: keeping direct resolution (`getFileHandle(last)`).
        // `GROS.BIN` would return `introuvable`, which is exactly the state
        // F1 reports.
        const { adaptateur } = monter();
        const meta = await adaptateur.attributs('GROS.BIN');
        expect(meta.repertoire).toBe(false);
        // 🔴 AND THE NAME RETURNED IS THE STORED NAME, never the one requested.
        expect(meta.nom).toBe('gros.bin');
    });

    it('reads the content through a different case', async () => {
        const { adaptateur } = monter();
        const octets = await adaptateur.lire('NOTE.TXT', 0, 3);
        expect([...octets]).toEqual([65, 66, 67]);
    });

    it('resolves an INTERMEDIATE component by case, and tells it apart', async () => {
        const { adaptateur } = monter();
        expect(await adaptateur.lister('DOSSIER')).toHaveLength(1);
        // An absent intermediate component returns `chemin-introuvable`, never
        // `introuvable`: ProjFS tells the two apart, and Explorer does not say
        // the same thing about them.
        await expect(adaptateur.lire('ABSENT/x.txt', 0, 1)).rejects.toMatchObject({
            code: 'chemin-introuvable',
        });
    });
});

describe('fault injection, DISARMED by default', () => {
    it('🔴 is INERT when it is not armed', async () => {
        // Red: reading it from the module instead of receiving it as an argument.
        // A user who created a `.faute-disque-plein` folder
        // would break their own bridge.
        const { adaptateur } = monter();
        await expect(adaptateur.lister('.faute-disque-plein')).rejects.toMatchObject({
            // No fault: it is an ordinary path, hence absent.
            code: 'introuvable',
        });
    });

    it('throws the requested code when it is armed', async () => {
        const a = monterArme();
        await expect(a.lister('.faute-disque-plein')).rejects.toMatchObject({
            code: 'disque-plein',
        });
        await expect(a.attributs('.faute-acces-refuse/x')).rejects.toMatchObject({
            code: 'acces-refuse',
        });
        await expect(a.lire('.faute-non-supporte', 0, 1)).rejects.toMatchObject({
            code: 'non-supporte',
        });
    });
});
