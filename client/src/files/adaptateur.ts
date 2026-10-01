// The adapter between the bridge's logical paths and the browser's File System Access
// API.
//
// 🔴 THE ROOT IS INJECTED, NEVER IMPORTED (spec §4.4). It is the seam that
// makes this file testable: Vitest runs under Node, which has no FSA. Without
// it, path resolution, range splitting and error classification
// would be exercised by nothing — and those are exactly the three
// places where the old bridge was wrong.
//
// This module knows neither the DOM, nor WebRTC, nor the binary frame: it returns
// values and raises `FilesError`. It is `protocole.ts` that puts them on the
// wire.
//
// ✅ CASE IS HANDLED SINCE F3, ON READ AS ON WRITE.
//
// ❌ These lines said: "ON READ, CASE IS HANDLED NOWHERE, AND
// IT IS A DECLARED LEGACY", then described the defect measured by F1 and
// concluded "the complete remedy, a correspondence table fed by
// enumeration, remains F3". **F3 has arrived, and the remedy is NOT a correspondence
// table**: it is `files/noms.ts`, which enumerates the parent at
// EACH resolution, **without any cache**. A cache nothing invalidates is the
// defect of the old bridge (`src/file.js`, cache WITHOUT TTL). ✅ `Rafraichir` HAS BEEN
// DELIVERED SINCE F5. ⚠️ But it empties the bridge's ENUMERATION cache and ProjFS's
// NEGATIVE cache — `noms.ts`, for its part, still has no cache, hence nothing to
// empty: the two objects are distinct, and confusing them would make one believe
// case had become cheaper.
//
// Each path component therefore goes through `canoniser`, and this module returns the
// **STORED** name, never the requested one.
//
// ⚠️ WHAT F3 DOES NOT FIX, AND WHAT IS NOT REPAIRABLE HERE: the VM half of the
// phenomenon. NTFS resolves case on an ALREADY HYDRATED file without ever
// reaching this module — and when NTFS answers, we are not consulted.
// It is the NORMAL behaviour of Windows, and the header of `noms.ts`
// details it.
//
// ⚠️ THE COST IS REAL AND IT IS DECLARED: one enumeration of the parent per
// resolved component, on top of the `getFile()` per entry the listing already pays.
// **F3 trades latency for correctness**, and it is F4 that will say what
// the trade costs.
//
// ⛔ **F4 DID NOT SAY IT** (August 21st, 2026): no gesture of its campaign exercises
// case canonicalisation. **The cost remains OWED.**
//
// 🔵 **`lire` NO LONGER PAYS IT PER CHUNK** (October 1st, 2026): it resolves a
// file once and keeps the handle in a SHORT, BOUNDED, EMPTIED memory
// (`memo.ts`, whose header says why it is not the old bridge's cache).
// `lister` and `attributs` still resolve afresh at every call.

import type { CodeEchec } from '../../../proto/ts/fichiers';
import { MAX_FRAME_SIZE } from '../../../proto/ts/fichiers';
import type { EnteteMeta, EntreeJson } from '../../../proto/ts/fichiers-entetes';
import { creerMemo } from './memo';
import { canoniserOuLever, injecterFaute } from './noms';

/* ── THE HANDLES, DESCRIBED BY WHAT WE USE OF THEM ────────────────────────
   These interfaces are a STRUCTURAL SUBSET of `FileSystemDirectoryHandle`,
   `FileSystemFileHandle`, `File` and `Blob`: the real handle satisfies them
   without conversion (`canal.ts` checks it at compile time), and an in-memory
   fake does too. Describing them here rather than importing the DOM types keeps
   this module usable under Node. */

/** What we can do with a slice: read its bytes. */
export interface TrancheLisible {
    arrayBuffer(): Promise<ArrayBuffer>;
}

/**
 * The subset of `File` we use.
 *
 * ⚠️ `arrayBuffer()` APPEARS IN THIS TYPE ALTHOUGH THE CODE MUST NEVER
 * CALL IT, and it is deliberate: the real `File` exposes it, and a type that
 * hid it would lie about what is injected. Above all, it is what lets the
 * fake of `adaptateur.test.ts` COUNT its calls, hence the range test
 * be seen red. A type forbidding the call would replace an executed
 * check with a compiler promise — stronger in appearance, but we would never
 * have seen it fail.
 */
export interface ReadableFile {
    readonly size: number;
    readonly lastModified: number;
    slice(debut: number, fin: number): TrancheLisible;
    arrayBuffer(): Promise<ArrayBuffer>;
}

