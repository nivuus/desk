// RENAMING AND DELETION, on the local machine side. **PURE**: neither DOM, nor
// WebRTC, nor binary frame; the root is INJECTED into it, as in
// `adaptateur.ts` and `ecriture.ts`.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔵 THE COST FIGURE OF SPEC §3.5.1 IS WRONG FOR THIS SETUP
// ════════════════════════════════════════════════════════════════════════════
//
// The spec writes: "the fallback […] makes THE WHOLE CONTENT OF THE FILE TRAVEL TWICE
// over the channel. Renaming a 1 GiB file on the fallback path therefore
// costs 2 GiB of channel".
//
// **That is only true if the BRIDGE orchestrates the copy**, through a sequence of
// `Lire` and `Write`. F3 does not orchestrate it: renaming is **ONE SINGLE
// MESSAGE** (`Renommer { de, vers }`), and the fallback copy happens between two
// handles that both live in the browser, on the local machine's
// disk. **Cost of the fallback on the channel: ZERO bytes, in both branches.**
//
// WHAT THE FALLBACK COSTS ANYWAY, and which must not be erased by the
// figure above:
//
//   - **it is not atomic** — a cut in the middle leaves two copies,
//     one of which carries the target name and is partial. The spec says so; it is
//     still true, and **it is not repairable here**;
//   - it **transiently doubles the disk usage** of the local machine;
//   - it is **O(size)** in time and, for a directory, **O(number
//     of entries)** FSA calls — on a deep directory, that can be long,
//     and **NOTHING HERE BOUNDS IT**;
//   - ✅ **F4 MEASURED IT (August 21st, 2026), AND THE COST IS NIL AT THESE SIZES.**
//     The fallback ran for the FIRST time — F3 had delivered it without any
//     of its lines running —, forced by an injection that removes `move`.
//     64 KiB: 173 / 193 ms; 1 MiB: 125 / 126 ms, two runs. The CONTROL
//     arm, without neutralising `move`, gives 159 / 162 and 126 / 126 ms:
//     **indistinguishable**. It is indeed a LOCAL time — the product's trace
//     says so, "zero bytes on the channel" — and the margin to `DELAI_MUTATION` (15 s)
//     is two orders of magnitude.
//     ⚠️ **The DIRECTORY half stays out of the product's reach**: ProjFS
//     refuses the renaming of a directory before consulting the provider.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 `move()` OVERWRITES, AND THAT IS WHY THE CHECK COMES FIRST
// ════════════════════════════════════════════════════════════════════════════
//
// `FileSystemHandle.move()` **does not belong to the standard** of the File System
// Access API: it is a Chromium extension. Spec §3.5.1 says so, and the old
// bridge uses it (`web/index.js:628`, `:644`).
//
// 🔴 **IT SILENTLY OVERWRITES AN EXISTING DESTINATION, AND IT IS MEASURED**
// — probe S2, two identical runs on Chrome 151:
// a file holding its original content is overwritten by
// `agresseur.move(racine, 'S2-Victime.txt')`, which reports `issue: "ok"` with the mover's content,
// **without error**. *No document of the repository said so before this one.*
// Resolving the destination therefore comes FIRST, in both branches —
// otherwise renaming `draft.txt` to `note.txt` would destroy `note.txt` without a
// word. Log: `journaux-pont-fichiers-f3/s2-move-casse.txt`. (policy: allow-fr, archived log path)
//
// 🔴 **AND IT DOES NOT EXIST ON A DIRECTORY** — same probe,
// `move_repertoire: { present: false }`. F3's plan held the fact that
// the old bridge only called it on files (`web/index.js:628`) as a
// "**hint, not proof**"; **the measurement settles it**.
//
// ⚠️ **CONSEQUENCE, AND IT REVERSES THE PLAN'S VOCABULARY**: for a
// DIRECTORY, the copy is not a "fallback" — **it is THE path, the only one.**
// Renaming a directory containing a subdirectory, which the defect of
// `web/index.js:631` made ALWAYS impossible, only works that way.
//
// ⚠️ **`move()` IS DETECTED AT CALL TIME, never captured when the module
// loads.** A detection made once and for all would be wrong the day
// another file system got injected — and that is exactly what
// this module's tests do, using TWO fakes: one exposing it,
// the other not.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 THE SPECIAL CASE THAT DESTROYS: `a.txt` → `A.txt`
// ════════════════════════════════════════════════════════════════════════════
//
// Neither the spec nor the old bridge handles it. On a local machine INSENSITIVE to
// case, the destination "already exists" — **and it is the source itself**.
// A naive implementation refuses (`deja-present`) or, worse, overwrites.
//
// **F3's rule**: if the destination's only namesake IS the source, it is
// a **pure case rename**, it is legal, and the fallback goes through an **intermediate
// name** — two moves, never an overwrite.
//
// ⚠️ **THIS PATH CANNOT BE EXERCISED BY THE ACCEPTANCE INSTRUMENT, and probe
// S2 measures it**: OPFS is **case SENSITIVE**
// (`opfs_sensible_a_la_casse: true`), so `S2-Pure.txt` → `S2-PURE.TXT`
// succeeds there DIRECTLY, without any collision to resolve. The intermediate name
// is only tested on the host, by the case-INSENSITIVE fake of `mutation.test.ts`.
// **Do not read an "ok" from the probe as a validation of this path.**
//
// ════════════════════════════════════════════════════════════════════════════
// ⚠️ DELETION IS NOT RECURSIVE — DIVERGENCE FROM SPEC §3.5
// ════════════════════════════════════════════════════════════════════════════
//
// It writes `dir.removeEntry(nom, { recursive })`. **F3 calls
// `removeEntry(nom)` WITHOUT `recursive`.**
//
// Reason: `recursive: true` turns ONE gesture in the VM into a **recursive
// destruction** on the local machine's disk, on the strength of a mirror no
// proof says is up to date. Windows, for its part, never deletes a non-empty directory
// in one gesture: Explorer and `rd /s` erase the children one by one, and
// **each child produces its own notification**. The non-recursive mirror therefore
// follows Windows step by step.
//
// 🔵 **Secondary benefit, and it is not decorative**: if the browser answers
// that the directory is not empty, it means **the mirror has drifted** —
// and `repertoire-non-vide` becomes a REAL and DIAGNOSTIC cause instead of
// a code never produced.
//
// ⚠️ **What this assumes, and which IS NOT MEASURED**: that ProjFS does emit
// a deletion notification PER CHILD, including for children never
// enumerated nor hydrated. It is question ③ of probe S1. If the answer is
// no, deleting a non-empty directory will leave the children on the
// local machine — **degrades, does not block**.

