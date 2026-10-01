// The ORDER in which the browser answers the bridge.
//
// 🔴 WHY IT EXISTS. `canal.ts` launched `traiter` on each frame as it came,
// and sent each answer AS SOON AS IT WAS READY. Two chunks of one read could
// therefore come back swapped — the LAST chunk is shorter, an enumeration
// takes longer here than there — and the agent's read window
// (`agent/src/pont/lecture.rs`, `HorsOrdre`) refuses, rightly, a chunk that
// is not the one it expects: writing a range at the wrong place would make a
// file only a SHA-256 could show to be wrong. The read then FAILED.
//
// The work still runs CONCURRENTLY — reading four chunks at once is the very
// point of the window —; only the SENDING is put back in the order of the
// requests.
//
// ⚠️ **THE ORDER IS KEPT PER PATH, NOT ACROSS THE WHOLE CHANNEL.** The
// invariant the agent checks binds the requests of ONE operation, all on one
// path. A global order would make a chunk wait behind a listing of another
// directory — F4 measured ~3 s for a thousand entries, against a
// `DELAI_LIRE` of 5 s: head-of-line blocking would have turned a slow listing
// into expired reads elsewhere.

import { decoder } from '../../../proto/ts/fichiers';

/**
 * The key under which `octets` is ordered: the path its header names, or the
 * empty string — one shared queue — when it names none or cannot be read.
 */
export function cleDOrdre(octets: ArrayBuffer): string {
    try {
        const entete: unknown = decoder(octets).entete;
        if (typeof entete !== 'object' || entete === null) return '';
        const o = entete as { chemin?: unknown; de?: unknown };
        if (typeof o.chemin === 'string') return o.chemin;
        // A rename names `de` and `vers`: it is ordered with what read `de`.
        if (typeof o.de === 'string') return o.de;
    } catch {
        // Unreadable: `traiter` will say so; ordering it is all we need here.
    }
    return '';
}

export interface Sequenceur {
    /**
     * Runs `emettre(await travail)` once every `emettre` enchained BEFORE it
     * under the same `cle` has returned. `travail` must not reject — the
     * caller turns its failure into a value, so that a failure keeps its
     * place in the order too.
     */
    enchainer<T>(cle: string, travail: Promise<T>, emettre: (r: T) => unknown): Promise<void>;
    /** How many keys still have an answer pending. */
    enAttente(): number;
}

export function creerSequenceur(): Sequenceur {
    const queues = new Map<string, Promise<void>>();
    return {
        enchainer(cle, travail, emettre) {
            const precedent = queues.get(cle) ?? Promise.resolve();
            const suivant = precedent
                .then(() => travail)
                .then(emettre)
                .then(() => {});
            // The queue never rejects: a throwing `emettre` must not block
            // every answer after it on the same path.
            const queue = suivant.catch(() => {});
            queues.set(cle, queue);
            void queue.then(() => {
                if (queues.get(cle) === queue) queues.delete(cle);
            });
            return suivant;
        },
        enAttente() {
            return queues.size;
        },
    };
}
