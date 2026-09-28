// The background cleanup of the TWO on-disk stores (icons, slices).
//
// 🔴 WITHOUT THIS FILE, `Magasin.evincer` AND `MagasinTranches.evincer` ARE A
// MECHANISM WRITTEN, TESTED, DOCUMENTED — AND CALLED BY NOBODY. That is the most
// discreet failure possible (correction round 1, 25 August 2026, on this
// same work): the legacy said "the stores are NEVER cleaned",
// and an eviction that nothing invokes does not close it — it creates a
// second one, harder to see than the first, because it LOOKS closed.
//
// 🔴 THE CADENCE FOLLOWS A PRECEDENT OF THE REPOSITORY, IT DOES NOT INVENT ONE. Two
// background mechanisms already exist in this service, and NEITHER OF THEM HAS A
// TIMER: `routes-sante.ts::CacheSante` recomputes at the latest on the
// NEXT READ (`PERIODE_SANTE_MS`), and `securite/frein.ts::Frein`
// evicts its entries at the moment of a following FAILURE (`faireDeLaPlace`) — both
// are LAZY, triggered by usage. An on-disk store does not have
// that usage: the route that READS an icon (`routes-icone.ts`) is the HOT
// path of a browser, and grafting a directory sweep onto it would break the
// same rule as "never trace per packet in the transport loop"
// — an upkeep cost paid by every useful request. The only precedent
// of a PERIODIC BACKGROUND TASK (a dedicated timer) in this whole repository lives on the
// AGENT side: the periodic reconciliation of the application catalogue
// (sub-block G4, `APPS_SURVEILLANCE`, a `Mode` that carries its own period).
// It is that SHAPE that is reused here — a dedicated timer, an UNCALIBRATED
// period, explicitly stoppable when the service closes — never its
// figure, specific to another process and to another work cadence.
//
// ⚠️ UNCALIBRATED, like every constant of this repository.
export const PERIODE_NETTOYAGE_MS = 6 * 60 * 60_000; // 6 heures.

import type { Pilote } from '../base/pilote';
import type { Magasin } from './icones';
import { AGE_EVICTION_TRANCHES_MS, type MagasinTranches } from './magasin-tranches';
import {
    lirePlusVieuxQue as televersementsPlusVieuxQue,
    remove as removeUpload,
} from '../depot/televersement';

/// Same remedy, same reason as `icones.ts::PAS_DE_REPRISE` — the loop that
/// tries to purge a BACKLOG of rows too old must not
/// freeze the service on its own either. ⚠️ UNCALIBRATED, same reasoning.
const PAS_DE_REPRISE = 50;

async function rendreLaMain(): Promise<void> {
    await new Promise<void>((resolve) => setImmediate(resolve));
}

/// The set of icon hashes REFERENCED by a LIVE entry of the
/// catalogue.
///
/// 🔴 A HIDDEN ENTRY STAYS LIVE, AND THIS IS DELIBERATE: `masquee_a` is a
/// DISPLAY gesture (`depot/application.ts`), never a deletion — only
/// `disparue_a` removes a row in the sense of this floor. Also filtering on
/// `masquee_a IS NULL`, as `lireParVm` does for DISPLAY, would evict
/// the icon of an application that can be unhidden, and unhiding it would find it
/// broken — that is exactly the corruption the floor exists to
/// prevent.
///
/// 🔵 RESIDUAL WINDOW DECLARED, NOT FIXED (correction round 2): the
/// references are read, THEN an application adopts an old icon that was
/// orphaned until then, THEN the eviction runs on the snapshot read before
/// that adoption — the icon then goes away just as it has become live again.
/// SELF-REPAIRING, NOT A LOSS: the next reconciliation
/// (`agents/canal-apps.ts::deps.magasin.manquantes`) asks again for every
/// missing hash, exactly as a lost store rebuilds itself
/// (see the header of `icones.ts`, criterion ⑦). The icon is broken for
/// a window bounded by `PERIODE_NETTOYAGE_MS`, never indefinitely.
///
/// 🔴 THIS WINDOW HAS GROWN, AND IT HAD TO BE SAID — FIXED (correction
/// round 3): it lasts as long as the call to
/// `magasin.evincer` itself (the snapshot is read BEFORE, the eviction runs
/// AFTER), and the latter has become SLOWER since Important ② (switch
/// to async + resumption points). MEASURED, in paired A/B within one
/// run: at 1,000 icons, ≈55 ms (against ≈8 ms before async — icons
/// granting no merit to the accuracy of the third decimal, the order
/// of magnitude is what counts); at 20,000 icons, ≈640 ms (against ≈147 ms
/// before). The window grows along TWO axes at once: with the size of the
/// catalogue (already true before this batch) AND with the switch to async
/// (extra factor of about 4 to 7×, measured). ⚠️ ON THE SLICE SIDE,
/// THE SNAPSHOT IS FRESH: `referencesTranches` is read just BEFORE
/// `tranches.evincer`, in the last line of `nettoyerTranches` — no
/// long eviction slips in between the read and the use, unlike
/// the icons.
export async function referencesIcones(p: Pilote): Promise<Set<string>> {
    const lignes = await p.interroger<{ icone: string }>(
        'SELECT DISTINCT icone FROM application WHERE disparue_a IS NULL AND icone IS NOT NULL',
        [],
    );
    return new Set(lignes.map((l) => l.icone));
}