import { FilesError, classer, type PoigneeBase, type FileHandle } from './adaptateur';
import type { FluxInscriptible, RacineInscriptible } from './ecriture';
import { canoniser, canoniserOuLever } from './noms';
import { copyFile, copierRepertoire, ouvrirRepertoire, retirerArbre } from './copie';

/** What we can do with a file handle we want to move. */
export interface MutableFileHandle extends FileHandle {
    createWritable(options?: { keepExistingData?: boolean }): Promise<FluxInscriptible>;
    /** **NON STANDARD** — extension Chromium. Absente ⇒ le repli local. */
    move?(parent: RacineMutable, nom: string): Promise<void>;
}

/**
 * A root one can mutate.
 *
 * 🔵 **`move?` IS OPTIONAL IN THE TYPE, and that is what lets us write
 * TWO fakes — one exposing it, the other not — and see both branches
 * green on the host.** A type imposing it would make the fallback
 * **unreachable by a test**.
 */
export interface RacineMutable extends RacineInscriptible {
    getDirectoryHandle(nom: string, options?: { create?: boolean }): Promise<RacineMutable>;
    getFileHandle(nom: string, options?: { create?: boolean }): Promise<MutableFileHandle>;
    /** ⚠️ **WITHOUT `recursive`** — see the header. */
    removeEntry(nom: string): Promise<void>;
    /** **NON STANDARD**. */
    move?(parent: RacineMutable, nom: string): Promise<void>;
}

/** What a rename cost LOCALLY — the instrumentation the spec requires. */
export interface TraceRenommage {
    /** `true` if `move()` served, `false` if the fallback copied. */
    parMove: boolean;
    /** Bytes copied. **Zero on the `move` branch.** */
    octets: number;
    /** Entries recreated. **Zero on the `move` branch**, 1 for a file. */
    entrees: number;
}

