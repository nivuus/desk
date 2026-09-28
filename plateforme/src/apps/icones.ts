// The icon store of the platform: bytes on DISK, addressed by
// their content, one file per digest.
//
// 🔴 WHY THE DISK AND NOT THE DATABASE. Three reasons, in order of their
// weight:
//
//   1. A BLOB DOES NOT CROSS THE DOUBLE PASS WITHOUT LYING. PostgreSQL has no
//      `BLOB` type (it has `bytea`); SQLite accepts ANY type name
//      by affinity — the lint of `base/sous-ensemble.test.ts`
//      documents it for `SERIAL`, with a measurement to back it. Writing `BYTEA` would therefore pass
//      BOTH passes while meaning two different things: it is the `SERIAL`
//      trap IN REVERSE, and neither of the two guards of the repository catches it.
//   2. The doctrine is already written by the specification, which settles the
//      upload resumption in favour of "a DIRECTORY LISTING, never
//      a bookkeeping table that could diverge from the disk".
//   3. The volume: 4,576,398 bytes measured for this catalogue alone.
//
// 🔴 AND IT IS SELF-REBUILDING THAT MAKES THE DISK ACCEPTABLE, not a
// convenience. A lost store — container without a volume, misnamed directory —
// refills on its own: the inventory of missing icons queries the DISK, so
// everything missing is requested again at the next reconciliation. That is
// acceptance criterion ⑦, and it must be TESTED rather than assumed.

import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { existsSync } from 'node:fs';
import { readdir, rm, stat } from 'node:fs/promises';
import { join } from 'node:path';

/// 🔴 EVERY 50 NAMES EXAMINED, THE ROUND YIELDS TO THE EVENT LOOP
/// (correction round 2) — `setImmediate` rather than a `Promise.resolve()`
/// : the latter only schedules a MICROTASK, which yields to NO pending
/// I/O or timer; `setImmediate` schedules a REAL task, after
/// the "poll" phase — which lets an HTTP request or a WebSocket message
/// already ready run before the next entry. MEASURED (bench of 20,000
/// icons, 25 August 2026, BEFORE this remedy): 222 ms in one block,
/// ZERO 10 ms beat served during it (≈22 expected) — the port was
/// open, and nothing answered. ⚠️ NOT CALIBRATED: 50 is reasoned (small enough
/// that no pending I/O waits more than a few passes over
/// entries, large enough not to drown the round in scheduling
/// tasks), never finely measured. The measurement BEFORE this remedy is
/// that of the REVIEW (correction round 2), not mine: repeated here
/// so as not to lose it, with its provenance stated.
const PAS_DE_REPRISE = 50;

async function rendreLaMain(): Promise<void> {
    await new Promise<void>((resolve) => setImmediate(resolve));
}

/// ⚠️ **NOT CALIBRATED.** No constant of this repository is.
///
/// 🔴 THE REFERENCE FLOOR (see `evincer`, below) IS WHAT TELLS
/// AN EVICTION FROM A CORRUPTION: evicting an icon still named by an
/// application would make its image vanish without anything saying so.
///
/// ⚠️ WHAT THIS RULE DOES NOT DO: it does NOT bound the disk. A
/// catalogue that keeps growing keeps growing. The size cap was
/// DISMISSED by decision, because it can evict an object still referenced
/// — that is, trade a visible growth for a silent failure.
export const AGE_EVICTION_ICONE_MS = 180 * 24 * 60 * 60_000;

/// 🔴 EXACTLY 64 LOWERCASE HEXADECIMAL CHARACTERS, AND NOTHING ELSE.
///
/// Without this guard, `:sha256` is A PATH COMPONENT SUPPLIED BY THE
/// NETWORK, and `..` is meaningful in it. The repository already says "never
/// interpolate a raw environment value into a path component";
/// here it is WORSE — it comes from a peer.
///
/// ⚠️ LOWERCASE ONLY, and it is not vanity: on a case-insensitive
/// file system, `AB…` and `ab…` would designate the SAME
/// file under two different digests, and content addressing
/// would stop being a bijection.
export function empreinteValide(s: string): boolean {
    return /^[0-9a-f]{64}$/.test(s);
}

export interface Magasin {
    possede(empreinte: string): boolean;
    manquantes(annoncees: readonly string[]): string[];
    ecrire(empreinte: string, octets: Buffer): void;
    lire(empreinte: string): Buffer | undefined;
    /// Evicts BY AGE, with a REFERENCE FLOOR — see `AGE_EVICTION_ICONE_MS`.
    /// `maintenant` is a PARAMETER, never read from the clock: same rule as
    /// everywhere else in this repository (`depot/application.ts`, etc.), and that is
    /// what makes `icones.test.ts` able to replay an exact age.
    evincer(options: { maintenant: number; referencees: ReadonlySet<string> }): Promise<void>;
    repertoire: string;
}

