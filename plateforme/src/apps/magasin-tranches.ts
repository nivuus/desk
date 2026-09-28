// The store of the CHUNKS of an upload: bytes on DISK, **one
// file per chunk**, under one directory per upload —
// `<root>/<upload-id>/<n>`.
//
// 🔴 CHUNKS ARE NEVER ASSEMBLED: THERE IS NO REBUILT
// FILE, neither at sealing nor anywhere else. Three reasons, in order of
// their weight:
//
//   1. THE DISK WOULD DOUBLE. An 800 MB installer would hold 1.6 GB for the time
//      of the assembly, and a queue of simultaneous deposits would make that doubling
//      the NORM rather than the peak. The service would be full for a copy
//      nobody needs — the agent reads a stream, it does not look for a
//      file.
//   2. AN ASSEMBLED FILE WOULD BE A SECOND SOURCE OF TRUTH. The day it
//      diverged from its chunks — interrupted write, chunk rewritten
//      afterwards — nothing here could tell which of the two to believe, and the
//      `sha256` of the contract would only blame the last link. The store has
//      only one state, and that is what is on the disk.
//   3. SEALING MUST STAY A DECISION, NOT A COPY. Assembling would make it
//      an O(size) operation, which can fail halfway and
//      leave a half file; sealing is writing a date.
//
// 🔴 WHY THE DISK AND NOT THE DATABASE: the whole reasoning lives at the top of
// `icones.ts` and is not copied here — a blob does not cross the double
// pass without lying, `SERIAL` in reverse. The two directories are siblings, and
// so are their two environment variables.
//
// ⚠️ **THIS STORE JUDGES NOTHING.** It does not say whether a split is complete,
// nor whether a chunk has the right size. That rule is PURE and SHARED with
// the browser (`proto/ts/tranches.ts`): two independent arithmetics
// would diverge one day, and the symptom would be a sealing that refuses without
// anyone knowing which of the two ends is wrong. Here we write, list, read back.