/// First closes the upload ROWS that have become too old, THEN
/// evicts from the DISK whatever no longer has a row.
///
/// 🔴 THE ROW FIRST, THE DISK AFTERWARDS — AND IT IS THE ORDER THAT MAKES THE
/// FLOOR REAL. `depot/televersement.ts::lirePlusVieuxQue`/`remove`
/// already existed, FULLY TESTED (`depot/installation.test.ts`,
/// "the age sweep…", "REFUSES to delete an upload that an
/// installation references"), and were called by NO production
/// code: a THIRD dead mechanism, and this time the comment of
/// their own test says it in advance — "the disk SLICES, for their part, are
/// swept ELSEWHERE". This file is that "elsewhere". The deletion
/// of a row is REFUSED BY THE FOREIGN KEY as long as an `installation` still
/// references it ("the history of an installation must remain
/// readable"): that refusal IS the floor, at the DATABASE level, even before
/// the disk one. Evicting the disk BEFORE this purge would leave a
/// window where a row just freed has not yet had the chance to
/// protect its directory; the reverse order carries no symmetric risk — a
/// row still present protects its directory until the NEXT ROUND at worst.
///
/// 🔴 CRITICAL NARROWED TO A RACE, NOT CLOSED (correction round 2, then
/// FIXED in correction round 3) — THE TWO FLOORS MUST MEASURE THE
/// SAME AGE. `lirePlusVieuxQue` filters on `cree_a` (the row),
/// `MagasinTranches.evincer` filters on `mtime` (the disk, the LAST
/// ACTIVITY) — IT IS NOT THE SAME THING, and it was DETERMINISTIC, not a
/// race: an upload created 31 days ago one of whose slices had just
/// arrived THIS VERY INSTANT had its ROW deleted (a candidate by `cree_a`,
/// no installation referencing it) while its DIRECTORY remained
/// intact (too young by `mtime`) — the resumption dies (`routes-televersement.ts`
/// returns `404 televersement-inconnu` on bytes that are nonetheless PRESENT), and the
/// disk is not even freed. That is the corruption the floor exists
/// to prevent, coming in through the DATABASE door rather than through the
/// DISK one. THE REMEDY: the row is deleted ONLY IF ITS DIRECTORY IS ALSO
/// EVICTABLE (or absent) — `derniereActivite` makes the two floors measure the SAME
/// age, WITHOUT TOUCHING THE SCHEMA.
///
/// 🔴 IT IS NOT "BY CONSTRUCTION" — THAT WAS FALSE, AND THE REVIEW
/// MEASURED IT (correction round 3, scenario ⑤): the disk age is read TWICE,
/// AT TWO DIFFERENT INSTANTS — here (`derniereActivite`), then
/// AGAIN in `MagasinTranches.evincer` (its own `stat`, run after
/// going through the whole rest of the loop and `referencesTranches`) —
/// and the window between the two reads covers THE WHOLE ROUND. A slice that
/// arrives BETWEEN THESE TWO READS reproduces the round 2 Critical
/// IDENTICALLY: row deleted, bytes kept, resumption dead, nothing
/// freed. 🔵 IT IS REAL PROGRESS — from a DETERMINISTIC defect (100 % of
/// rounds) to a RACE (a narrow window, open only for THE
/// DURATION OF A ROUND) — but it is not a closure, and writing "by
/// construction" would have passed it off as one.
///
/// ⚠️ THE REAL CLOSURE IS A LEAD FOR A LATER BATCH, NOT A
/// CONDITION OF THIS CLOSURE: `rm` the directory IN THE SAME ITERATION
/// as the deletion of the row (a single age read, reused for
/// both gestures), rather than relying on the separate `evincer`
/// pass that rereads the age from scratch at the end of the round.
async function nettoyerTranches(
    p: Pilote,
    tranches: MagasinTranches,
    maintenant: number,
): Promise<void> {
    const seuil = maintenant - AGE_EVICTION_TRANCHES_MS;
    // 🔵 `lirePlusVieuxQue` loads the WHOLE ROW (name, size, sha256…)
    // whereas only `id` is used here — DECLARED, NOT FIXED (correction
    // round 2): a memory peak is possible on a backlog, but
    // adding a narrower query would duplicate a primitive ALREADY
    // tested (`depot/installation.test.ts`) to gain a saving that only
    // counts if the backlog is huge — a database unreachable for
    // weeks, a case that has other symptoms before this one.
    let i = 0;
    for (const ligne of await televersementsPlusVieuxQue(p, seuil)) {
        if (i > 0 && i % PAS_DE_REPRISE === 0) await rendreLaMain();
        i += 1;

        let activite: number | undefined;
        try {
            activite = await tranches.derniereActivite(ligne.id);
        } catch {
            // 🔴 HARDENING (correction round 3): NOT REACHABLE
            // TODAY — `create` (`depot/televersement.ts`) is the ONLY
            // `INSERT` of this table and always sets a UUID —, but
            // `derniereActivite` THROWS on an invalid identifier (same
            // convention as `lister`/`concatener`/`remove`). WITHOUT this
            // `catch`, a single malformed row would make the exception BUBBLE UP
            // out of the loop: the whole round would stop there, the NEIGHBOURING
            // orphan — legitimate though it is — would never be evicted, NEITHER IN THIS
            // ROUND NOR IN THE FOLLOWING ONES (the faulty row would stay ahead of it in
            // the same read order, every round). `continue`: we skip
            // the FAULTY row, never the round.
            continue;
        }
        if (activite !== undefined && maintenant - activite < AGE_EVICTION_TRANCHES_MS) {
            // The DIRECTORY is still active: the row stays, whatever
            // the age of `cree_a`. This is the remedy to the critical above.
            continue;
        }

        try {
            await removeUpload(p, ligne.id);
        } catch (cause) {
            if (!estRefusDeCleEtrangere(cause)) {
                // ⚠️ Important ① (correction round 2): ONLY the foreign
                // key refusal stays silent — it is EXPECTED, see above.
                // Any other failure (database cut off, full disk…) IS NOT
                // EXPECTED at all and must be visible: measured by the review, a
                // bare `catch {}` produced ZERO log lines on a partial
                // outage. The amplification argument does not apply:
                // this loop is BOUNDED, and runs at most once every six
                // hours — nothing to do with one packet per frame.
                console.error(
                    `purge de la ligne de televersement ${ligne.id} en echec : ${String(cause)}`,
                );
            }
        }
    }
    await tranches.evincer({ maintenant, referencees: await referencesTranches(p) });
}

