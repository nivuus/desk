// The database trace of a paired session: a row opened when BOTH
// roles are present, closed when the session empties.
//
// 🔴 WHEN, exactly, and it is not obvious. The relay knows two
// instants: the DECLARATION of a peer, and the PAIRING of the second. A single peer
// is not a pairing — the supervisor declares itself `agent` on `bureau` at
// VM startup and can stay there alone for hours
// (`agent/src/superviseur/protocole.rs`). The row therefore opens at the SECOND role,
// and closes when `Appariement::retirer` returns `{ vide: true }`.
//
// ⚠️ ACCEPTED CONSEQUENCE: an agent that declares itself and leaves without ever
// meeting a client LEAVES NO TRACE. It is a decision, not an
// oversight.
//
// ✅ THIS DECISION ANNOUNCED ITS OWN REOPENING — "the day we want to
// observe the agents present, which is the topic of P3 (`vu_a`)" —, AND
// P3 HAPPENED WITHOUT REOPENING IT (19 August 2026, cross-cutting end-of-branch
// review). Observing agents does NOT go through the session trace: it is
// `agent_enrole.vu_a`, advanced by the heartbeat of the `/agent` channel
// (`agents/canal.ts`), and judged by `agents/fraicheur.ts`. An agent that
// declares itself and leaves is therefore indeed seen there — simply elsewhere, and by a
// mechanism that depends on no pairing.
//
// **The forecast was right about the NEED and wrong about the PLACE**, and it is the
// most frequent form of stale forecast in this repository: this module
// had nothing to change.
//
// 🔴 THE WRITE MUST NEVER BE ABLE TO KILL A SESSION. `ouvrirSession` is
// asynchronous, the relay's `message` handler is synchronous, and a
// promise rejected without `catch` in a `ws` event handler brings down the whole
// Node process — that is exactly the failure mode that the comment
// of `isJsonObject` (`relais.ts`) describes. The write is therefore launched WITHOUT being
// awaited, with a `.catch` that logs and interrupts nothing.
//
// The cost is named: a lost write only shows in the log. That is
// why `trace.test.ts` waits for the row with a BOUND and fails on
// timeout, rather than settling for "the row ends up existing".
//
// 🔴 THIS IS WHERE `session.vm_id` IS RESOLVED, AND NOT IN THE RELAY (E10 of the
// P3 plan). `ObservateurDeSession.apparie` is SYNCHRONOUS and returns nothing, on
// purpose (`relais.ts`): a resolution living over there would force the
// relay to know the database, which it does not know and must not
// know. The trace, for its part, already knows it — it is its sole reason for being.
//
// The session name CARRIES the VM: `<prefix>:bureau` (`agents/prefixe.ts`).
// We split it, look up the prefix in `agent_enrole`, and record
// the identifier found. TWO cases return `null`, and neither one is
// an error:
//   - NO PREFIX (plain `bureau`): it is the local trial mode that
//     spec §10 sets as legitimate, and it is not logged — it is
//     nominal, not abnormal;
//   - UNKNOWN PREFIX: the VM is not (or is no longer) enrolled. That one IS
//     LOGGED, because it is abnormal and would otherwise be
//     indistinguishable from the previous one.
//
// ⚠️ RECORDING ANYWAY WOULD MAKE THE COLUMN LIE: it would name a VM
// the database does not know. `session.vm_id` stays NULLABLE for this very
// reason — `NOT NULL` would be WRONG there, not merely costly.
//
// The clock is a PARAMETER, never read here: same rule as `depot/session.ts`
// and `src/signaling/ice.ts`, and it is what makes the instants assertable on
// exact values.

import type { Pilote } from '../base/pilote';
import { decouper } from '../agents/prefixe';
import { lireParPrefixe } from '../depot/agent';
import { clore, ouvrirSession } from '../depot/session';
import type { ObservateurDeSession } from './relais';

/// Reason set when both peers left on their own — to be told apart from
/// `MOTIF_BALAYAGE`, which marks a row that an abrupt stop left open.
///
/// Passed as a query PARAMETER, never written into the SQL: a literal
/// value would make `rendreMarqueurs` throw on the Postgres side.
export const MOTIF_DEPART = 'les deux pairs sont partis';

/// Resolves the VM that the session name designates, or `undefined`.
///
/// ⚠️ IT DOES NOT THROW ON AN UNKNOWN PREFIX: it is a legitimate state from the
/// trace's point of view, which observes and arbitrates nothing. An exception here
/// would climb into the caller's `.catch` and lose the WHOLE
/// ROW — we would have traded a `null` column for no trace at all.
///
/// It does, however, let a DATABASE failure climb up, which is an entirely
/// different event: the caller's `.catch` logs it, and the missing
/// row is then the right symptom.
async function resoudreVm(base: Pilote, nomSession: string): Promise<string | undefined> {
    const { prefixe } = decouper(nomSession);
    // The local trial mode. Nominal, therefore silent: logging it would drown the
    // abnormal case below under one line per session.
    if (prefixe === '') return undefined;

    const ligne = await lireParPrefixe(base, prefixe);
    if (ligne === undefined) {
        // ⚠️ THE PREFIX IS IN THE LINE, and it is not an oracle: this
        // trace stays WITH US, it leaves on no wire. It is the same
        // split as `identite/garde.ts` between `message` and `journal`.
        console.warn(
            `session ${nomSession} : préfixe ${prefixe} inconnu de agent_enrole, vm_id non inscrit`,
        );
        return undefined;
    }
    return ligne.vm_id;
}

export function observateurDeSession(
    base: Pilote,
    horloge: () => number,
): ObservateurDeSession {
    // The PROMISE, and not the identifier: `separe` can arrive before
    // the INSERT has completed (two peers that close right after pairing).
    // Holding the promise and chaining on it closes this race without a lock.
    const ouvertes = new Map<string, Promise<string | undefined>>();

    return {
        apparie(nomSession, userId) {
            // An already traced session does not open a second row: a peer
            // that reconnects while the other stays in place re-pairs the
            // session, and the first row would otherwise stay orphaned — never
            // closed, until the sweep at the next startup.
            if (ouvertes.has(nomSession)) return;
            // 🔴 THE INSTANT IS TAKEN **BEFORE** THE RESOLUTION, never after:
            // `resoudreVm` reads the database, so its execution time is
            // unknown, and `ouverte_a` must date the PAIRING — not the end
            // of a query. Reading the clock in the call to `ouvrirSession`
            // would drift the instant by that much.
            const instant = horloge();
            ouvertes.set(
                nomSession,
                // The user identifier comes from the guard's verdict,
                // passed on by the relay. It is absent when the second peer to
                // arrive is the agent — whose identity exists since P3, but
                // which still claims nothing (`identite/garde.ts`).
                resoudreVm(base, nomSession)
                    .then((vmId) => ouvrirSession(base, nomSession, instant, userId, vmId))
                    .catch((cause) => {
                        console.error(
                            `trace de session non écrite pour ${nomSession} : ${String(cause)}`,
                        );
                        return undefined;
                    }),
            );
        },

        separe(nomSession) {
            const attendue = ouvertes.get(nomSession);
            // Nothing to close: the session was never paired. It is not
            // an anomaly — see the header.
            if (!attendue) return;
            ouvertes.delete(nomSession);
            void attendue
                .then((id) => (id ? clore(base, id, horloge(), MOTIF_DEPART) : undefined))
                .catch((cause) => {
                    console.error(
                        `trace de session non close pour ${nomSession} : ${String(cause)}`,
                    );
                });
        },
    };
}
