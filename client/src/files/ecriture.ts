// The writer: it lays the bridge's bytes into the local machine's directory.
// **PURE** — neither DOM, nor WebRTC, nor binary frame; the root is INJECTED into it,
// as in `adaptateur.ts`, and that is what makes it testable under Vitest's
// Node, which has no File System Access API.
//
// 🔴 WHAT F2 DOES WITH THE CASE DEFECT, WHICH BECAME A DATA LOSS
//
// F1 measured it, THREE runs out of three: with `Casse.txt` on the local
// machine, `casse.txt` AND `CASSE.TXT` return the CONTENT of `Casse.txt`, without
// error. On READ, it is a wrong file returned. ON WRITE, IT IS AN
// OVERWRITTEN FILE.
//
// ⚠️ AND THE MECHANISM IS WORSE ON THE BROWSER SIDE THAN ON THE VM SIDE:
//
//   - On the VM side, the case difference is ABSORBED by NTFS on an ALREADY
//     hydrated file: the bridge then receives the REAL case of the local entry, and the
//     push is right. That path is not dangerous.
//   - On the browser side, `getFileHandle(nom, { create: true })` runs on the
//     file system of the LOCAL MACHINE, case-insensitive on Windows and
//     on macOS by default. A push to `CASSE.TXT` there therefore opens
//     `Casse.txt` and OVERWRITES IT. It is THERE that the loss happens.
//   - The case reaching it: a file NEVER HYDRATED, created in the VM with
//     a case different from an existing local entry. NTFS has nothing to
//     resolve, the notification carries the VM's case, and the push overwrites
//     the local namesake.
//
// THE RULE, and it is PURE: before any write or creation, the writer
// enumerates the parent directory and looks for the requested name EXACTLY.
//
//   - exact name found                   → we write into it;
//   - no name, no namesake               → we create;
//   - a namesake up to case ALONE        → `casse-ambigue`, WE WRITE NOTHING.
//
// ⚠️ COST, WRITTEN DOWN: one enumeration of the parent directory PER WRITE. On a
// directory of a thousand entries, that is a thousand names walked — cheaper than the
// listing, which opens each file (`adaptateur.ts`), but not zero. Measurable
// in F4, not here. The alternative — the correspondence table fed by
// enumeration, which F1 bequeaths to F3 — would remove it; F2 does NOT build it,
// because a cache nothing invalidates is the old bridge's defect
// (`src/file.js`, cache WITHOUT TTL) and `Rafraichir` is a deliverable of F5.
//
// ⚠️ WHAT F2 DOES NOT DO: it does NOT fix case on READ. `casse.txt`
// will keep returning the content of `Casse.txt`. The guard only protects the
// WRITE direction, the only one where the error DESTROYS something.
//
// ⚠️ AND IT DOES NOT SEE UNICODE NORMALISATION. macOS stores its names in
// NFD, Windows in NFC: `résumé.txt` can exist there under two different sequences of code (policy: allow-fr, accented file name example)
// units, which `===` distinguishes and the user does not.
// The guard would then create a DUPLICATE instead of overwriting — less serious than the
// loss, but wrong. NOT HANDLED, declared; it is F3's canonicaliser.

import {
    FilesError,
    classer,
    type PoigneeBase,
    type FileHandle,
    type PoigneeRepertoire,
} from './adaptateur';

/* ── THE WRITABLE HANDLES ───────────────────────────────────────────
   One more STRUCTURAL SUBSET, described by what we use of it. The real
   `FileSystemDirectoryHandle` satisfies them without conversion — `canal.ts`
   checks it at compile time, exactly as for reading in F1. */

/**
 * The bytes a browser stream accepts.
 *
 * 🔴 **`Uint8Array<ArrayBuffer>` AND NOT A BARE `Uint8Array`, AND IT IS THE STRUCTURAL
 * CHECK OF `canal.ts` THAT REQUIRED IT.** `Uint8Array` alone means
 * `Uint8Array<ArrayBufferLike>`, hence **`SharedArrayBuffer` included** — and the
 * real `FileSystemWritableFileStream.write` only accepts a `BufferSource`,
 * that is, an `ArrayBufferView<ArrayBuffer>`. The real handle therefore did
 * **NOT** satisfy `RacineInscriptible`, and no one had seen it:
 * F2's "structural compatibility check" was an `as` towards a
 * subtype, which asserts instead of checking.
 *
 * ⚠️ **It is NOT a runtime incompatibility** — a `Uint8Array` backed
 * by an ordinary `ArrayBuffer` is a perfectly valid `BufferSource`. It was
 * the TYPE that lied, promising to accept views on shared memory
 * that this module never produces nor receives. Tightening it makes it
 * true.
 */