/** `"a/b/c"` → `["a","b","c"]`, `""` → `[]`. */
function composants(chemin: string): string[] {
    return chemin.split('/').filter((c) => c.length > 0);
}

/**
 * Walks down the first `jusqua` components **while canonicalising them**, without
 * creating any.
 */
async function descendre(
    racine: RacineMutable,
    parts: string[],
    jusqua: number,
): Promise<RacineMutable> {
    let ici = racine;
    for (let i = 0; i < jusqua; i += 1) {
        const nom = await canoniserOuLever(ici, parts[i], 'chemin-introuvable');
        try {
            ici = await ici.getDirectoryHandle(nom);
        } catch (e) {
            throw classer(e, 'chemin-introuvable');
        }
    }
    return ici;
}

/** Walks down CREATING the missing directories — for the destination. */
async function descendreEnCreant(
    racine: RacineMutable,
    parts: string[],
    jusqua: number,
): Promise<RacineMutable> {
    let ici = racine;
    for (let i = 0; i < jusqua; i += 1) {
        // ⚠️ We canonicalise FIRST: without that, `archives/` and `Archives/`
        // would become two directories on a case-SENSITIVE machine, and
        // the same one on an insensitive machine — two behaviours for one path.
        const r = await canoniser(ici, parts[i]);
        const nom = r.sorte === 'trouve' ? r.nom : parts[i];
        if (r.sorte === 'ambigu') {
            throw new FilesError(
                'casse-ambigue',
                `« ${parts[i]} » cannot be told apart from « ${r.noms.join(' », « ')} »`,
            );
        }
        try {
            ici = await ici.getDirectoryHandle(nom, { create: true });
        } catch (e) {
            throw classer(e, 'chemin-introuvable');
        }
    }
    return ici;
}

/**
 * Renames `de` to `vers`, both relative to the root.
 *
 * ⚠️ **`repertoire` is CARRIED from the ProjFS callback**, never
 * rediscovered: the browser would ask for it again at the cost of a round trip, and would
 * be wrong about an entry the rename has just made disappear.
 */
export async function renommer(
    racine: RacineMutable,
    de: string,
    vers: string,
    repertoire: boolean,
): Promise<TraceRenommage> {
    const partsDe = composants(de);
    const partsVers = composants(vers);
    if (partsDe.length === 0 || partsVers.length === 0) {
        throw new FilesError('non-supporte', 'the root cannot be renamed');
    }
    const parentSource = await descendre(racine, partsDe, partsDe.length - 1);
    const nomSource = await canoniserOuLever(
        parentSource,
        partsDe[partsDe.length - 1],
        'introuvable',
    );
    const parentDest = await descendreEnCreant(racine, partsVers, partsVers.length - 1);
    const nomDemande = partsVers[partsVers.length - 1];

    // ── RESOLVING THE DESTINATION, AND IT PRECEDES EVERYTHING ────────────────
    // 🔴 `move()` OVERWRITES: without this block, renaming `draft.txt` to `note.txt`
    // would destroy `note.txt` without a word.
    const dest = await canoniser(parentDest, nomDemande);
    const memeParent = parentSource === parentDest;
    let cassePure = false;
    if (dest.sorte === 'ambigu') {
        throw new FilesError(
            'casse-ambigue',
            `« ${nomDemande} » cannot be told apart from « ${dest.noms.join(' », « ')} »`,
        );
    }
    if (dest.sorte === 'trouve') {
        // 🔴 **THE PURE CASE RENAME.** If the only namesake of the
        // destination IS the source, it is not a collision: it is
        // `a.txt` → `A.txt`, and it is legal.
        if (memeParent && dest.nom === nomSource) {
            cassePure = true;
        } else {
            throw new FilesError(
                'deja-present',
                `« ${vers} » already exists under the name « ${dest.nom} »`,
            );
        }
    }

    if (cassePure) {
        // Two moves, NEVER an overwrite: on a case-insensitive
        // machine, moving onto oneself is either refused, or — worse —
        // a truncation.
        const intermediaire = nomIntermediaire(nomSource);
        await deplacer(parentSource, nomSource, parentSource, intermediaire, repertoire);
        await deplacer(parentSource, intermediaire, parentDest, nomDemande, repertoire);
        return { parMove: true, octets: 0, entrees: 0 };
    }
    return deplacer(parentSource, nomSource, parentDest, nomDemande, repertoire);
}

