// TREE COPY AND ITS REMOVAL — the branch of renaming that has no
// `move()`. **PURE**: neither DOM, nor WebRTC, nor frame; the handles are
// injected, as everywhere in `fichiers/`.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 FOR A DIRECTORY, IT IS NOT A "FALLBACK" — IT IS THE ONLY PATH
// ════════════════════════════════════════════════════════════════════════════
//
// F3's probe S2 measures it, two identical runs on Chrome 151:
// **`move()` DOES NOT EXIST on a directory** (`move_repertoire:
// { present: false }`). F3's plan held the fact that the old bridge only
// called it on files (`web/index.js:628`) as a "hint, not
// proof"; the measurement settles it. Log:
// `docs/superpowers/plans/journaux-pont-fichiers-f3/s2-move-casse.txt`.
//
// ════════════════════════════════════════════════════════════════════════════
// WHY THIS EXTRACTION, AND WHY IT COMES BEFORE THE CROSSING
// ════════════════════════════════════════════════════════════════════════════
//
// `mutation.ts` was at **497** lines for a gate at 500 — margin **3**. The
// repository crossed that ceiling four sub-blocks in a row without seeing it go by, and
// each time it was the survey of a neighbouring workstream that named it first.
// **Here the extraction precedes the addition**, which is the gesture D9 invented
// and D10 played three times.
//
// The dividing line is a responsibility: `mutation.ts` decides **WHAT**
// to do — resolve, refuse, choose the branch —, this module does **HOW**
// a tree is copied over and removed.

import { classer, type PoigneeBase } from './adaptateur';
import type { RacineMutable, TraceRenommage } from './mutation';

/** Opens an existing directory, or classifies the failure. */
export async function ouvrirRepertoire(
    parent: RacineMutable,
    nom: string,
): Promise<RacineMutable> {
    try {
        return await parent.getDirectoryHandle(nom);
    } catch (e) {
        throw classer(e, 'introuvable');
    }
}

export async function copierFichier(
    parentSource: RacineMutable,
    nomSource: string,
    parentDest: RacineMutable,
    nomDest: string,
    trace: TraceRenommage,
): Promise<void> {
    try {
        const fichier = await (await parentSource.getFileHandle(nomSource)).getFile();
        const cible = await parentDest.getFileHandle(nomDest, { create: true });
        // ⚠️ **WITHOUT `keepExistingData`** — the destination is new or empty by
        // right, and the old bridge's defect was precisely to keep the
        // byte tail of a file rewritten shorter (spec §12).
        const flux = await cible.createWritable();
        const octets = new Uint8Array(await fichier.slice(0, fichier.size).arrayBuffer());
        await flux.write({ type: 'write', position: 0, data: octets });
        // 🔵 THE COMMIT HAPPENS HERE, AND NOWHERE ELSE.
        await flux.close();
        trace.octets += octets.length;
        trace.entrees += 1;
    } catch (e) {
        throw classer(e, 'introuvable');
    }
}

/**
 * Recreates the tree, leaf by leaf.
 *
 * 🔴 **THE OLD BRIDGE'S DEFECT WE REFUSE TO REPLAY**:
 * `web/index.js:631` writes `const newDir = await newDir.getDirectoryHandle(...)`
 * **inside the block where `newDir` is the parameter** — a temporal dead
 * zone, hence a `ReferenceError`. **Renaming a directory
 * containing a subdirectory therefore ALWAYS fails there.**
 *
 * ⛔ **THIS PATH IS NEVER TAKEN, AND IT IS F3'S ACCEPTANCE RUN THAT
 * ESTABLISHED IT.** An earlier wording said "it is F3's criterion (1),
 * written to exercise exactly this case": **the measurement refutes it**. ProjFS
 * REFUSES the renaming of a directory **before consulting the provider** —
 * Windows' "This request is not supported" error, and **no** `code=32`
 * notification in the log, two acceptance runs plus probe S1. The
 * criterion (1) b is therefore not deliverable, and **no line of this function
 * has ever run in real conditions**.
 *
 * ⚠️ It stays written and tested on the host: the old bridge's defect is
 * real, and the day a path reaches it — a copy/delete driven
 * from the VM — this is where to look.
 */
export async function copierRepertoire(
    parentSource: RacineMutable,
    nomSource: string,
    parentDest: RacineMutable,
    nomDest: string,
    trace: TraceRenommage,
): Promise<void> {
    const source = await ouvrirRepertoire(parentSource, nomSource);
    let cible: RacineMutable;
    try {
        cible = await parentDest.getDirectoryHandle(nomDest, { create: true });
    } catch (e) {
        throw classer(e, 'introuvable');
    }
    trace.entrees += 1;
    // ⚠️ The enumeration is MATERIALISED before mutating: iterating a directory
    // being modified during the iteration has no defined semantics.
    const enfants: PoigneeBase[] = [];
    try {
        for await (const enfant of source.values()) enfants.push(enfant);
    } catch (e) {
        throw classer(e, 'introuvable');
    }
    for (const enfant of enfants) {
        if (enfant.kind === 'directory') {
            await copierRepertoire(source, enfant.name, cible, enfant.name, trace);
        } else {
            await copierFichier(source, enfant.name, cible, enfant.name, trace);
        }
    }
}

/**
 * Removes a directory and everything it carries, **leaf by leaf**.
 *
 * 🔴 **IT IS NOT `recursive: true`, AND THE DIFFERENCE IS THE WHOLE POINT.**
 * `removeEntry(nom)` without `recursive` refuses a non-empty directory (the test
 * fake refuses it like the real browser), and the rename fallback must
 * nonetheless remove the source tree it has just copied over. Two ways:
 *
 *   - `recursive: true` — **REFUSED**: it would destroy on the strength of a mirror
 *     no proof says is up to date, and that is the whole argument of the header;
 *   - walk down ourselves and remove **what we have just copied**, entry by
 *     entry, bottom up. **That is what is done.**
 *
 * 🔵 **The second is SAFER than the first, not merely more verbose**:
 * we only remove what [`copierRepertoire`] enumerated and copied a few
 * lines earlier. An entry that appeared meanwhile on the local machine **makes
 * the removal fail** instead of being carried off silently — and `deplacer`
 * then propagates the failure, source intact.
 *
 * ⚠️ **[`supprimer`], for its part, NEVER CALLS THIS FUNCTION.** A deletion
 * requested by the VM only removes ONE entry: Windows sends one notification
 * PER CHILD, and the mirror follows it step by step. The two paths are neighbours and
 * must not be unified.
 */
export async function retirerArbre(parent: RacineMutable, nom: string): Promise<void> {
    const dossier = await parent.getDirectoryHandle(nom);
    const enfants: PoigneeBase[] = [];
    for await (const enfant of dossier.values()) enfants.push(enfant);
    for (const enfant of enfants) {
        if (enfant.kind === 'directory') {
            await retirerArbre(dossier, enfant.name);
        } else {
            await dossier.removeEntry(enfant.name);
        }
    }
    // The directory is empty NOW: `removeEntry` without `recursive`
    // accepts it. If it is not, an entry appeared between
    // the enumeration and here — and refusal is the right behaviour.
    await parent.removeEntry(nom);
}