export type OctetsInscriptibles = Uint8Array<ArrayBuffer>;

/** The write stream returned by `createWritable()`. */
export interface FluxInscriptible {
    write(data: {
        type: 'write';
        position: number;
        data: OctetsInscriptibles;
    }): Promise<void>;
    /**
     * 🔵 THE COMMIT HAPPENS HERE, AND NOWHERE ELSE. `createWritable()`
     * writes into a swap file and only commits on `close()`: a
     * push interrupted mid-flight therefore leaves the local file UNCHANGED.
     *
     * ⚠️ That is excellent — no half-written file at the user's —
     * and it has a flip side: an interruption returns NOTHING, not even the beginning.
     * INFERENCE from the File System Access API specification, NOT MEASURED
     * here; criterion ⑤ of the acceptance run is written to test it.
     */
    close(): Promise<void>;
}

export interface WritableFileHandle extends FileHandle {
    createWritable(options?: { keepExistingData?: boolean }): Promise<FluxInscriptible>;
}

export interface RacineInscriptible extends PoigneeRepertoire {
    getDirectoryHandle(
        nom: string,
        options?: { create?: boolean },
    ): Promise<RacineInscriptible>;
    getFileHandle(
        nom: string,
        options?: { create?: boolean },
    ): Promise<WritableFileHandle>;
    values(): AsyncIterable<PoigneeBase>;
}

export interface Ecrivain {
    write(
        chemin: string,
        position: number,
        octets: Uint8Array,
        premier: boolean,
        last: boolean,
    ): Promise<void>;
    create(chemin: string, repertoire: boolean): Promise<void>;
    /** Closes any stream left open. Called when the channel closes. */
    abandonner(): void;
}

/** `"a/b/c"` → `["a","b","c"]`, `""` → `[]`. */
function composants(chemin: string): string[] {
    return chemin.split('/').filter((c) => c.length > 0);
}

