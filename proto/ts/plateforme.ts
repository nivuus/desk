/**
 * TypeScript mirror of `proto/src/plateforme.rs` — the platform <->
 * agent channel (`/agent`).
 *
 * ⚠️ ANY CHANGE IS MIRRORED ON BOTH SIDES, and
 * `plateforme-vectors.json` is there so that an omission shows: it carries the
 * exact strings, and both languages check them.
 *
 * ⚠️ THIS PARSER VALIDATES AT THE BOUNDARY THEN CASTS — it does NOT check each
 * field. It is the exact precedent of `parseAgentControl` (`control.ts`), and it
 * is written here so that a reader does not believe in a stronger validation
 * than it is: `v` and `type` are checked, the rest is taken as conforming
 * because the sender is the service itself, never a third party.
 */

// The constant lives in a separate module, to break the cycle of VALUES that
// the extraction of `plateforme-installation.ts` would have created. It is
// re-exported: no consumer moved.
import { PLATEFORME_VERSION } from './plateforme-version';
export { PLATEFORME_VERSION } from './plateforme-version';

/** Why the platform refuses. `enrolement` is INDISTINCT by
 * construction: telling "unknown VM" apart from "wrong secret" would be an
 * enumeration oracle. */
export type MotifCanal = 'version' | 'forme' | 'enrolement' | 'sequence';

// The payload types of ④ live in a sibling module, extracted BEFORE
// the addition of sub-block G3 so that this file does not cross 500
// lines. They are RE-EXPORTED here: none of the ten importers found had
// to move, and that is what makes the extraction a pure transposition.
// ⚠️ IMPORT **AND** RE-EXPORT, AND BOTH ARE NEEDED: an
// `export … from` re-exports without putting anything in local scope, and the
// four uses of `Application` and `IssueLancement` below would no longer
// compile. The typecheck said so, and it is better that it says so here
// than in a consuming package.
import type { Application, IssueLancement } from './plateforme-apps';
export type { Application, SourceMax, IssueLancement } from './plateforme-apps';

// The INSTALLATION types live in a sibling module, extracted for the same
// reason. Import AND re-export, for the same reason as above.
import type {
    InstallerMessage,
    ProgressionMessage,
    TermineMessage,
} from './plateforme-installation';
export type { Phase, Issue, InstallerMessage, ProgressionMessage, TermineMessage } from './plateforme-installation';
export { estPhase, estIssueInstallation } from './plateforme-installation';
export { encodeProgression, encodeTermine, encodeInstaller } from './plateforme-installation';
import { lireProgression, lireTermine } from './plateforme-installation';

// ⚠️ THESE FIVE GUARDS WERE IMPORTED HERE, AND NO LONGER ARE: they
// left with the two readers, into `plateforme-installation.ts`. The orphan
// import that remained was seen by NO typecheck of `proto/` — it is the one
// of `client/`, whose `tsconfig` is stricter, that reported it (`TS6192`).
// **Two packages typecheck the same file with two severities**, and only the
// stricter one tells the truth.


export interface EnrolerMessage { v: number; type: 'enroler'; vm: string; secret: string }
export interface BattementMessage { v: number; type: 'battement' }
/**
 * The VM catalogue, as a DIFF.
 *
 * 🔴 `complet` HAS A NAMED SEMANTICS: at `true`, the platform marks as
 * gone EVERY row of this VM missing from `applications` and ignores
 * `disparues`; at `false`, it applies the delta. The agent emits `true` at
 * every (re)enrolment, which makes the loss of an upstream message without
 * consequence — this channel is a `push` with no delivery guarantee, and without this
 * full resend a loss would leave the platform divergent WITH NO END.
 */
export interface CatalogueMessage {
    v: number;
    type: 'catalogue';
    complet: boolean;
    applications: Application[];
    /** KEYS, never objects. */
    disparues: string[];
}
export interface LanceeMessage {
    v: number;
    type: 'lancee';
    demande: string;
    issue: IssueLancement;
}

