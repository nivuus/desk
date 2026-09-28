// The payload types of the INSTALLATION of an uploaded program, and the
// shape guards that judge them.
//
// 🔴 EXTRACTED BEFORE THE ADDITION, like `plateforme-apps.ts` — same reason, same
// mechanism: `plateforme.ts` was at 426 lines after the first extraction,
// and sub-block G3 adds three messages, two enums, three
// encoders and their parser branches. The doctrine of `CLAUDE.md` is to
// restore the margin through an extraction played AHEAD, never through
// compression.
//
// ⚠️ THIS FILE MUST IMPORT NEITHER `node:` NOR ANY DOM: it is loaded by the
// service AND by the browser.
//
// 🔴 IT IMPORTS `PLATEFORME_VERSION` FROM `plateforme-version.ts`, AND NOT FROM
// `plateforme.ts`: the latter imports THIS module, and the resulting cycle
// would be a cycle of VALUES — not of types, which TypeScript erases —, hence a
// real runtime cycle, the kind that makes a constant `undefined` depending on
// the module evaluation order.

import { PLATEFORME_VERSION } from './plateforme-version';
import { chaineNonVide, estChaine } from './plateforme-gardes';

/**
 * Where an installation stands.
 *
 * ⚠️ THE `empreinte` PHASE IS NOT HERE, and it is not an oversight: it takes
 * place in the BROWSER, before the platform has a single row to
 * write. It never crosses the `/agent` channel.
 *
 * 🔴 `execution` CARRIES NO PERCENTAGE: a Windows installer publishes
 * none, and inventing one would lie about a progress nobody
 * measures. It carries the elapsed time, and the interface shows an
 * indeterminate state.
 */
export type Phase = 'transfert' | 'execution' | 'reconciliation';

/**
 * What an installation produced.
 *
 * 🔴 THE EXIT CODE DOES NOT ENTER THIS DECISION. `msiexec` returns 3010
 * for a success that requires a reboot, and many installers return
 * 0 after a cancellation: a product judging on the code would be wrong
 * in both directions. It is REPORTED next to the outcome, never interpreted.
 *
 * ⚠️ `refusee` is an ADDITION to the specification, which only names three.
 * Wrong fingerprint, elevation required, extension refused, process assigned to
 * a job object: these are neither successes, nor "no effect", nor
 * unknowns — they are refusals, and they carry their reason. Merging them into
 * `issue-inconnue` would read "we do not know" where we know very well.
 */
export type Issue = 'reussie' | 'sans-effet' | 'issue-inconnue' | 'refusee';

/**
 * The installation order. **The bytes never travel through it**: it carries a
 * URL, and the agent pulls the file over HTTP with its agent token. The channel
 * is JSON and carries the heartbeat; an 8 MiB slice would cost
 * +33 % in base64 there while blocking that heartbeat.
 */
export interface InstallerMessage {
    v: number;
    type: 'installer';
    installation: string;
    url: string;
    nom: string;
    taille: number;
    sha256: string;
}

/** Where an installation stands. SAMPLED — see `cadence.rs` on the agent side. */
export interface ProgressionMessage {
    v: number;
    type: 'progression';
    installation: string;
    phase: Phase;
    octets_faits: number;
    octets_total: number;
    ecoule_ms: number;
}

/**
 * The outcome, and what really happened.
 *
 * 🔴 `code_sortie` IS `number | null`, NEVER A SENTINEL `-1`: "no
 * code" and "code −1" are two different facts.
 *
 * ⚠️ AN EMPTY `journal` IS THE NORMAL CASE, not a failure: most
 * Windows installers are graphical and write nothing on the standard streams.
 */
export interface TermineMessage {
    v: number;
    type: 'termine';
    installation: string;
    issue: Issue;
    motif: string | null;
    code_sortie: number | null;
    journal: string;
    journal_tronque: boolean;
}

const PHASES: readonly Phase[] = ['transfert', 'execution', 'reconciliation'];
const ISSUES: readonly Issue[] = ['reussie', 'sans-effet', 'issue-inconnue', 'refusee'];

export function estPhase(valeur: unknown): valeur is Phase {
    return typeof valeur === 'string' && (PHASES as readonly string[]).includes(valeur);
}

export function estIssueInstallation(valeur: unknown): valeur is Issue {
    return typeof valeur === 'string' && (ISSUES as readonly string[]).includes(valeur);
}

/**
 * A count of bytes or milliseconds: integer, finite, non-negative, and below
 * `Number.MAX_SAFE_INTEGER`.
 *
 * ⚠️ `typeof x === 'number'` IS NOT ENOUGH: it lets `NaN`, `Infinity`
 * and `1.5` through. A `NaN` would travel all the way to the database, where it would become a `NULL`
 * on a `NOT NULL` column — that is an SQL error very far from its
 * cause.
 */
export function estCompte(valeur: unknown): valeur is number {
    return (
        typeof valeur === 'number' &&
        Number.isSafeInteger(valeur) &&
        valeur >= 0
    );
}

