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
//      (`protocole.ts` calls `clear()`);
//   3. IS EMPTIED by a failure: a handle that no longer opens is never
//      retried as is — `adaptateur.lire` forgets everything and resolves again.
//
// And it is BOUNDED (`MAX_ENTREES`), oldest first.

/** How long a resolution stays valid, from the moment it was made. */
export const TTL_MS = 5_000;

/** How many resolutions are kept at most. */
export const MAX_ENTREES = 256;

export interface Memo<T> {
    get(key: string): T | undefined;
    set(key: string, value: T): void;
    clear(): void;
}

export function createMemo<T>(now: () => number = Date.now): Memo<T> {
    const entries = new Map<string, { value: T; expire: number }>();
    return {
        get(key) {
            const e = entries.get(key);
            if (e === undefined) return undefined;
            if (now() >= e.expire) {
                entries.delete(key);
                return undefined;
            }
            return e.value;
        },
        set(key, value) {
            entries.delete(key);
            entries.set(key, { value, expire: now() + TTL_MS });
            // A `Map` iterates in insertion order: the first key is the oldest.
            while (entries.size > MAX_ENTREES) {
                const oldest = entries.keys().next().value as string;
                entries.delete(oldest);
            }
        },
        clear() {
            entries.clear();
        },
    };
}