/**
 * An intermediate name that cannot collide with anything.
 *
 * ⚠️ **It carries a random component**, and not a fixed suffix: two
 * concurrent pure case renames in the same directory would
 * step on each other, and the second would destroy the first one's file.
 */
function nomIntermediaire(source: string): string {
    const jeton = Math.random().toString(36).slice(2, 10);
    return `${source}.pont-${jeton}.tmp`;
}

/** `move()` if it exists, the local copy otherwise. */
async function deplacer(
    parentSource: RacineMutable,
    nomSource: string,
    parentDest: RacineMutable,
    nomDest: string,
    repertoire: boolean,
): Promise<TraceRenommage> {
    // ⚠️ **DETECTED AT CALL TIME**, on the handle actually obtained.
    const poignee: PoigneeBase & { move?: unknown } = repertoire
        ? await ouvrirRepertoire(parentSource, nomSource)
        : await openFile(parentSource, nomSource);
    if (typeof poignee.move === 'function') {
        try {
            await (poignee as { move(p: RacineMutable, n: string): Promise<void> }).move(
                parentDest,
                nomDest,
            );
            return { parMove: true, octets: 0, entrees: 0 };
        } catch (e) {
            throw classer(e, 'introuvable');
        }
    }
    // ── THE FALLBACK, ENTIRELY IN THE BROWSER ─────────────────────────────
    const trace = { parMove: false, octets: 0, entrees: 0 };
    if (repertoire) {
        await copierRepertoire(parentSource, nomSource, parentDest, nomDest, trace);
    } else {
        await copyFile(parentSource, nomSource, parentDest, nomDest, trace);
    }
    // 🔴 **THE SOURCE IS ONLY REMOVED AFTERWARDS**, and an interrupted copy
    // therefore leaves it INTACT. The reverse would lose the file on a cut.
    try {
        if (repertoire) {
            await retirerArbre(parentSource, nomSource);
        } else {
            await parentSource.removeEntry(nomSource);
        }
    } catch (e) {
        throw classer(e, 'introuvable');
    }
    return trace;
}

async function openFile(
    parent: RacineMutable,
    nom: string,
): Promise<MutableFileHandle> {
    try {
        return await parent.getFileHandle(nom);
    } catch (e) {
        throw classer(e, 'introuvable');
    }
}

/**
 * Deletes `chemin`, relative to the root.
 *
 * ⚠️ **`removeEntry(nom)` WITHOUT `recursive`** — see the header.
 */
export async function remove(
    racine: RacineMutable,
    chemin: string,
    _repertoire: boolean,
): Promise<void> {
    const parts = composants(chemin);
    if (parts.length === 0) {
        throw new FilesError('non-supporte', 'the root cannot be removed');
    }
    const parent = await descendre(racine, parts, parts.length - 1);
    const nom = await canoniserOuLever(parent, parts[parts.length - 1], 'introuvable');
    try {
        await parent.removeEntry(nom);
    } catch (e) {
        // 🔴 **`InvalidModificationError` MEANS TWO THINGS DEPENDING ON THE VERB,
        // AND `adaptateur.classer` CANNOT TELL THEM APART.**
        //
        // On a CREATION, it means "an entry of the same name exists" —
        // and `classer` translates it into `deja-present`, which F2 wrote. On a
        // `removeEntry` WITHOUT `recursive`, it means **"the directory
        // is not empty"**, which is an entirely different diagnosis: the
        // mirror has drifted.
        //
        // The classification is therefore done HERE, where the verb is known.
        // Widening it in `classer` would make a creation return
        // `repertoire-non-vide`, or the reverse.
        // ✅ **THIS EXCEPTION NAME IS MEASURED, not assumed**: probe S2 returns
        // `remove_non_vide: "REFUSE:InvalidModificationError"` on a
        // `removeEntry` WITHOUT `recursive` of a non-empty directory, two
        // identical runs. `repertoire-non-vide` is therefore indeed
        // reachable — it is not a code written for the table.
        if (e instanceof DOMException && e.name === 'InvalidModificationError') {
            throw new FilesError(
                'repertoire-non-vide',
                `« ${chemin} » is not empty on the local host: the mirror has drifted, ` +
                    `nothing was removed`,
            );
        }
        throw classer(e, 'introuvable');
    }
}
