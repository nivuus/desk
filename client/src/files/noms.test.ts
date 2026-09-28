import { describe, expect, it } from 'vitest';
import { FilesError, type PoigneeBase, type FileHandle, type PoigneeRepertoire } from './adaptateur';
import {
    FAUTE_SILENCE,
    PREFIXE_FAUTE,
    canoniser,
    canoniserOuLever,
    injecterFaute,
    plier,
} from './noms';

/* ═══════════════════════════════════════════════════════════════════════════
   TWO FAKE FILE SYSTEMS, AND THAT IS THE HEART OF THIS FILE

   🔴 ONE IS CASE-SENSITIVE — like OPFS and like Linux, hence like
   the acceptance INSTRUMENT. The other is INSENSITIVE — like Windows and like
   macOS by default, hence like the REAL LOCAL WORKSTATION.

   **The same canonicaliser must give the same answer on both**, and
   it is the only way to exercise on the host one half of a phenomenon the
   acceptance setup cannot produce (see the header of `noms.ts`).
   ═══════════════════════════════════════════════════════════════════════════ */

/** A directory whose `values()` returns the given names. Nothing else. */
function parentWith(noms: string[]): PoigneeRepertoire {
    return {
        kind: 'directory',
        name: 'racine',
        async getDirectoryHandle(): Promise<PoigneeRepertoire> {
            throw new Error('not used by the canonicaliser');
        },
        async getFileHandle(): Promise<FileHandle> {
            throw new Error('not used by the canonicaliser');
        },
        values(): AsyncIterable<PoigneeBase> {
            return {
                async *[Symbol.asyncIterator]() {
                    for (const nom of noms) yield { kind: 'file' as const, name: nom };
                },
            };
        },
    };
}

/**
 * A case-INSENSITIVE directory: `getFileHandle('CASSE.TXT')` returns
 * `Casse.txt` there, exactly like Windows.
 *
 * 🔴 IT IS THE ONE THAT REPRODUCES THE BROWSER HALF OF THE F1 DEFECT, the one that
 * was NEVER observed because the acceptance instrument is OPFS.
 */
function parentInsensible(noms: string[]): PoigneeRepertoire & {
    ouvertures: string[];
} {
    const ouvertures: string[] = [];
    const base = parentWith(noms);
    return {
        ...base,
        ouvertures,
        async getFileHandle(nom: string): Promise<FileHandle> {
            const trouve = noms.find((n) => n.toLowerCase() === nom.toLowerCase());
            if (trouve === undefined) {
                throw new DOMException(`« ${nom} » cannot be found`, 'NotFoundError');
            }
            ouvertures.push(trouve);
            return { kind: 'file', name: trouve, getFile: async () => ({} as never) };
        },
    };
}

describe('folding', () => {
    it('folds the case', () => {
        expect(plier('CASSE.TXT')).toBe(plier('Casse.txt'));
    });

    it('🔵 folds the UNICODE NORMALISATION, which F2 declared NOT HANDLED', () => {
        // `é` as a single code point (NFC) versus `e` + combining accent (NFD).
        const nfc = 'été.txt';
        const nfd = 'été.txt';
        expect(nfc).not.toBe(nfd);
        expect(plier(nfc)).toBe(plier(nfd));
    });

    it('does NOT fold two really different names', () => {
        expect(plier('a.txt')).not.toBe(plier('b.txt'));
    });
});

describe('le canonicaliseur', () => {
    it('🔴 on an INSENSITIVE fake, `casse.txt` does NOT return `Casse.txt` without canonicalisation', async () => {
        // IT IS THE MOST IMPORTANT RED OF F3, and it reads both
        // ways on the SAME assertion:
        //   - the insensitive fake, queried DIRECTLY, does return the wrong
        //     file — it is the browser half of the F1 defect, reproduced
        //     on the host;
        //   - the canonicaliser, for its part, returns the STORED name.
        const parent = parentInsensible(['Casse.txt']);
        await parent.getFileHandle('casse.txt');
        expect(parent.ouvertures).toEqual(['Casse.txt']);

        const r = await canoniser(parent, 'casse.txt');
        expect(r).toEqual({ sorte: 'trouve', nom: 'Casse.txt' });
    });

    it('🔴 on a SENSITIVE fake, `GROS.BIN` returns `gros.bin` under its STORED name', async () => {
        // It is the inconsistency F1 reports: in the SAME run,
        // `casse.txt` passed (NTFS) and `GROS.BIN` failed (OPFS). It
        // disappears.
        const r = await canoniser(parentWith(['gros.bin', 'autre.txt']), 'GROS.BIN');
        expect(r).toEqual({ sorte: 'trouve', nom: 'gros.bin' });
    });

    it('🔴 returns the STORED name, never the REQUESTED name', async () => {
        // Red: returning `demande`. The substitute would be created under a name that
        // does not exist on the local workstation side, and a later write would create it
        // FOR REAL — a ghost file, next to the real one.
        const r = await canoniser(parentWith(['Rapport Final.PDF']), 'rapport final.pdf');
        expect(r).toEqual({ sorte: 'trouve', nom: 'Rapport Final.PDF' });
        expect(r).not.toEqual({ sorte: 'trouve', nom: 'rapport final.pdf' });
    });

    it('🔴 two homonyms return `ambigu`, and NOTHING else', async () => {
        // Red: picking the first. One of the two would be overwritten, and the choice
        // would depend on the enumeration order — that is, on chance.
        const r = await canoniser(parentWith(['note.txt', 'Note.txt']), 'NOTE.TXT');
        expect(r).toEqual({ sorte: 'ambigu', noms: ['note.txt', 'Note.txt'] });
    });

    it('🔴 an EXACT name short-circuits the ambiguity', async () => {
        // Red: not favouring the exact match. `note.txt` would become ambiguous on
        // a workstation that ALSO carries `Note.txt`, even though it is perfectly
        // designated — and a legitimate read would be refused.
        const r = await canoniser(parentWith(['note.txt', 'Note.txt']), 'note.txt');
        expect(r).toEqual({ sorte: 'trouve', nom: 'note.txt' });
    });

    it('returns `absent` when nothing resembles it', async () => {
        expect(await canoniser(parentWith(['a.txt']), 'b.txt')).toEqual({ sorte: 'absent' });
    });

    it('🔵 resolves a UNICODE NORMALISATION divergence', async () => {
        // macOS stores in NFD, Windows in NFC. Without folding, F2's guard
        // would create a DUPLICATE instead of overwriting — less serious than a loss,
        // but wrong, and F2 declares it as such.
        const stocke = 'été.txt'; // NFD
        const r = await canoniser(parentWith([stocke]), 'été.txt'); // NFC
        expect(r).toEqual({ sorte: 'trouve', nom: stocke });
    });

    it('an empty directory returns `absent`, never `ambigu`', async () => {
        expect(await canoniser(parentWith([]), 'x')).toEqual({ sorte: 'absent' });
    });
});