/// Opens — or creates — the store, and LOGS the retained path.
///
/// ⚠️ THE LOG LINE IS NOT DECORATIVE: `PLATEFORME_ICONES` is
/// optional, so an operator can get the directory wrong without anything
/// breaking — the store would rebuild itself elsewhere, silently, by
/// re-uploading everything. The retained path must be readable.
export function ouvrirMagasin(repertoire: string, journaliser: (chemin: string) => void): Magasin {
    mkdirSync(repertoire, { recursive: true });
    journaliser(repertoire);

    const chemin = (empreinte: string): string => {
        if (!empreinteValide(empreinte)) {
            throw new Error(`empreinte d'icône invalide : ${JSON.stringify(empreinte)}`);
        }
        return join(repertoire, empreinte);
    };

    return {
        repertoire,

        /// 🔴 A FILE EXISTENCE, NEVER A TABLE. A bookkeeping
        /// table would diverge from the store the day a file got
        /// lost — and that is PRECISELY the day one needs to know.
        possede(empreinte: string): boolean {
            return empreinteValide(empreinte) && existsSync(join(repertoire, empreinte));
        },

        /// The complement, IN ANNOUNCEMENT ORDER and without duplicates.
        ///
        /// ⚠️ The announcement order is that of the catalogue, hence the one in which
        /// the user will see the icons arrive. Sorting would lose it for nothing.
        ///
        /// ⚠️ A MALFORMED DIGEST IS NOT "MISSING": it is
        /// ignored. Requesting it again would make the agent loop on a value the
        /// route would refuse anyway.
        manquantes(annoncees: readonly string[]): string[] {
            const vues = new Set<string>();
            const manque: string[] = [];
            for (const e of annoncees) {
                if (!empreinteValide(e) || vues.has(e)) continue;
                vues.add(e);
                if (!existsSync(join(repertoire, e))) manque.push(e);
            }
            return manque;
        },

        /// 🔴 RECOMPUTES THE DIGEST, AND REFUSES IF IT DIFFERS.
        ///
        /// It is the third of the three checks of the specification —
        /// "no hop trusts the previous one". **Without it,
        /// content addressing would NOT be content addressing**: a faulty agent
        /// would poison the store with a file that does not match its
        /// name, and the `Cache-Control: immutable` of the route would make
        /// the poisoning PERMANENT in the caches.
        ///
        /// 🔴 THE WRITE IS ATOMIC: temporary file then `rename`. An
        /// interrupted `PUT` would otherwise leave a TRUNCATED file **under a name
        /// that promises its content**, and the next link would serve it without
        /// ever reading it back.
        ecrire(empreinte: string, octets: Buffer): void {
            const cible = chemin(empreinte);
            const reel = createHash('sha256').update(octets).digest('hex');
            if (reel !== empreinte) {
                throw new Error(
                    `empreinte annoncée ${empreinte} mais contenu en ${reel} : refusé`,
                );
            }
            // The random suffix keeps two concurrent `PUT`s of the same
            // digest from writing the same temporary file.
            const provisoire = `${cible}.${process.pid}.${Math.random().toString(36).slice(2)}.part`;
            try {
                writeFileSync(provisoire, octets);
                renameSync(provisoire, cible);
            } catch (erreur) {
                rmSync(provisoire, { force: true });
                throw erreur;
            }
        },

        lire(empreinte: string): Buffer | undefined {
            if (!empreinteValide(empreinte)) return undefined;
            try {
                return readFileSync(join(repertoire, empreinte));
            } catch {
                return undefined;
            }
        },

        /// 🔴 THE FLOOR FIRST: a REFERENCED digest is never
        /// examined for its age, however stale it is. It is the only
        /// thing that tells an eviction from a corruption — see the
        /// comment of `AGE_EVICTION_ICONE_MS`.
        ///
        /// ⚠️ A NAME THAT IS NOT A VALID DIGEST IS NEVER TOUCHED:
        /// a foreign file dropped by hand into the store (the case
        /// covered by `icones.test.ts::'un fichier étranger…'`) is not the
        /// responsibility of this eviction.
        ///
        /// 🔴 DECLARED LEGACY (correction round 3): THE `stat` → `rm` GAP
        /// CAN MOW DOWN A CONCURRENT WRITE. Between reading the age
        /// and the deletion, an `ecrire()` on this SAME name (rewrite of an
        /// icon recently requested again by the reconciliation, for example)
        /// can fall into the window — the file has just been touched,
        /// but its age was read BEFORE. MEASURED BY THE REVIEW: 0 to 3 icons
        /// out of 2,000 mowed down despite a fresh date, over 5 runs.
        /// THIS WINDOW DID NOT EXIST WHEN SYNCHRONOUS (`stat` then `rm`
        /// followed each other without any concurrent I/O being able to
        /// slip in). ⚠️ SELF-HEALING HERE, NOT ELSEWHERE: the
        /// next reconciliation (`agents/canal-apps.ts::manquantes`)
        /// requests again any missing digest — the review checked that this
        /// promise is not vacuous. `MagasinTranches.evincer` has EXACTLY
        /// the same code shape, hence the SAME gap, but WITHOUT this safety net:
        /// see its own comment.
        async evincer({ maintenant, referencees }: { maintenant: number; referencees: ReadonlySet<string> }): Promise<void> {
            let noms: string[];
            try {
                noms = await readdir(repertoire);
            } catch {
                return;
            }
            let i = 0;
            for (const nom of noms) {
                if (i > 0 && i % PAS_DE_REPRISE === 0) await rendreLaMain();
                i += 1;
                if (!empreinteValide(nom) || referencees.has(nom)) continue;
                let mtimeMs: number;
                try {
                    mtimeMs = (await stat(join(repertoire, nom))).mtimeMs;
                } catch {
                    continue;
                }
                if (maintenant - mtimeMs >= AGE_EVICTION_ICONE_MS) {
                    await rm(join(repertoire, nom), { force: true });
                }
            }
        },
    };
}