/**
 * An optional field **MANDATORY ON THE WIRE**: the key must be present,
 * its value may be `null`.
 *
 * 🔴 IT IS THE EXACT TWIN OF `champs::option_obligatoire` ON THE RUST SIDE, and without
 * it the two ends would not say the same thing. `parsed.motif` is
 * `undefined` both for "key missing" and for "key set to `undefined`":
 * only `'motif' in parsed` tells the missing field apart, and it is that
 * distinction the version bump exists to make visible. A `termine`
 * from an older version, without `motif`, must be REFUSED — not filled in with
 * a missing reason.
 */
export function presentEtNulOu<T>(
    objet: Record<string, unknown>,
    cle: string,
    garde: (valeur: unknown) => valeur is T,
): { present: true; valeur: T | null } | { present: false } {
    if (!(cle in objet)) return { present: false };
    const valeur = objet[cle];
    if (valeur === null) return { present: true, valeur: null };
    if (garde(valeur)) return { present: true, valeur };
    return { present: false };
}

export function estEntierSigne(valeur: unknown): valeur is number {
    return typeof valeur === 'number' && Number.isSafeInteger(valeur);
}

// --- The encoders and the readers, moved here from `plateforme.ts` ---
export function encodeProgression(
    installation: string,
    phase: Phase,
    octetsFaits: number,
    octetsTotal: number,
    ecouleMs: number,
): string {
    const message: ProgressionMessage = {
        type: 'progression',
        v: PLATEFORME_VERSION,
        installation,
        phase,
        octets_faits: octetsFaits,
        octets_total: octetsTotal,
        ecoule_ms: ecouleMs,
    };
    return JSON.stringify(message);
}

/**
 * ⚠️ `motif` AND `code_sortie` ARE WRITTEN EVEN AS `null`, and that is what
 * `JSON.stringify` does with a `null` — but NOT with an `undefined`, which it OMITS.
 * Passing `undefined` would produce a string without the key, which the Rust twin
 * would refuse through `option_obligatoire`. The signature thus demands `| null`, not
 * `?`, and that is the only thing that keeps the omission from being writable.
 */
export function encodeTermine(
    installation: string,
    issue: Issue,
    motif: string | null,
    codeSortie: number | null,
    journal: string,
    journalTronque: boolean,
): string {
    const message: TermineMessage = {
        type: 'termine',
        v: PLATEFORME_VERSION,
        installation,
        issue,
        motif,
        code_sortie: codeSortie,
        journal,
        journal_tronque: journalTronque,
    };
    return JSON.stringify(message);
}

export function lireProgression(parsed: Record<string, unknown>): ProgressionMessage | null {
    if (!chaineNonVide(parsed.installation)) return null;
    if (!estPhase(parsed.phase)) return null;
    if (
        !estCompte(parsed.octets_faits) ||
        !estCompte(parsed.octets_total) ||
        !estCompte(parsed.ecoule_ms)
    ) {
        return null;
    }
    return {
        type: 'progression',
        v: PLATEFORME_VERSION,
        installation: parsed.installation,
        phase: parsed.phase,
        octets_faits: parsed.octets_faits,
        octets_total: parsed.octets_total,
        ecoule_ms: parsed.ecoule_ms,
    };
}

export function lireTermine(parsed: Record<string, unknown>): TermineMessage | null {
    if (!chaineNonVide(parsed.installation)) return null;
    if (!estIssueInstallation(parsed.issue)) return null;
    // 🔴 `presentEtNulOu` RATHER THAN A VALUE TEST: the key must be
    // PRESENT, its value may be `null`. It is the exact twin of
    // `champs::option_obligatoire` on the Rust side, and without it a `termine`
    // from an older version — without `motif` — would be accepted with a
    // silently missing reason. That is the exact disguise the
    // version bump exists to prevent.
    const motif = presentEtNulOu(parsed, 'motif', estChaine);
    if (!motif.present) return null;
    const code = presentEtNulOu(parsed, 'code_sortie', estEntierSigne);
    if (!code.present) return null;
    if (!estChaine(parsed.journal)) return null;
    if (typeof parsed.journal_tronque !== 'boolean') return null;
    return {
        type: 'termine',
        v: PLATEFORME_VERSION,
        installation: parsed.installation,
        issue: parsed.issue,
        motif: motif.valeur,
        code_sortie: code.valeur,
        journal: parsed.journal,
        journal_tronque: parsed.journal_tronque,
    };
}

/**
 * The installation order, encoded HERE and nowhere else.
 *
 * ⚠️ THE FIELD ORDER IS `type` THEN `v`, as everywhere in this protocol:
 * serde emits the internal tag first, `JSON.stringify` respects insertion
 * order, and `plateforme-vectors.json` freezes the EXACT string BOTH
 * languages must produce.
 */
export function encodeInstaller(
    installation: string,
    url: string,
    nom: string,
    taille: number,
    sha256: string,
): string {
    const message: InstallerMessage = {
        type: 'installer',
        v: PLATEFORME_VERSION,
        installation,
        url,
        nom,
        taille,
        sha256,
    };
    return JSON.stringify(message);
}