export type VersLaPlateforme =
    | EnrolerMessage
    | BattementMessage
    | CatalogueMessage
    | LanceeMessage
    | ProgressionMessage
    | TermineMessage;

export interface EnroleMessage {
    v: number;
    type: 'enrole';
    prefixe: string;
    jeton: string;
    /** In MILLISECONDS, like every timestamp of this service. */
    expire_a: number;
}
export interface BattementRecuMessage {
    v: number;
    type: 'battement-recu';
    jeton: string;
    expire_a: number;
}
/**
 * The refusal, and THE ONLY ENVELOPE OUTSIDE VERSIONING in this protocol.
 *
 * 🔴 `motif` IS A `string`, NOT A `MotifCanal`, AND THAT IS DELIBERATE
 * (fix of 20 August 2026, mirror of `proto/src/plateforme.rs`). A
 * reader must be able to read a refusal emitted by a version it does not
 * know — otherwise an outdated agent can never learn WHY it is
 * refused and loops with no end, which the G1 acceptance measured: 0 refusal
 * line, 10 retries. A reason added by a future version must therefore stay
 * readable and loggable as is.
 *
 * ⚠️ WHEN WRITING, NOTHING IS FREE: `encodeRefus` takes a `MotifCanal`. The
 * tolerance is a READING tolerance.
 *
 * 🔴 ITS SHAPE IS FROZEN — `type`, `v`, `motif`, and nothing else, ever. A
 * field added here would be rejected by older Rust readers
 * (`deny_unknown_fields`) and would on its own cancel all the tolerance.
 */
export interface RefusMessage { v: number; type: 'refus'; motif: string }
/**
 * Launch an application of the VM.
 *
 * ⚠️ THE ORDER DOES NOT CARRY THE SHORTCUT PATH, it carries the key, and the agent
 * resolves it in ITS OWN catalogue — the one it has just read from the
 * disk. The platform's copy may be one reconciliation old;
 * the agent's never is. `demande` pairs the order with its `lancee`.
 */
export interface LancerMessage { v: number; type: 'lancer'; demande: string; cle: string }

/**
 * The fingerprints the platform does NOT have, among those the last
 * `catalogue` announced.
 *
 * 🔴 IT IS NOT EMITTED WHEN THE SET IS EMPTY: an empty list would cost
 * one message per reconciliation on an idle disk, which the G1 diff
 * exists precisely to avoid. The rule lives in the caller
 * (`plateforme/src/agents/canal.ts`), which alone knows the set.
 *
 * ⚠️ THE BYTES NEVER TRAVEL THROUGH IT: this message carries only an inventory.
 * The images go through `PUT /icone/:sha256`.
 */
export interface IconesManquantesMessage {
    v: number;
    type: 'icones-manquantes';
    empreintes: string[];
}

export type DepuisLaPlateforme =
    | EnroleMessage
    | BattementRecuMessage
    | RefusMessage
    | LancerMessage
    | IconesManquantesMessage
    | InstallerMessage;

/**
 * The only `type`s this parser accepts — the PLATFORM -> AGENT direction.
 *
 * 🔴 `enroler` and `battement` are ABSENT ON PURPOSE: accepting them would let
 * a peer treat its own message as an answer, a confusion of
 * direction no version check would see.
 *
 * 🔴 THE LIST IS DERIVED FROM THE UNION, AND IT IS A STRUCTURAL REMEDY, NOT ONE
 * MORE TEST. Written by hand, it is the exact twin of `TYPES_AGENT`
 * (`control.ts`), which nothing confronts with its union and whose omission breaks
 * "neither build nor test". Here, `Record<DepuisLaPlateforme['type'], true>`
 * makes `tsc` REFUSE any variant added to the union without its key — the
 * red is the typecheck itself, and it was played.
 *
 * ⚠️ Only exhaustiveness is checked by the type; the ABSENCE of the types of the
 * reverse direction is not — an extra `enroler: true` would be a
 * `tsc` error (key outside the union), so both directions are indeed covered.
 */
