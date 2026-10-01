// The SHORT memory of resolved handles, for READING ONLY.
//
// 🔴 WHY IT EXISTS. `noms.ts` canonicalises each path component by
// ENUMERATING its parent, and `adaptateur.lire` used to pay that for EVERY
// chunk: a 100 MiB file is 1,600 chunks of 64 KiB, hence 1,600 enumerations
// of each directory on its path — a directory of a thousand entries made a
// copy pay a million `values()` steps to move bytes the enumeration never
// carried.
//
// ⚠️ IT IS NOT THE CACHE THE HEADER OF `adaptateur.ts` REFUSES. The old
// bridge's defect was a cache WITHOUT TTL that nothing invalidated. This one:
//
//   1. FORGETS AFTER `TTL_MS`, counted from the RESOLUTION, never extended by
//      a hit — a file read without pause is re-resolved every `TTL_MS`, so a
//      case change made on the local machine is seen at worst `TTL_MS` late;
//   2. IS EMPTIED by every gesture that may change the tree: a write that
//      opens a file, a creation, a rename, a removal, a listing
//      (`protocole.ts` calls `oublier()`);
//   3. IS EMPTIED by a failure: a handle that no longer opens is never
//      retried as is — `adaptateur.lire` forgets everything and resolves again.
//
// And it is BOUNDED (`MAX_ENTREES`), oldest first.

/** How long a resolution stays valid, from the moment it was made. */
export const TTL_MS = 5_000;

/** How many resolutions are kept at most. */
export const MAX_ENTREES = 256;

export interface Memo<T> {
    obtenir(cle: string): T | undefined;
    poser(cle: string, valeur: T): void;
    oublier(): void;
}

export function creerMemo<T>(maintenant: () => number = Date.now): Memo<T> {
    const entrees = new Map<string, { valeur: T; expire: number }>();
    return {
        obtenir(cle) {
            const e = entrees.get(cle);
            if (e === undefined) return undefined;
            if (maintenant() >= e.expire) {
                entrees.delete(cle);
                return undefined;
            }
            return e.valeur;
        },
        poser(cle, valeur) {
            entrees.delete(cle);
            entrees.set(cle, { valeur, expire: maintenant() + TTL_MS });
            // A `Map` iterates in insertion order: the first key is the oldest.
            while (entrees.size > MAX_ENTREES) {
                const plusVieille = entrees.keys().next().value as string;
                entrees.delete(plusVieille);
            }
        },
        oublier() {
            entrees.clear();
        },
    };
}
