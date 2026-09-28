/**
 * The SHAPE guards of the platform <-> agent channel: what decides that a
 * value coming off the wire really is what it claims to be.
 *
 * 🔴 EXTRACTED FROM `plateforme.ts` VERBATIM (sub-block G2), BECAUSE THE
 * 500-LINE CEILING WAS CROSSED — 528 — AND THE REPOSITORY DOCTRINE IS
 * TO CATCH UP THROUGH AN EXTRACTION, NEVER THROUGH COMPRESSION.
 *
 * ⚠️ **THE EXTRACTION SHOULD HAVE PRECEDED THE ADDITION, AND IT DID NOT.**
 * The G2 plan had named three extractions to play ahead; all three were
 * played, and this one was not planned — the file was announced at 429
 * lines for "the mirror". The crossing is DECLARED rather than
 * hidden.
 *
 * ⚠️ NO LINE OF BEHAVIOUR WAS ADDED, REMOVED OR REWORDED.
 */
import type { Application, IssueLancement, SourceMax } from './plateforme';

/** The four outcomes, enumerated — see `estIssue`. */
const ISSUES: ReadonlyArray<IssueLancement> = ['raccourci', 'cible', 'inconnue', 'echec'];

/**
 * The `string` FIELDS of `Application` to validate, one by one — see
 * `estApplication`.
 *
 * 🔴 `icone` AND `source_max` ARE NOT IN IT, AND PUTTING THEM THERE WOULD BE A SILENT
 * DEFECT. This list is walked by `isString`: adding `icone` to it
 * would REFUSE ANY CATALOGUE in which a single application has no icon —
 * `null` is not a string —, with the `forme` reason, that is a whole
 * catalogue lost without any trace saying why. The two
 * new fields therefore have their own guards.
 */
const CHAMPS_APPLICATION: ReadonlyArray<keyof Application> = [
    'cle', 'nom', 'chemin', 'cible', 'arguments', 'repertoire',
];

export function estObjetJson(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function nonEmptyString(value: unknown): value is string {
    return typeof value === 'string' && value.length > 0;
}

/** ⚠️ `arguments` is LEGITIMATELY EMPTY: the guard is `string`, not `nonEmptyString`. */
export function isString(value: unknown): value is string {
    return typeof value === 'string';
}

/**
 * ⚠️ `null` IS AN EXPECTED VALUE, NOT AN ABSENCE. The guard requires the
 * key to be PRESENT — `'icone' in value` — then its value to be `null` or
 * a string. Settling for `=== null || typeof === 'string'` would accept
 * an object WITHOUT the field, `value.icone` then being `undefined`… which is
 * neither `null` nor a string, so the case would be refused by accident. Writing the
 * presence explicitly makes the property readable rather than lucky.
 */
export function estIcone(value: Record<string, unknown>): boolean {
    if (!('icone' in value)) return false;
    return value.icone === null || isString(value.icone);
}

/**
 * 🔴 AN ARBITRARY OBJECT DOES NOT PASS. `{"pixels":"gros"}` is refused, and
 * so is `{"pixels":256,"bonus":1}`: the shape is exactly one of the two
 * the Rust side knows how to emit, and nothing else.
 */
export function estSourceMax(value: unknown): value is SourceMax {
    if (value === 'non-mesuree') return true;
    if (!estObjetJson(value)) return false;
    const cles = Object.keys(value);
    if (cles.length !== 1 || cles[0] !== 'pixels') return false;
    return typeof value.pixels === 'number' && Number.isInteger(value.pixels);
}

export function estApplication(value: unknown): value is Application {
    if (!estObjetJson(value)) return false;
    if (!CHAMPS_APPLICATION.every((champ) => isString(value[champ]))) return false;
    return (
        estIcone(value)
        && estSourceMax(value.source_max)
        && estAccent(value)
        && estAssociations(value.associations)
    );
}

/**
 * ⚠️ SAME SHAPE AS `estIcone`, AND FOR THE SAME REASON: `null` is a
 * LEGITIMATE value — "no dominant colour" —, but the field must be PRESENT. A missing
 * field would be a catalogue from another version, and accepting it silently is
 * exactly what the versioning of this protocol exists to prevent.
 */
export function estAccent(value: Record<string, unknown>): boolean {
    if (!('accent' in value)) return false;
    return value.accent === null || isString(value.accent);
}

/**
 * 🔴 AN ARRAY OF STRINGS, AND EMPTY IS VALID. Refusing empty would reject
 * the vast majority of applications, which open no
 * file type.
 */
export function estAssociations(value: unknown): value is string[] {
    return Array.isArray(value) && value.every((e) => isString(e));
}

export function estIssue(value: unknown): value is IssueLancement {
    return ISSUES.includes(value as IssueLancement);
}