const TOUS_DEPUIS: Record<DepuisLaPlateforme['type'], true> = {
    enrole: true,
    'battement-recu': true,
    refus: true,
    lancer: true,
    'icones-manquantes': true,
    installer: true,
};
const TYPES_DEPUIS = Object.keys(TOUS_DEPUIS) as DepuisLaPlateforme['type'][];

/** Exposed so the test can compare the derived list to its union. */
export function typesDepuis(): readonly DepuisLaPlateforme['type'][] {
    return TYPES_DEPUIS;
}

/**
 * ⚠️ THE FIELD ORDER IS `type` THEN `v`, AND IT IS DELIBERATE: serde emits the
 * internal tag FIRST (`plateforme.rs`), and `JSON.stringify` respects
 * insertion order. Writing `v` first — which `control.ts` does — produces
 * a string DIFFERENT from the Rust one. Nothing would break for all that (the
 * two ends parse JSON, they do not compare strings), but
 * `plateforme-vectors.json` freezes the EXACT string and checks it on both
 * sides: byte-for-byte parity is what makes this vector decidable.
 * This divergence was found by the test, not by review.
 */
export function encodeEnroler(vm: string, secret: string): string {
    const message: EnrolerMessage = { type: 'enroler', v: PLATEFORME_VERSION, vm, secret };
    return JSON.stringify(message);
}

export function encodeBattement(): string {
    const message: BattementMessage = { type: 'battement', v: PLATEFORME_VERSION };
    return JSON.stringify(message);
}

export function encodeCatalogue(
    complet: boolean,
    applications: Application[],
    disparues: string[],
): string {
    const message: CatalogueMessage = {
        type: 'catalogue',
        v: PLATEFORME_VERSION,
        complet,
        applications,
        disparues,
    };
    return JSON.stringify(message);
}

export function encodeLancee(demande: string, issue: IssueLancement): string {
    const message: LanceeMessage = { type: 'lancee', v: PLATEFORME_VERSION, demande, issue };
    return JSON.stringify(message);
}

/**
 * The only `type`s the PLATFORM's parser accepts — the
 * AGENT -> PLATFORM direction.
 *
 * 🔴 Symmetric to `TYPES_DEPUIS`, and DERIVED FROM THE UNION for exactly the same
 * reason: the answer types are ABSENT ON PURPOSE. Accepting them
 * would let the platform treat its own answer as a request.
 */
const TOUS_VERS: Record<VersLaPlateforme['type'], true> = {
    enroler: true,
    battement: true,
    catalogue: true,
    lancee: true,
    progression: true,
    termine: true,
};
const TYPES_VERS = Object.keys(TOUS_VERS) as VersLaPlateforme['type'][];

/** Exposed so the test can compare the derived list to its union. */
export function typesVers(): readonly VersLaPlateforme['type'][] {
    return TYPES_VERS;
}

/**
 * What `parseVersLaPlateforme` returns.
 *
 * 🔴 A VERDICT, NEVER AN EXCEPTION, and that is what sets it apart from its
 * twin `parseDepuisLaPlateforme`. The platform must ANSWER a typed reason
 * to the peer — `{"type":"refus","motif":…}` — before deciding what to do with the
 * socket; an exception would force it to guess the reason from an error
 * message, or to answer the same reason for all causes.
 */
export type LectureVersLaPlateforme =
    | { ok: true; message: VersLaPlateforme }
    | { ok: false; motif: MotifCanal };

// 🔴 THE SHAPE GUARDS LIVE IN A NEIGHBOURING MODULE, extracted because this
// file CROSSED 500 lines (528). See the header of `plateforme-gardes.ts`
// for the declaration of the crossing.
import {
    chaineNonVide,
    estApplication,
    estChaine,
    estIssue,
    estObjetJson,
} from './plateforme-gardes';