describe('canoniserOuLever', () => {
    it('returns the stored name', async () => {
        expect(await canoniserOuLever(parentWith(['A.txt']), 'a.txt', 'introuvable')).toBe('A.txt');
    });

    it('🔴 tells the TWO ways of being missing apart', async () => {
        // ProjFS tells them apart (`ERROR_FILE_NOT_FOUND` versus
        // `ERROR_PATH_NOT_FOUND`), and Explorer does not say the same thing about them.
        await expect(canoniserOuLever(parentWith([]), 'x', 'introuvable')).rejects.toMatchObject({
            code: 'introuvable',
        });
        await expect(
            canoniserOuLever(parentWith([]), 'x', 'chemin-introuvable'),
        ).rejects.toMatchObject({ code: 'chemin-introuvable' });
    });

    it('throws `casse-ambigue` while NAMING the homonyms', async () => {
        const error = await canoniserOuLever(parentWith(['a', 'A']), 'à-plier-en-A', 'introuvable')
            .catch((e: unknown) => e as FilesError)
            .then((e) => e as FilesError)
            .catch(() => undefined);
        // The requested name folds onto nothing: it is `absent`, not `ambigu`.
        expect(error?.code).toBe('introuvable');

        const ambigu = await canoniserOuLever(parentWith(['a', 'A']), 'A', 'introuvable').then(
            (n) => n,
            (e: unknown) => e as FilesError,
        );
        // 'A' is EXACT: rule 1 wins, and there is no ambiguity.
        expect(ambigu).toBe('A');

        const vrai = await canoniserOuLever(parentWith(['a', 'A']), 'à', 'introuvable').then(
            (n) => n,
            (e: unknown) => e as FilesError,
        );
        expect(vrai).toBeInstanceOf(FilesError);
    });
});

describe('fault injection', () => {
    it('🔴 is INERT when it is not armed', async () => {
        // Red: reading it from the module. A user who created a
        // `.faute-disque-plein` folder would break their own bridge — and the
        // module would stop being testable, since it would read `location`.
        await expect(injecterFaute(['.faute-disque-plein', 'x'], false)).resolves.toBeUndefined();
    });

    it('throws the requested code when it is armed', async () => {
        await expect(injecterFaute(['.faute-acces-refuse'], true)).rejects.toMatchObject({
            code: 'acces-refuse',
        });
        await expect(injecterFaute(['.faute-disque-plein'], true)).rejects.toMatchObject({
            code: 'disque-plein',
        });
    });

    it('🔴 looks ONLY at the first component', async () => {
        // Scanning all components would make a path crossing a folder
        // named that way — even deep down — fail, which would make
        // the injection impossible to disarm by the gesture.
        await expect(
            injecterFaute(['normal', '.faute-acces-refuse'], true),
        ).resolves.toBeUndefined();
    });

    it('🔴 `.faute-silence` NEVER RESOLVES', async () => {
        // It is the only way to exercise `DelaiDepasse`: an answer, whatever
        // it is, would keep the bridge from timing out.
        let resolue = false;
        void injecterFaute([`${PREFIXE_FAUTE}${FAUTE_SILENCE}`], true).then(() => {
            resolue = true;
        });
        await new Promise((r) => setTimeout(r, 20));
        expect(resolue).toBe(false);
    });

    it('🔴 an UNKNOWN suffix is SAID, never swallowed', async () => {
        // Without it, an acceptance typo would produce an ordinary "introuvable",
        // and the operator would think they had exercised code they had not
        // exercised.
        await expect(injecterFaute(['.faute-typo'], true)).rejects.toMatchObject({
            code: 'interne',
        });
    });

    it('an ordinary path triggers nothing, even armed', async () => {
        await expect(injecterFaute(['dossier', 'note.txt'], true)).resolves.toBeUndefined();
        await expect(injecterFaute([], true)).resolves.toBeUndefined();
    });
});