/// Tells a FOREIGN KEY refusal — the ONLY expected failure of
/// `removeUpload` — apart from everything else.
///
/// 🔴 A CODE, NOT A TEXT: `errcode` (node:sqlite,
/// `SQLITE_CONSTRAINT_FOREIGNKEY = 787`, measured on this repository — see
/// `nettoyage.test.ts`) and `code` (`pg`, `23503`, the stable
/// `foreign_key_violation` code of Postgres) are STABLE CODES, published by
/// each engine — never the `message`, which is prose and can change
/// from one version to the next without anything signalling it here.
function estRefusDeCleEtrangere(cause: unknown): boolean {
    if (!(cause instanceof Error)) return false;
    const e = cause as Error & { errcode?: unknown; code?: unknown };
    return e.errcode === 787 || e.code === '23503';
}

/// The set of REFERENCED upload identifiers — in the broadest
/// sense: the ROW still exists in `televersement`. Exported separately,
/// like `referencesIcones`, so that the guard "the set is never empty
/// by accident" (`nettoyage.test.ts`) can measure it without going through a
/// full round.
export async function referencesTranches(p: Pilote): Promise<Set<string>> {
    const lignes = await p.interroger<{ id: string }>('SELECT id FROM televersement', []);
    return new Set(lignes.map((l) => l.id));
}