/**
 * Reads a message coming from the agent.
 *
 * ⚠️ THIS ONE VALIDATES EACH FIELD, WHERE ITS TWIN CASTS, and the asymmetry is
 * deliberate: the sender of a `DepuisLaPlateforme` is the service itself,
 * the sender of a `VersLaPlateforme` is a network peer. It is the ONLY
 * place in this file where the bytes come from a third party that has no
 * reason to be well behaved — a missing `vm` would otherwise travel all the way to the
 * SQL query, and the refusal coming out of it would say `enrolement`, that is
 * "wrong secret", for a message that never carried a VM.
 *
 * 🔴 THE VERSION IS CHECKED BEFORE THE TYPE. It is the envelope: a message
 * from a future version may give a known `type` a meaning we
 * do not know, and refusing it as `forme` would point the wrong cause to the peer
 * that reads the reason to decide whether it must update or fix itself.
 */
export function parseVersLaPlateforme(raw: string): LectureVersLaPlateforme {
    let parsed: unknown;
    try {
        parsed = JSON.parse(raw);
    } catch {
        return { ok: false, motif: 'forme' };
    }
    // `null` is the dangerous case: `null.type` THROWS, whereas a number or a
    // string would yield `undefined` through auto-boxing. Same guard as the relay.
    if (!estObjetJson(parsed)) return { ok: false, motif: 'forme' };

    // 🔴 STRICT, AND WITH NO DEFAULT: a `parsed.v ?? PLATEFORME_VERSION`
    // would accept a message WITHOUT a `v` field, and a `v: null` with it.
    if (parsed.v !== PLATEFORME_VERSION) return { ok: false, motif: 'version' };

    if (!TYPES_VERS.includes(parsed.type as (typeof TYPES_VERS)[number])) {
        return { ok: false, motif: 'forme' };
    }

    if (parsed.type === 'enroler') {
        if (!chaineNonVide(parsed.vm) || !chaineNonVide(parsed.secret)) {
            return { ok: false, motif: 'forme' };
        }
        return {
            ok: true,
            message: { type: 'enroler', v: PLATEFORME_VERSION, vm: parsed.vm, secret: parsed.secret },
        };
    }

    if (parsed.type === 'catalogue') {
        // ⚠️ EACH FIELD IS VALIDATED, and not only the type: it is the only
        // parser in the file whose bytes come from a third party. A missing
        // `applications` would otherwise travel all the way to the SQL query.
        if (typeof parsed.complet !== 'boolean') return { ok: false, motif: 'forme' };
        if (!Array.isArray(parsed.applications)) return { ok: false, motif: 'forme' };
        if (!Array.isArray(parsed.disparues) || !parsed.disparues.every(estChaine)) {
            return { ok: false, motif: 'forme' };
        }
        if (!parsed.applications.every(estApplication)) return { ok: false, motif: 'forme' };
        return {
            ok: true,
            message: {
                type: 'catalogue',
                v: PLATEFORME_VERSION,
                complet: parsed.complet,
                applications: parsed.applications,
                disparues: parsed.disparues,
            },
        };
    }

    if (parsed.type === 'lancee') {
        if (!chaineNonVide(parsed.demande)) return { ok: false, motif: 'forme' };
        if (!estIssue(parsed.issue)) return { ok: false, motif: 'forme' };
        return {
            ok: true,
            message: {
                type: 'lancee',
                v: PLATEFORME_VERSION,
                demande: parsed.demande,
                issue: parsed.issue,
            },
        };
    }

    // The two readers of ④-installation live in the sibling module, with the
    // types and guards they use: this file would otherwise be above
    // 500 lines. They return `null` for "shape", never an exception.
    if (parsed.type === 'progression') {
        const message = lireProgression(parsed);
        return message ? { ok: true, message } : { ok: false, motif: 'forme' };
    }

    if (parsed.type === 'termine') {
        const message = lireTermine(parsed);
        return message ? { ok: true, message } : { ok: false, motif: 'forme' };
    }

    return { ok: true, message: { type: 'battement', v: PLATEFORME_VERSION } };
}

