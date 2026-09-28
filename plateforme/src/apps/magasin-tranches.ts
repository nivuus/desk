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
/// The required shape is the one `depot/televersement.ts::creer` produces —
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
export type ResultatEcriture =
    | { ok: true; octets: number }
    | { ok: false; motif: 'plafond-depasse'; plafond: number };

export interface MagasinTranches {
    racine: string;
    /// Deposits a chunk AS A STREAM, under a hard byte cap.
    ecrire(id: string, n: number, flux: AsyncIterable<Uint8Array>, plafondOctets: number): Promise<ResultatEcriture>;
    /// The chunks actually present on the disk, with their sizes.
    lister(id: string): Tranche[];
    /// A stream that concatenates the requested ranks, in the given order.
    concatener(id: string, rangs: readonly number[]): Readable;
    /// Removes a whole upload.
    supprimer(id: string): void;
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
    /// `supprimer`: this store never writes such a name itself, and a
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
        async ecrire(
            id: string,
            n: number,
            flux: AsyncIterable<Uint8Array>,
            plafondOctets: number,
        ): Promise<ResultatEcriture> {
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
            } catch (erreur) {
                // 🔴 LE FICHIER PARTIEL EST SUPPRIMÉ, quelle que soit la cause :
                // dépassement, coupure, disque plein. Un `.part` abandonné
                // n'est jamais compté comme une tranche (voir `lister`), mais
                // il occuperait le disque jusqu'à la purge.
                //
                // 🔴 ET CE `rmSync` A ÉTÉ INEFFICACE — MESURÉ, PAS SUPPOSÉ.
                // Une sonde directe sur `ecrire`, hors HTTP, a relevé des
                // répertoires non vides portant chacun un `.part` :
                // **42 sur 400 dépassements** à une première mesure, puis
                // **100 sur 400** à une seconde, sous une autre charge.
                // ⚠️ AUCUN TAUX N'EST REVENDIQUÉ : les deux chiffres diffèrent
                // d'un facteur deux et demi selon la charge de la machine, ce
                // qui est le propre d'une course. Ce qui est établi est
                // l'existence du défaut, jamais sa fréquence.
                // ✅ APRÈS LE CORRECTIF, LA MÊME SONDE REND **0 SUR 400** —
                // une exécution par bras, différentiel joué sur ce fichier
                // seul, l'état d'avant repris du dépôt et non reconstruit.
                // La cause était une COURSE, et non
                // un chemin d'erreur oublié : `createWriteStream(chemin)` ouvre
                // le fichier de façon ASYNCHRONE. Sur un dépassement, notre
                // générateur lève AVANT que l'`open(2)` n'ait abouti ;
                // `rmSync` courait alors sur un fichier qui n'existait pas
                // encore — `{ force: true }` avalant le `ENOENT` en silence —
                // et l'ouverture le créait juste après.
                //
                // ✅ LE REMÈDE N'EST PAS UN RÉESSAI MAIS UNE SUPPRESSION DE LA
                // COURSE : le descripteur est ouvert par `openSync` AVANT le
                // `pipeline`, si bien que l'inode existe déjà quand le
                // `pipeline` démarre. Il n'y a donc plus d'instant où le
                // fichier soit à la fois « en cours de création » et
                // supprimable. Un réessai temporisé aurait réduit la fenêtre
                // sans la fermer, et aurait rendu le défaut intermittent au
                // lieu de le supprimer.
                //
                // ⚠️ Ce n'était PAS un trou de protocole — `lister` ignore les
                // noms non numériques, donc aucune fausse tranche n'a jamais
                // été comptée et le scellement n'en voyait rien. C'était une
                // FUITE DE DISQUE, sur un service qui accepte 4 Gio.
                rmSync(provisoire, { force: true });
                if (depasse) return { ok: false, motif: 'plafond-depasse', plafond: plafondOctets };
                throw erreur;
            }
            renameSync(provisoire, cible);
            return { ok: true, octets };
        },

        /// 🔴 LA REPRISE EST UN LISTAGE DE RÉPERTOIRE, JAMAIS UNE COMPTABILITÉ.
        /// Une table `tranches_presentes` divergerait du disque le jour où un
        /// fichier serait perdu — et c'est PRÉCISÉMENT le jour où l'on a besoin
        /// de le savoir. C'est ce qui rend le magasin auto-reconstructible :
        /// tout ce qui manque est redemandé, et le déposant recomplète.
        ///
        /// ⚠️ SEULS LES NOMS PUREMENT NUMÉRIQUES SONT DES TRANCHES. Un `.part`
        /// laissé par un dépôt mort n'en est pas une, et le compter ferait
        /// paraître complète une tranche qui n'a jamais fini de s'écrire.
        ///
        /// ⚠️ LA LISTE EST TRIÉE, pour la raison de `proto/ts/tranches.ts` : un
        /// relevé qui dépendrait de l'ordre de `readdir` ne serait comparable
        /// ni d'une exécution à l'autre, ni d'un système de fichiers à l'autre.
        ///
        /// ⚠️ UN RÉPERTOIRE ABSENT REND UNE LISTE VIDE, jamais une erreur : un
        /// téléversement déclaré dont aucune tranche n'est encore arrivée est
        /// l'état NORMAL du premier dépôt, et `verdict` en dira `manquantes`.
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
                    // Une tranche disparue entre le `readdir` et le `stat` est
                    // simplement absente : le listage est un instantané, et
                    // `verdict` la dira `manquantes` — ce qui se répare.
                    if (etat.isFile()) tranches.push({ n, octets: etat.size });
                } catch {
                    continue;
                }
            }
            return tranches.sort((a, b) => a.n - b.n);
        },

        /// Concatène les rangs DEMANDÉS, dans l'ordre donné.
        ///
        /// 🔴 LES RANGS SONT UN PARAMÈTRE, PAS UN LISTAGE, ET C'EST CE QUI
        /// DONNE SON SENS À LA GARDE CI-DESSOUS. Si ce flux listait lui-même le
        /// répertoire, une tranche disparue depuis le verdict serait simplement
        /// absente de la liste : le flux se terminerait PROPREMENT, plus court,
        /// et l'agent calculerait une empreinte fausse sans que personne ne
        /// sache pourquoi. En servant le plan que l'appelant a fait vérifier, un
        /// fichier manquant devient une ERREUR de flux.
        ///
        /// 🔴 UNE TRANCHE SUPPRIMÉE SOUS LES PIEDS DU FLUX DONNE UNE ERREUR,
        /// JAMAIS UN FLUX TRONQUÉ SILENCIEUX. `createReadStream` sur un fichier
        /// absent émet `error` ; l'itération le relance, le générateur meurt, et
        /// `Readable.from` détruit le lisible avec cette erreur — le
        /// consommateur reçoit `error`, pas `end`.
        ///
        /// ⚠️ TOUS LES CHEMINS SONT VALIDÉS D'AVANCE, avant que le flux n'existe :
        /// un rang fautif lève à l'appel, où l'appelant peut encore répondre,
        /// plutôt qu'au milieu d'une réponse déjà commencée.
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
                // ⚠️ `objectMode: false` EST EXIGÉ : `Readable.from` est en mode
                // objet par défaut, et un consommateur qui attend des octets
                // recevrait un flux dont la contre-pression se compte en
                // morceaux plutôt qu'en octets.
                { objectMode: false },
            );
        },

        /// ⚠️ `force` : supprimer un téléversement qui n'a jamais reçu de
        /// tranche est un succès, pas une erreur — c'est l'état d'une purge qui
        /// passe après un dépôt abandonné avant sa première trame.
        supprimer(id: string): void {
            rmSync(repertoireDe(id), { recursive: true, force: true });
        },

        /// 🔴 LE PLANCHER D'ABORD : un identifiant RÉFÉRENCÉ n'est jamais
        /// examiné pour son âge, quelle que soit sa vétusté — voir le
        /// commentaire de `AGE_EVICTION_TRANCHES_MS`.
        ///
        /// 🔴 L'ÂGE EST CELUI DU RÉPERTOIRE, PAS D'UNE TRANCHE : chaque dépôt
        /// (`ecrire`, via son `renameSync` final) touche le répertoire parent,
        /// donc son horodatage suit la dernière activité du téléversement
        /// entier, tranche par tranche, sans qu'il faille les lister toutes.
        ///
        /// ⚠️ UN NOM QUI N'EST PAS UN IDENTIFIANT VALIDE N'EST JAMAIS TOUCHÉ,
        /// même s'il est vieux : ce magasin n'écrit jamais un tel nom
        /// lui-même, et un répertoire étranger n'est pas sa responsabilité.
        ///
        /// 🔴 LEG DÉCLARÉ (round de correction 3) : LA MÊME CÉSURE `stat` →
        /// `rm` QUE `icones.ts::evincer` — même forme de code, même fenêtre.
        /// Une tranche déposée entre la lecture de l'âge et la suppression
        /// peut se faire faucher. ⚠️ **SANS LE FILET DES ICÔNES** : côté
        /// icônes, `manquantes` fait redemander toute empreinte absente à la
        /// PROCHAINE réconciliation (auto-réparant, mesuré par la revue) ;
        /// côté tranches, RIEN de comparable n'existe — un téléversement
        /// fauché ici perd des octets qu'AUCUN mécanisme ne redemande de
        /// lui-même. Non mesuré séparément pour ce magasin ; déclaré par
        /// analogie de code, pas par une mesure dédiée.
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