import { createReadStream, createWriteStream, mkdirSync, openSync, readdirSync, rmSync, renameSync, statSync } from 'node:fs';
import { readdir, rm, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import type { Tranche } from '../../../proto/ts/tranches';

/// Same remedy, same reason, as `icones.ts::PAS_DE_REPRISE` — MEASURED by the
/// review (correction round 2), on a bench of 3,200 uploads: 272 ms
/// in one block, ZERO 10 ms beat served during it, port open.
/// ⚠️ NOT CALIBRATED, same reasoning as on the icon side.
const PAS_DE_REPRISE = 50;

async function rendreLaMain(): Promise<void> {
    await new Promise<void>((resolve) => setImmediate(resolve));
}

/// ⚠️ **NOT CALIBRATED.** No constant of this repository is.
///
/// 🔴 THE REFERENCE FLOOR (see `evincer`, below) IS WHAT TELLS
/// AN EVICTION FROM A CORRUPTION: evicting an upload still named by
/// an ongoing installation would make its chunks vanish without anything
/// saying so — the resumption would request again bytes a user believes
/// they already sent.
///
/// ⚠️ WHAT THIS RULE DOES NOT DO: it does NOT bound the disk. An
/// upload directory that keeps growing keeps growing. The
/// size cap was DISMISSED by decision, because it can evict an
/// object still referenced — that is, trade a visible growth
/// for a silent failure.
export const AGE_EVICTION_TRANCHES_MS = 30 * 24 * 60 * 60_000;

/// 🔴 THE IDENTIFIER OF AN UPLOAD BECOMES A DIRECTORY NAME, AND IT
/// COMES FROM THE NETWORK. `/televersement/..%2f..%2fetc/tranche/0` must be refused,
/// never sanitised: sanitising silently would write somewhere, and
/// nobody would know where.
///
/// The required shape is the one `depot/televersement.ts::create` produces —
/// `randomUUID()`, hence a LOWERCASE UUID. The coupling is deliberate and
/// named: if that format changed one day, the guard would LOUDLY refuse every
/// upload rather than open a path.
///
/// ⚠️ LOWERCASE ONLY, like `empreinteValide` and for the same reason:
/// on a case-insensitive file system, two distinct
/// identifiers would designate the same directory.
export function identifiantValide(s: string): boolean {
    return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(s);
}

/// 🔴 THE FILE NAME OF A CHUNK IS THE VALIDATED NUMBER `n`, NEVER A
/// COPIED URL SEGMENT. A rank is a SAFE non-negative integer — the zero
/// base is that of `proto/ts/tranches.ts`, where it makes the position in the
/// file computable without a table.
///
/// ⚠️ `Number.isSafeInteger` IS NOT VANITY, and it does TWO
/// things here. Beyond 2^53 the arithmetic of the plan would stop being exact
/// — that is already written in `proto/ts/tranches.ts` — AND `String(n)` would stop
/// being a sequence of digits: `String(1e21)` is `'1e+21'`. Under this
/// guard, the produced name is ALWAYS `/^\d+$/`, which is exactly what
/// `lister` recognises.
export function rangValide(n: number): boolean {
    return Number.isSafeInteger(n) && n >= 0;
}

/// The result of a chunk deposit.
///
/// 🔴 THE BOUNDARY IS THAT OF `proto/ts/tranches.ts`: what is a PROGRAM
/// DEFECT raises, what is WIRE DATA becomes a result. A malformed
/// identifier or rank raises — the route already refused them with
/// `identifiantValide`/`rangValide`, and a call that arrives here with a
/// bad value is faulty wiring, not a clumsy depositor. Exceeding
/// the cap, on the other hand, is the NORMAL behaviour of a peer that sends
/// too much: it must translate into an HTTP refusal without a `catch` having to guess the
/// code from an error message.
export type WriteResult =
    | { ok: true; octets: number }
    | { ok: false; motif: 'plafond-depasse'; plafond: number };

export interface MagasinTranches {
    racine: string;
    /// Deposits a chunk AS A STREAM, under a hard byte cap.
    write(id: string, n: number, flux: AsyncIterable<Uint8Array>, plafondOctets: number): Promise<WriteResult>;
    /// The chunks actually present on the disk, with their sizes.
    lister(id: string): Tranche[];
    /// A stream that concatenates the requested ranks, in the given order.
    concatener(id: string, rangs: readonly number[]): Readable;
    /// Removes a whole upload.
    remove(id: string): void;
    /// Evicts BY AGE, with a REFERENCE FLOOR — see `AGE_EVICTION_TRANCHES_MS`.
    /// `maintenant` is a PARAMETER, never read from the clock: same rule as
    /// everywhere else in this repository.
    evincer(options: { maintenant: number; referencees: ReadonlySet<string> }): Promise<void>;
    /// 🔴 ADDED IN CORRECTION ROUND 2 — the LAST ACTIVITY date
    /// (mtime) of the directory of an upload, or `undefined` if it does not exist
    /// on the disk. It is what lets `apps/nettoyage.ts` make the
    /// purge of the ROW (`cree_a`, in the database) and the eviction of the DISK (`mtime`)
    /// measure THE SAME AGE: without it, an upload CREATED
    /// long ago but where a chunk just arrived saw its row deleted
    /// while its bytes stayed — deterministic, measured by the review,
    /// see `nettoyage.ts::nettoyerTranches`.
    ///
    /// ⚠️ RAISES ON AN INVALID IDENTIFIER, like `lister`/`concatener`/
    /// `remove`: this store never writes such a name itself, and a
    /// caller that hands it one has a wiring defect, not wire data
    /// to absorb silently.
    derniereActivite(id: string): Promise<number | undefined>;
}

/// Opens — or creates — the upload root, and LOGS the path.
///
/// ⚠️ THE LOG LINE IS NOT DECORATIVE, and it is the same argument as
/// for the icon store: `PLATEFORME_TELEVERSEMENTS` is optional, so
/// an operator can get the directory wrong without anything breaking. Here, the
/// consequence is even LESS repairable than for the icons — a lost
/// upload does not rebuild itself, the user has to deposit it again.
/// The retained path must be readable.
export function ouvrirMagasinTranches(
    racine: string,
    journaliser: (chemin: string) => void,
): MagasinTranches {
    mkdirSync(racine, { recursive: true });
    journaliser(racine);

    const repertoireDe = (id: string): string => {
        if (!identifiantValide(id)) {
            throw new Error(`identifiant de téléversement invalide : ${JSON.stringify(id)}`);
        }
        return join(racine, id);
    };

    const cheminDe = (id: string, n: number): string => {
        const rep = repertoireDe(id);
        if (!rangValide(n)) {
            throw new Error(`rang de tranche invalide : ${JSON.stringify(n)}`);
        }
        return join(rep, String(n));
    };

    return {
        racine,

        /// 🔴 AS A STREAM, NEVER BY ACCUMULATING IN MEMORY. A chunk is in
        /// the order of several megabytes and N deposits can run side by side:
        /// a `Buffer.concat` would make the service a memory bomb driven by
        /// its clients.
        ///
        /// 🔴 THE CAP IS HARD, AND IT CUTS THE SOURCE. On crossing it we
        /// RAISE in the middle of the `pipeline`, which destroys the incoming stream:
        /// we stop READING the request body rather than draining it
        /// to throw it away. A cap that let 800 MB flow before
        /// refusing would not be one.
        ///
        /// ⚠️ THE CAP BOUNDS THE DISK, IT DOES NOT JUDGE THE SPLIT. A
        /// chunk SHORTER than expected goes through here without a word: it is
        /// `proto/ts/tranches.ts::verdict` that will declare it `incoherentes` at
        /// sealing, and this module does not know the contract.
        ///
        /// 🔴 THE WRITE IS ATOMIC — temporary file, then `rename` —, and
        /// the stake is HEAVIER HERE THAN FOR THE ICONS. An interrupted deposit
        /// would otherwise leave a TRUNCATED chunk under its final name; the
        /// resumption would see it present, `verdict` would call it `incoherentes`, and
        /// an inconsistency is NOT repaired by requesting again — it makes
        /// the whole upload fail. A network cut would therefore poison an
        /// 800 MB deposit without any trace saying so.
        async write(
            id: string,
            n: number,
            flux: AsyncIterable<Uint8Array>,
            plafondOctets: number,
        ): Promise<WriteResult> {
            const cible = cheminDe(id, n);
            mkdirSync(join(racine, id), { recursive: true });

            // The random suffix keeps two concurrent deposits of the same
            // rank from writing the same temporary file — same defence as `icones.ts`.
            const provisoire = `${cible}.${process.pid}.${Math.random().toString(36).slice(2)}.part`;
            // ⚠️ OPENED SYNCHRONOUSLY, AND THAT IS THE FIX FOR THE RACE
            // described in the `catch` below: from here on the file
            // EXISTS, so the `rmSync` of the error path can no longer
            // miss it. `createWriteStream` receives the descriptor and not the
            // path; it will close it itself (`autoClose`).
            const fd = openSync(provisoire, 'w');
            let octets = 0;
            let depasse = false;
            try {
                await pipeline(
                    flux,
                    async function* borner(source: AsyncIterable<Uint8Array>) {
                        for await (const morceau of source) {
                            octets += morceau.byteLength;
                            if (octets > plafondOctets) {
                                depasse = true;
                                throw new Error(
                                    `tranche ${n} au-delà du plafond de ${plafondOctets} octets`,
                                );
                            }
                            yield morceau;
                        }
                    },
                    createWriteStream('', { fd, autoClose: true }),
                );
            } catch (error) {
                // 🔴 THE PARTIAL FILE IS DELETED, whatever the cause:
                // overflow, cut-off, full disk. An abandoned `.part`
                // is never counted as a slice (see `lister`), but
                // it would occupy the disk until the purge.
                //
                // 🔴 AND THIS `rmSync` WAS INEFFECTIVE — MEASURED, NOT ASSUMED.
                // A direct probe on `write`, outside HTTP, found
                // non-empty directories each carrying a `.part`:
                // **42 out of 400 overflows** on a first measurement, then
                // **100 out of 400** on a second one, under another load.
                // ⚠️ NO RATE IS CLAIMED: the two figures differ
                // by a factor of two and a half depending on the machine load, which
                // is the hallmark of a race. What is established is
                // the existence of the defect, never its frequency.
                // ✅ AFTER THE FIX, THE SAME PROBE GIVES **0 OUT OF 400** —
                // one run per arm, differential played on this file
                // alone, the before state taken back from the repository and not rebuilt.
                // The cause was a RACE, and not
                // a forgotten error path: `createWriteStream(chemin)` opens
                // the file ASYNCHRONOUSLY. On an overflow, our
                // generator throws BEFORE the `open(2)` has completed;
                // `rmSync` then ran on a file that did not exist
                // yet — `{ force: true }` silently swallowing the `ENOENT` —
                // and the open created it right after.
                //
                // ✅ THE REMEDY IS NOT A RETRY BUT A REMOVAL OF THE
                // RACE: the descriptor is opened by `openSync` BEFORE the
                // `pipeline`, so that the inode already exists when the
                // `pipeline` starts. There is therefore no longer an instant where the
                // file is at once "being created" and
                // deletable. A timed retry would have narrowed the window
                // without closing it, and would have made the defect intermittent instead
                // of removing it.
                //
                // ⚠️ This was NOT a protocol hole — `lister` ignores
                // non-numeric names, so no false slice was ever
                // counted and the sealing saw nothing of it. It was a
                // DISK LEAK, on a service that accepts 4 GiB.
                rmSync(provisoire, { force: true });
                if (depasse) return { ok: false, motif: 'plafond-depasse', plafond: plafondOctets };
                throw error;
            }
            renameSync(provisoire, cible);
            return { ok: true, octets };
        },

        /// 🔴 RESUMPTION IS A DIRECTORY LISTING, NEVER BOOKKEEPING.
        /// A `tranches_presentes` table would drift from the disk on the day a
        /// file got lost — and that is PRECISELY the day one needs
        /// to know it. This is what makes the store self-rebuilding:
        /// whatever is missing is asked for again, and the uploader fills it back in.
        ///
        /// ⚠️ ONLY PURELY NUMERIC NAMES ARE SLICES. A `.part`
        /// left by a dead upload is not one, and counting it would make
        /// a slice that never finished being written look complete.
        ///
        /// ⚠️ THE LIST IS SORTED, for the reason given in `proto/ts/tranches.ts`: a
        /// reading that depended on the `readdir` order would be comparable
        /// neither from one run to the next, nor from one file system to another.
        ///
        /// ⚠️ A MISSING DIRECTORY GIVES AN EMPTY LIST, never an error: a
        /// declared upload for which no slice has arrived yet is
        /// the NORMAL state of the first upload, and `verdict` will report it as `manquantes`.
        lister(id: string): Tranche[] {
            const rep = repertoireDe(id);
            let noms: string[];
            try {
                noms = readdirSync(rep);
            } catch {
                return [];
            }
            const tranches: Tranche[] = [];
            for (const nom of noms) {
                if (!/^\d+$/.test(nom)) continue;
                const n = Number(nom);
                if (!rangValide(n)) continue;
                try {
                    const etat = statSync(join(rep, nom));
                    // A slice that vanished between the `readdir` and the `stat` is
                    // simply absent: the listing is a snapshot, and
                    // `verdict` will report it as `manquantes` — which can be repaired.
                    if (etat.isFile()) tranches.push({ n, octets: etat.size });
                } catch {
                    continue;
                }
            }
            return tranches.sort((a, b) => a.n - b.n);
        },

        /// Concatenates the REQUESTED ranks, in the given order.
        ///
        /// 🔴 THE RANKS ARE A PARAMETER, NOT A LISTING, AND THAT IS WHAT
        /// GIVES THE GUARD BELOW ITS MEANING. If this stream listed the
        /// directory itself, a slice gone since the verdict would simply be
        /// absent from the list: the stream would end CLEANLY, shorter,
        /// and the agent would compute a wrong hash without anybody
        /// knowing why. By serving the plan the caller had checked, a
        /// missing file becomes a stream ERROR.
        ///
        /// 🔴 A SLICE DELETED FROM UNDER THE STREAM GIVES AN ERROR,
        /// NEVER A SILENTLY TRUNCATED STREAM. `createReadStream` on a missing
        /// file emits `error`; the iteration rethrows it, the generator dies, and
        /// `Readable.from` destroys the readable with that error — the
        /// consumer receives `error`, not `end`.
        ///
        /// ⚠️ ALL PATHS ARE VALIDATED UP FRONT, before the stream exists:
        /// a faulty rank throws at the call, where the caller can still respond,
        /// rather than in the middle of a response already started.
        concatener(id: string, rangs: readonly number[]): Readable {
            const chemins = rangs.map((n) => cheminDe(id, n));
            return Readable.from(
                (async function* () {
                    for (const chemin of chemins) {
                        for await (const morceau of createReadStream(chemin)) {
                            yield morceau as Uint8Array;
                        }
                    }
                })(),
                // ⚠️ `objectMode: false` IS REQUIRED: `Readable.from` is in object
                // mode by default, and a consumer expecting bytes
                // would receive a stream whose backpressure is counted in
                // chunks rather than in bytes.
                { objectMode: false },
            );
        },

        /// ⚠️ `force`: deleting an upload that never received a
        /// slice is a success, not an error — it is the state of a purge that
        /// runs after an upload abandoned before its first frame.
        remove(id: string): void {
            rmSync(repertoireDe(id), { recursive: true, force: true });
        },

        /// 🔴 THE FLOOR FIRST: a REFERENCED identifier is never
        /// examined for its age, however stale it is — see the
        /// comment of `AGE_EVICTION_TRANCHES_MS`.
        ///
        /// 🔴 THE AGE IS THAT OF THE DIRECTORY, NOT OF A SLICE: each upload
        /// (`write`, through its final `renameSync`) touches the parent directory,
        /// so its timestamp follows the last activity of the whole
        /// upload, slice by slice, without having to list them all.
        ///
        /// ⚠️ A NAME THAT IS NOT A VALID IDENTIFIER IS NEVER TOUCHED,
        /// even if it is old: this store never writes such a name
        /// itself, and a foreign directory is not its responsibility.
        ///
        /// 🔴 DECLARED LEGACY (correction round 3): THE SAME `stat` →
        /// `rm` GAP AS `icones.ts::evincer` — same code shape, same window.
        /// A slice uploaded between the age read and the deletion
        /// can get mowed down. ⚠️ **WITHOUT THE SAFETY NET OF THE ICONS**: on the
        /// icon side, `manquantes` makes every missing hash be asked for again at the
        /// NEXT reconciliation (self-repairing, measured by the review);
        /// on the slice side, NOTHING comparable exists — an upload
        /// mowed down here loses bytes that NO mechanism asks for again
        /// by itself. Not measured separately for this store; declared by
        /// code analogy, not by a dedicated measurement.
        async evincer({ maintenant, referencees }: { maintenant: number; referencees: ReadonlySet<string> }): Promise<void> {
            let noms: string[];
            try {
                noms = await readdir(racine);
            } catch {
                return;
            }
            let i = 0;
            for (const nom of noms) {
                if (i > 0 && i % PAS_DE_REPRISE === 0) await rendreLaMain();
                i += 1;
                if (!identifiantValide(nom) || referencees.has(nom)) continue;
                let mtimeMs: number;
                try {
                    mtimeMs = (await stat(join(racine, nom))).mtimeMs;
                } catch {
                    continue;
                }
                if (maintenant - mtimeMs >= AGE_EVICTION_TRANCHES_MS) {
                    await rm(join(racine, nom), { recursive: true, force: true });
                }
            }
        },

        async derniereActivite(id: string): Promise<number | undefined> {
            const rep = repertoireDe(id);
            try {
                return (await stat(rep)).mtimeMs;
            } catch {
                return undefined;
            }
        },
    };
}