/**
 * The three answers of the platform, encoded HERE and nowhere else.
 *
 * 🔴 THE FIELD ORDER IS `type` THEN `v`, as for the reverse direction and
 * for the same reason: serde emits the internal tag first, and
 * `plateforme-vectors.json` freezes the EXACT string BOTH languages
 * must produce. These three functions exist precisely so that the
 * platform does not have to copy the message shape into its own code —
 * a copy would diverge silently, and nothing would compare anything any more.
 */
export function encodeEnrole(prefixe: string, jeton: string, expireA: number): string {
    const message: EnroleMessage = {
        type: 'enrole',
        v: PLATEFORME_VERSION,
        prefixe,
        jeton,
        expire_a: expireA,
    };
    return JSON.stringify(message);
}

export function encodeBattementRecu(jeton: string, expireA: number): string {
    const message: BattementRecuMessage = {
        type: 'battement-recu',
        v: PLATEFORME_VERSION,
        jeton,
        expire_a: expireA,
    };
    return JSON.stringify(message);
}

export function encodeRefus(motif: MotifCanal): string {
    const message: RefusMessage = { type: 'refus', v: PLATEFORME_VERSION, motif };
    return JSON.stringify(message);
}

export function encodeLancer(demande: string, cle: string): string {
    const message: LancerMessage = { type: 'lancer', v: PLATEFORME_VERSION, demande, cle };
    return JSON.stringify(message);
}

/**
 * ⚠️ THE CALLER MUST CHECK THAT `empreintes` IS NOT EMPTY before calling:
 * this encoder does not do it for them, because it would not know what to return
 * instead.
 */
export function encodeIconesManquantes(empreintes: string[]): string {
    const message: IconesManquantesMessage = {
        type: 'icones-manquantes',
        v: PLATEFORME_VERSION,
        empreintes,
    };
    return JSON.stringify(message);
}

/**
 * Reads a message coming from the platform.
 *
 * 🔴 THE VERSION COMPARISON IS STRICT (`!==`), AND THE FIELD HAS NO
 * DEFAULT. A `parsed.v ?? PLATEFORME_VERSION` would accept a message WITHOUT
 * a `v` field, and a `v: null` message with it — that is exactly the hole that
 * `verifie_version` refuses on the Rust side, and that its comment names.
 *
 * 🔴 THE `refus` IS EXEMPT FROM IT, AND THE TYPE IS THUS CHECKED BEFORE THE VERSION
 * HERE — the exact opposite of `parseVersLaPlateforme`, which checks the version
 * first. The asymmetry is the remedy of 20 August 2026: a refusal says "I will not
 * serve you", which is understood without version negotiation, and it is the
 * ONLY message an outdated peer must be able to read. The `v` field stays
 * mandatory there and stays a number — only its VALUE is tolerated —, otherwise we
 * would reopen the hole of the missing `v` that the paragraph above closes.
 *
 * ⚠️ THIS FUNCTION IS CALLED BY NO PLATFORM CODE, which EMITS
 * `DepuisLaPlateforme` messages without ever reading any. It is the executable mirror
 * of the Rust reader, and it is `plateforme-vectors.json` (key `refus_lisibles`)
 * that forces the two to agree.
 */
export function parseDepuisLaPlateforme(raw: string): DepuisLaPlateforme {
    const parsed = JSON.parse(raw) as Partial<DepuisLaPlateforme>;
    if (!TYPES_DEPUIS.includes(parsed.type as (typeof TYPES_DEPUIS)[number])) {
        throw new Error(`type de message de plateforme inconnu : ${parsed.type}`);
    }
    if (typeof parsed.v !== 'number') {
        throw new Error(`version de plateforme absente ou non numérique : ${parsed.v}`);
    }
    if (parsed.type !== 'refus' && parsed.v !== PLATEFORME_VERSION) {
        throw new Error(`version de plateforme non supportée : ${parsed.v}`);
    }
    return parsed as DepuisLaPlateforme;
}