/**
 * What every handle carries, whatever its kind.
 *
 * ⚠️ IT IS WHAT `values()` RETURNS, AND NOT THE UNION OF THE TWO KINDS — because
 * it is all TypeScript's DOM library guarantees:
 * `FileSystemDirectoryHandle.values()` is typed there as
 * `AsyncIterator<FileSystemHandle>`, the BASE class, whereas the real API
 * returns the concrete subtypes. Declaring the union here would make the real handle
 * NOT assignable, and the compatibility check of `canal.ts` would fail on
 * a divergence of the library, not of the product. The adapter therefore narrows
 * down to `FileHandle` after reading `kind` — the same thing
 * TypeScript would do on its own if the union were declared.
 */
export interface PoigneeBase {
    readonly kind: 'file' | 'directory';
    readonly name: string;
}

export interface FileHandle extends PoigneeBase {
    readonly kind: 'file';
    getFile(): Promise<ReadableFile>;
}

export interface PoigneeRepertoire extends PoigneeBase {
    readonly kind: 'directory';
    getDirectoryHandle(nom: string): Promise<PoigneeRepertoire>;
    getFileHandle(nom: string): Promise<FileHandle>;
    values(): AsyncIterable<PoigneeBase>;
}

/** The root chosen by the user: a directory, and nothing else. */
export type Racine = PoigneeRepertoire;

/**
 * A failure CARRYING ITS CODE.
 *
 * 🔴 It is the answer to the defect found in the old bridge: `web/index.js:669`
 * emitted `JSON.stringify(e)`, which returns `"{}"` for any `Error`, and
 * `src/file.js:127` rebuilt a `new Error("{}")` on arrival. The cause
 * was destroyed at emission, and no one could recover it.
 *
 * Here the cause travels as a `CodeEchec`, a member of the shared enumeration
 * `proto::files::CodeEchec`: it crosses the wire without losing
 * anything, and the agent translates it back into an `HRESULT`. The `message`, for its part, does not
 * cross — it is what one reads in the browser console.
 */
export class FilesError extends Error {
    readonly code: CodeEchec;

    constructor(code: CodeEchec, message: string) {
        super(message);
        this.name = 'EchecFichiers';
        this.code = code;
    }
}

/**
 * Classifies an exception coming from the File System Access API.
 *
 * `siAbsent` distinguishes the two ways of being not found, which ProjFS
 * distinguishes too (`ERROR_FILE_NOT_FOUND` versus `ERROR_PATH_NOT_FOUND`) and
 * about which Explorer does not say the same thing: a missing INTERMEDIATE component
 * returns `chemin-introuvable`, the FINAL component returns `introuvable`.
 */
export function classer(e: unknown, siAbsent: CodeEchec): FilesError {
    if (e instanceof FilesError) return e;
    const nom = e instanceof DOMException ? e.name : '';
    const texte = e instanceof Error ? e.message : String(e);
    switch (nom) {
        case 'NotFoundError':
        case 'TypeMismatchError':
            return new FilesError(siAbsent, texte);
        case 'NotAllowedError':
        case 'SecurityError':
            return new FilesError('acces-refuse', texte);
        // ── THE TWO CAUSES OF F2 ──────────────────────────────────────────
        // They did not exist in read-only mode, and without them both
        // would fall into `interne`: the log would no longer say WHY a
        // write failed, and the user would not know whether to free
        // space or grant a permission back.
        //
        // ⚠️ **`TypeMismatchError` IS NOT CLASSIFIED AS `deja-present`**, against
        // the letter of F2's plan: it is ALREADY classified as absence, two lines
        // above, and that is what lets `attributs` retry as a
        // file after failing as a directory. Classifying it twice is
        // impossible; classifying it here would break reading.
        case 'QuotaExceededError':
            return new FilesError('disque-plein', texte);
        case 'InvalidModificationError':
            return new FilesError('deja-present', texte);
        default:
            // Everything else is `interne`: inventing a more precise code
            // would amount to guessing, and the agent would translate it into a wrong
            // HRESULT rather than a vague one.
            return new FilesError('interne', texte);
    }
}

/** Can an absence failure be recovered by trying the other kind? */
function estAbsence(e: unknown): boolean {
    return e instanceof DOMException && (e.name === 'NotFoundError' || e.name === 'TypeMismatchError');
}

/** `"a/b/c"` → `["a","b","c"]`, `""` → `[]`. */
function composants(chemin: string): string[] {
    return chemin.split('/').filter((c) => c.length > 0);
}

export interface Adaptateur {
    lister(chemin: string): Promise<EntreeJson[]>;
    attributs(chemin: string): Promise<EnteteMeta>;
    lire(chemin: string, position: number, length: number): Promise<Uint8Array>;
    /**
     * Forgets the resolutions `lire` memorised (`memo.ts`). Optional so that
     * a fake adapter need not carry it; the real one always does.
     */
    oublier?(): void;
}

