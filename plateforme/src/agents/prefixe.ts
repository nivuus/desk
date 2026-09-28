// The opaque session prefix, and the two pure functions that compose
// and split it.
//
// 🔴 WHY IT EXISTS: until now, the session namespace was
// LOCAL TO A VM. `agent/src/superviseur/protocole.rs` names its control
// session `bureau` — a literal constant — and `agent/src/superviseur/
// table.rs` numbers its windows `w-1`, `w-2`… through an INSTANCE counter.
// Two VMs plugged into the same platform therefore both produce
// `bureau` and both `w-1`, and fight over the same entry of the pairing
// table. The prefix makes the name global without touching the counter.
//
// 🔴 THIS MODULE IS PURE WITH ONE NAMED EXCEPTION: `nouveauPrefixe` draws from
// `randomBytes`. Everything else — `composer`, `decouper` — is a string
// function, with no clock, no database, no state.
//
// ⚠️ THE ALPHABET IS NOT A DETAIL. `base64url` (`A-Za-z0-9_-`) and not
// plain `base64`, for two reasons, one of which carries all the rest:
//   1. it does NOT contain the `:` separator, so the TURN identifier
//      `<expiry>:<prefix>:<name>` (`signaling/ice.ts`) keeps an unambiguous FIRST
//      boundary — coturn splits on the first `:` in
//      `use-auth-secret` mode. ⚠️ This last property is a reading of the
//      coturn convention, NEVER TESTED against a live coturn;
//   2. it contains neither `+` nor `/`, which would break a query string or
//      a path component the day the prefix travelled in one.

import { randomBytes } from 'node:crypto';

/// The separator between the prefix and the local session name, as
/// spec §3.4 writes it: `<prefix>:bureau`.
export const SEPARATEUR = ':';

/// 16 bytes, i.e. 128 bits — the minimum spec §3.4 demands of a
/// "non-guessable" prefix. ⚠️ NOT CALIBRATED beyond that minimum: no measurement
/// judged that more was needed, it joins the list of uncalibrated
/// constants of the repository.
export const OCTETS_PREFIXE = 16;

/// Draws a new prefix. 22 characters, without padding.
export function nouveauPrefixe(): string {
    return randomBytes(OCTETS_PREFIXE).toString('base64url');
}

/// Composes the global session name.
///
/// 🔴 AN EMPTY PREFIX RETURNS THE NAME UNCHANGED, and that is the most
/// important property of this file: it restores EXACTLY the behaviour from before
/// P3 (`bureau`, `w-1`). Putting the separator unconditionally would return
/// `:bureau`, which is the name of no existing session — and nothing, anywhere,
/// would report it. It is the class of silent failure that this whole
/// repository is written against.
export function composer(prefixe: string, nom: string): string {
    if (prefixe === '') return nom;
    return `${prefixe}${SEPARATEUR}${nom}`;
}

/// Undoes the composition. Returns an EMPTY prefix when there is none, never
/// `undefined` and never an exception: the local trial mode (spec §10) is
/// a legitimate state, not an error.
///
/// The cut is made on the FIRST separator: the prefix never contains one
/// (see the alphabet), but nothing guarantees a local name will not carry
/// one some day. Cutting on the last one would then pass the start of the name
/// off as part of the prefix.
export function decouper(session: string): { prefixe: string; nom: string } {
    const i = session.indexOf(SEPARATEUR);
    if (i === -1) return { prefixe: '', nom: session };
    return { prefixe: session.slice(0, i), nom: session.slice(i + SEPARATEUR.length) };
}