export function createWriter(racine: RacineInscriptible): Ecrivain {
    /**
     * The open streams, ONE PER PATH.
     *
     * ⚠️ Opening a stream per CHUNK would make `keepExistingData` mandatory —
     * hence the old bridge's defect, which left its byte tail to a
     * file rewritten shorter (spec §12).
     */
    const flux = new Map<string, FluxInscriptible>();

    /** Walks down the first `jusqua` components, CREATING them if needed. */
    async function descendre(parts: string[], jusqua: number): Promise<RacineInscriptible> {
        let ici = racine;
        for (let i = 0; i < jusqua; i += 1) {
            try {
                ici = await ici.getDirectoryHandle(parts[i], { create: true });
            } catch (e) {
                throw classer(e, 'chemin-introuvable');
            }
        }
        return ici;
    }

    /**
     * 🔴 THE CASE GUARD. Returns the name to use, or THROWS.
     *
     * It returns the EXACT name when it exists, the requested name when nothing
     * resembles it, and throws `casse-ambigue` when a namesake only differs by
     * case. In that last case, WE WRITE NOTHING.
     */
    async function nomSur(parent: RacineInscriptible, nom: string): Promise<string> {
        const homonymes: string[] = [];
        try {
            for await (const enfant of parent.values()) {
                if (enfant.name === nom) return nom;
                if (enfant.name.toLowerCase() === nom.toLowerCase()) homonymes.push(enfant.name);
            }
        } catch (e) {
            throw classer(e, 'chemin-introuvable');
        }
        if (homonymes.length > 0) {
            // ⚠️ The message NAMES both, because that is all the
            // user will be able to do: rename one of the two. The CODE, for its part,
            // crosses the wire; the message stays in the browser console
            // and in the shell page.
            throw new FilesError(
                'casse-ambigue',
                `« ${nom} » differs from « ${homonymes.join(' », « ')} » only by case: ` +
                    `writing would overwrite the wrong file, nothing was written`,
            );
        }
        return nom;
    }

    async function ouvrir(chemin: string): Promise<FluxInscriptible> {
        const parts = composants(chemin);
        if (parts.length === 0) {
            throw new FilesError('introuvable', 'the root is not a file');
        }
        const parent = await descendre(parts, parts.length - 1);
        const nom = await nomSur(parent, parts[parts.length - 1]);
        try {
            const poignee = await parent.getFileHandle(nom, { create: true });
            // 🔴 WITHOUT `keepExistingData`, AND IT IS THE OLD BRIDGE'S DEFECT
            // WE REFUSE TO REPLAY: it used `keepExistingData: true`
            // without `truncate`, so that A FILE REWRITTEN SHORTER
            // KEPT ITS BYTE TAIL (spec §12). The local file would
            // then have a content the VM never had.
            return await poignee.createWritable();
        } catch (e) {
            throw classer(e, 'introuvable');
        }
    }

    return {
        async write(chemin, position, octets, premier, last) {
            if (premier) {
                // ⚠️ A `premier` on an ALREADY open path can only come
                // from a replay whose previous stream was never closed — an
                // interrupted push, then restarted. We close the old one rather
                // than leave TWO open on the same file: the
                // second `close()` would win, and the first would leave its
                // swap file behind.
                const ancien = flux.get(chemin);
                if (ancien !== undefined) {
                    flux.delete(chemin);
                    await ancien.close().catch(() => {});
                }
                flux.set(chemin, await ouvrir(chemin));
            }
            const ouvert = flux.get(chemin);
            if (ouvert === undefined) {
                // ⚠️ A chunk that is NOT the first on a path without a stream
                // : the bridge and the browser have diverged. Opening here would write
                // a file truncated to this very chunk, which is WORSE than
                // refusing — the truncation would be silent.
                throw new FilesError(
                    'interne',
                    `non-initial chunk on « ${chemin} » without an open stream`,
                );
            }
            try {
                // ⚠️ **THE TYPE TIGHTENING HAPPENS HERE, AND ONLY ONCE.**
                // `proto/ts/fichiers` returns a BARE `Uint8Array` — hence (policy: allow-fr, proto module path)
                // `Uint8Array<ArrayBufferLike>`, `SharedArrayBuffer` included —
                // because that is what the frame decoder produces. Nothing, at
                // runtime, can give it a view on shared
                // memory: the frame comes from an `ArrayBuffer` of
                // `RTCDataChannel`. **The copy is therefore free in practice and
                // honest in type**: it says what the module really
                // receives, rather than asserting it.
                //
                // 🔵 It is the structural check of `canal.ts` that required this
                // tightening — see [`OctetsInscriptibles`].
                const data: OctetsInscriptibles = new Uint8Array(octets);
                await ouvert.write({ type: 'write', position, data: data });
            } catch (e) {
                // The stream is lost: remove it, otherwise the next chunk
                // would write into a dead stream and the failure would change cause.
                flux.delete(chemin);
                throw classer(e, 'introuvable');
            }
            if (last) {
                flux.delete(chemin);
                try {
                    // 🔵 LA COMMITTAISON.
                    await ouvert.close();
                } catch (e) {
                    throw classer(e, 'introuvable');
                }
            }
        },

        async create(chemin, repertoire) {
            const parts = composants(chemin);
            if (parts.length === 0) {
                throw new FilesError('deja-present', 'the root already exists');
            }
            const parent = await descendre(parts, parts.length - 1);
            const nom = await nomSur(parent, parts[parts.length - 1]);
            try {
                if (repertoire) {
                    await parent.getDirectoryHandle(nom, { create: true });
                } else {
                    // ⚠️ CREATE, AND NOTHING MORE: no `createWritable()` here.
                    // Opening one would TRUNCATE an existing local file, whereas
                    // a creation has no effect on what is already there.
                    await parent.getFileHandle(nom, { create: true });
                }
            } catch (e) {
                throw classer(e, 'introuvable');
            }
        },

        abandonner() {
            // ⚠️ `close()` AND NOT `abort()`: a stream abandoned without being closed
            // leaves its swap file behind. Nothing is awaited —
            // this function is called from the channel's closing, which is
            // synchronous.
            for (const [, ouvert] of flux) void ouvert.close().catch(() => {});
            flux.clear();
        },
    };
}