export function createAdapter(
    racine: Racine,
    fautesArmees = false,
    maintenant: () => number = Date.now,
): Adaptateur {
    /**
     * 🔵 `lire`'s memory: `r:<path>` holds a directory, `f:<path>` a file.
     * See `memo.ts` for why it is short, bounded and emptied — and why
     * `lister` and `attributs` do NOT use it: they keep resolving afresh.
     */
    const memo = creerMemo<PoigneeRepertoire | FileHandle>(maintenant);

    /**
     * [`descendre`], but starting from the DEEPEST directory `lire` already
     * resolved, and remembering the ones it resolves.
     */
    async function descendreMemo(parts: string[], jusqua: number): Promise<PoigneeRepertoire> {
        let i = jusqua;
        let ici: PoigneeRepertoire = racine;
        for (; i > 0; i -= 1) {
            const connu = memo.obtenir(`r:${parts.slice(0, i).join('/')}`);
            if (connu !== undefined && connu.kind === 'directory') {
                ici = connu;
                break;
            }
        }
        for (; i < jusqua; i += 1) {
            const nom = await canoniserOuLever(ici, parts[i], 'chemin-introuvable');
            try {
                ici = await ici.getDirectoryHandle(nom);
            } catch (e) {
                throw classer(e, 'chemin-introuvable');
            }
            memo.poser(`r:${parts.slice(0, i + 1).join('/')}`, ici);
        }
        return ici;
    }

    /** Resolves the file `parts` designates — canonicalising, as always. */
    async function resoudreFichier(parts: string[]): Promise<FileHandle> {
        const parent = await descendreMemo(parts, parts.length - 1);
        const nom = await canoniserOuLever(parent, parts[parts.length - 1], 'introuvable');
        try {
            return await parent.getFileHandle(nom);
        } catch (e) {
            throw classer(e, 'introuvable');
        }
    }

    /** The bytes `[position, position + length[` of `poignee`, clamped to its size. */
    async function trancher(
        poignee: FileHandle,
        position: number,
        length: number,
    ): Promise<Uint8Array> {
        let file: ReadableFile;
        try {
            file = await poignee.getFile();
        } catch (e) {
            throw classer(e, 'introuvable');
        }
        // 🔴 `slice` THEN `arrayBuffer`, NEVER THE REVERSE. Reading the whole
        // file to return 4 KB of it is the defect found in the old bridge
        // (`web/index.js:562-564`): on a one-gigabyte file, each
        // ProjFS read would materialise it in memory.
        const debut = Math.min(position, file.size);
        const fin = Math.min(position + length, file.size);
        try {
            return new Uint8Array(await file.slice(debut, fin).arrayBuffer());
        } catch (e) {
            throw classer(e, 'introuvable');
        }
    }

    /**
     * Walks down the first `jusqua` components, all directories, **while
     * CANONICALISING them**.
     */
    async function descendre(parts: string[], jusqua: number): Promise<PoigneeRepertoire> {
        let ici = racine;
        for (let i = 0; i < jusqua; i += 1) {
            // An INTERMEDIATE component: the path itself is at fault, and
            // ProjFS distinguishes the two (`ERROR_PATH_NOT_FOUND` versus
            // `ERROR_FILE_NOT_FOUND`).
            const nom = await canoniserOuLever(ici, parts[i], 'chemin-introuvable');
            try {
                ici = await ici.getDirectoryHandle(nom);
            } catch (e) {
                throw classer(e, 'chemin-introuvable');
            }
        }
        return ici;
    }

    async function fileMeta(nom: string, f: FileHandle): Promise<EnteteMeta> {
        const file = await f.getFile();
        // 🔴 **`nom` IS THE STORED NAME**, the one the canonicaliser returned —
        // and it is the one `PrjWritePlaceholderInfo` will receive.
        return { nom, repertoire: false, taille: file.size, modifie: file.lastModified }; // policy: allow-fr - wire keys of the file protocol
    }

    return {
        async lister(chemin) {
            const parts = composants(chemin);
            await injecterFaute(parts, fautesArmees);
            const parent = await descendre(parts, Math.max(parts.length - 1, 0));
            let dossier = parent;
            if (parts.length > 0) {
                const nom = await canoniserOuLever(parent, parts[parts.length - 1], 'introuvable');
                try {
                    dossier = await parent.getDirectoryHandle(nom);
                } catch (e) {
                    throw classer(e, 'introuvable');
                }
            }
            const entrees: EntreeJson[] = [];
            try {
                for await (const enfant of dossier.values()) {
                    if (enfant.kind === 'directory') {
                        // ⚠️ The FSA exposes NEITHER size NOR timestamp of a
                        // directory. Zero is what ProjFS expects of a
                        // directory for the size; the null timestamp is an
                        // accepted loss, and saying so avoids anyone looking for it.
                        entrees.push({ nom: enfant.name, repertoire: true, taille: 0, modifie: 0 }); // policy: allow-fr - wire keys of the file protocol
                    } else {
                        // ⚠️ ACCEPTED COST: one `getFile()` per entry. The FSA
                        // offers no way to obtain size and date without
                        // opening the file, and ProjFS requires both in its
                        // enumeration. A directory of a thousand entries costs
                        // a thousand openings. ✅ **MEASURED BY F4**: a listing
                        // of 1,000 entries costs **~6.0 s** end to end
                        // (two runs), of which ~3.0 s per traversal and
                        // TWO traversals per `Get-ChildItem`. These thousand
                        // `getFile()` are INSIDE and are not isolated:
                        // F4 measures the traversal, never what composes it.
                        // Explicit narrowing: `kind` is `'file'`, so the
                        // handle IS a `FileHandle`. See the note on
                        // `PoigneeBase` — it is the DOM library that
                        // subtypes `values()`, not the API.
                        const f = await (enfant as FileHandle).getFile();
                        entrees.push({
                            nom: enfant.name,
                            repertoire: false,
                            taille: f.size, // policy: allow-fr - wire key of the file protocol
                            modifie: f.lastModified, // policy: allow-fr - wire key of the file protocol
                        });
                    }
                }
            } catch (e) {
                throw classer(e, 'introuvable');
            }
            return entrees;
        },

        async attributs(chemin) {
            const parts = composants(chemin);
            await injecterFaute(parts, fautesArmees);
            if (parts.length === 0) {
                // ⚠️ The ROOT has no name: `nom` is the empty string, and
                // `PrjWritePlaceholderInfo` is never called
                // for it anyway.
                return { nom: '', repertoire: true, taille: 0, modifie: 0 }; // policy: allow-fr - wire keys of the file protocol
            }
            const parent = await descendre(parts, parts.length - 1);
            // 🔴 **RESOLUTION IS DONE ONCE, HERE**, and the name obtained
            // serves BOTH attempts — directory then file. Redoing it
            // twice would cost two enumerations of the parent for the same
            // question.
            const last = await canoniserOuLever(parent, parts[parts.length - 1], 'introuvable');
            try {
                await parent.getDirectoryHandle(last);
                return { nom: last, repertoire: true, taille: 0, modifie: 0 }; // policy: allow-fr - wire keys of the file protocol
            } catch (e) {
                // We retry AS A FILE only if the failure is an absence. A
                // retried permission refusal would be masked as "not found",
                // and the user would look for a file instead of granting
                // access back.
                if (!estAbsence(e)) throw classer(e, 'introuvable');
            }
            try {
                return await fileMeta(last, await parent.getFileHandle(last));
            } catch (e) {
                throw classer(e, 'introuvable');
            }
        },

        async lire(chemin, position, length) {
            await injecterFaute(composants(chemin), fautesArmees);
            if (length > MAX_FRAME_SIZE) {
                // The peer is granted no trust on the size
                // it requests: `pont::decoupe` already bounds on the agent side, but
                // it is the agent doing it, hence the other end of the wire.
                throw new FilesError(
                    'trop-grand',
                    `${length} bytes requested, maximum ${MAX_FRAME_SIZE}`,
                );
            }
            const parts = composants(chemin);
            if (parts.length === 0) {
                throw new FilesError('introuvable', 'the root is not a file');
            }
            // 🔴 **ONE RESOLUTION PER FILE, NOT PER CHUNK** (`memo.ts`). The
            // chunks of one read share the handle; the name it was resolved
            // under is the canonical one, exactly as before.
            const cle = `f:${parts.join('/')}`;
            const connue = memo.obtenir(cle);
            if (connue !== undefined && connue.kind === 'file') {
                try {
                    return await trancher(connue, position, length);
                } catch {
                    // The handle no longer opens: the file was renamed,
                    // removed, replaced. We never retry a handle as is — we
                    // forget everything and resolve again, below, so that the
                    // failure returned is the one a fresh resolution gives.
                    memo.oublier();
                }
            }
            const poignee = await resoudreFichier(parts);
            memo.poser(cle, poignee);
            return trancher(poignee, position, length);
        },

        oublier() {
            memo.oublier();
        },
    };
}