/// ONE cleanup round, on the TWO stores. Exported separately from the
/// timer to stay testable WITHOUT `setInterval` — see
/// `nettoyage.test.ts`.
///
/// 🔵 NO RE-ENTRANCY GUARD — DECLARED, NOT ADDED (correction round
/// 2). In PRODUCTION, `PERIODE_NETTOYAGE_MS` (6 h) is several orders of
/// magnitude larger than the duration of a round (a fraction of a second, even
/// at the scale of the review benches): two overlapping rounds is
/// not a case we expect to see. And if it happened anyway — a
/// tiny test `periodeMs`, for instance —, the two halves of the round
/// are IDEMPOTENT: `evincer` on a file already gone is a `rm force`
/// that finds nothing, and `remove` on a row already purged touches zero
/// rows. A guard would add surface for a risk that does not
/// need it.
///
/// 🔴 FIXED (correction round 3): THIS ANALYSIS ASSUMED THE ROUND
/// WAS SYNCHRONOUS, and since the remedy to Important ② (`evincer` yields
/// between entries), the sentence said that the switch to async "brings closer
/// — WITHOUT OPENING IT —" the window of an overlap. **THAT WAS FALSE.** The
/// review MEASURED, at a tiny test cadence: 78 SIMULTANEOUS ROUNDS, and
/// `arreter()` leaves 77 of them IN FLIGHT, each one logging `database is not
/// open` once the database is closed from under it — the overlap is indeed
/// REAL, not merely brought closer. 🔵 WITHOUT CONSEQUENCE IN PRODUCTION (the
/// period, six hours, stays orders of magnitude above the duration
/// of a round) AND WITHOUT CORRUPTION (the idempotence above holds — confirmed
/// by the review on FOUR disagreement scenarios, on BOTH engines) —
/// but the sentence must say what IS, not what we would like it to be.
/// Idempotence remains the guarantor; the absence of overlap is no
/// longer one, and never was at a tiny cadence.
export async function unTour(deps: {
    base: Pilote;
    magasin: Magasin;
    tranches: MagasinTranches;
    maintenant: number;
}): Promise<void> {
    const refs = await referencesIcones(deps.base);
    await deps.magasin.evincer({ maintenant: deps.maintenant, referencees: refs });
    await nettoyerTranches(deps.base, deps.tranches, deps.maintenant);
}

/// Starts the background cleanup: an IMMEDIATE and AWAITED round, then a round
/// every `periodeMs` in the background (THOSE are never awaited,
/// just as the rest of this service never reads a timer while blocking a
/// request).
///
/// ⚠️ WHY THE FIRST ROUND IS AWAITED, STATED FOR WHAT IT IS (correction
/// round 2, on a finding of the review): the DECISIVE reason is
/// TESTABILITY, not a property of the product — it is what makes the round 1
/// failure REPRODUCIBLE BY A DETERMINISTIC TEST (`http/serveur.test.ts`
/// starts a real service and watches an orphan icon disappear WITHOUT
/// calling `evincer` by hand, without polling or an arbitrary delay, precisely
/// because this first round is awaited before `startServer` returns
/// control). That a freshly restarted service does not wait a full
/// period before its first sweep is a real but SECONDARY benefit:
/// one more DB query before `http.listen` has a startup latency cost
/// that this file has not measured, and nothing rules out that one day this
/// cost outweighs the benefit — it would then be a trade-off to revisit,
/// not a regression of this remedy.
///
/// ⚠️ NO ERROR BUBBLES UP beyond a round: an unreachable database or a
/// full disk must neither interrupt the startup nor prevent the NEXT
/// round — same philosophy as the `.catch` of each HTTP request in
/// `serveur.ts`. It is logged, never silently swallowed.
export async function startCleanup(
    deps: { base: Pilote; magasin: Magasin; tranches: MagasinTranches; maintenant: () => number },
    periodeMs: number = PERIODE_NETTOYAGE_MS,
): Promise<{ arreter(): void }> {
    const tour = async (): Promise<void> => {
        try {
            await unTour({ ...deps, maintenant: deps.maintenant() });
        } catch (cause) {
            console.error(`nettoyage des magasins en echec : ${String(cause)}`);
        }
    };
    await tour();
    const minuteur = setInterval(() => {
        void tour();
    }, periodeMs);
    // 🔴 `unref()`: this timer must never be, ON ITS OWN, the reason
    // why the process stays alive — `close()` (see `serveur.ts`)
    // remains the NORMAL shutdown path, `unref()` is only the safety net.
    minuteur.unref();
    return {
        /// 🔴 WHAT `arreter()` DOES, AND WHAT IT DOES NOT DO — FIXED (correction
        /// round 2): the previous sentence suggested that calling
        /// this method prevented a running round from hitting a database
        /// about to be closed. **FALSE**: `clearInterval` only prevents the
        /// NEXT SCHEDULING — it does NOT interrupt a round already in flight.
        /// A round started just before this call goes on, `await`s its
        /// queries, and can still fail (and log itself, see
        /// above) if the database closes in the meantime. What this method really
        /// GUARANTEES: after it returns, NO NEW round will start.
        /// `nettoyage.test.ts` holds the proof of it — FIXED (correction
        /// round 3): that test calls `arreter()` DIRECTLY, it closes
        /// NO service, and checks that no extra round
        /// runs beyond the one already in flight at the time of the call. The
        /// REAL path — `close()` of `serveur.ts`, which calls THIS
        /// method — is exercised too, but by the `afterEach` of
        /// `serveur.test.ts`, not by the test quoted here.
        arreter(): void {
            clearInterval(minuteur);
        },
    };
}
