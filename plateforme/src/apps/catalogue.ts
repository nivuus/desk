// The catalogue merge: deciding, WITHOUT A DATABASE AND WITHOUT A CLOCK, what to
// write when an agent announces what it sees on its disk.
//
// 🔴 THIS MODULE IS PURE, and that is what makes its three rules testable. It
// reads no clock — the disappearance instant is written by the repository layer, which
// receives it as a parameter — and it knows no driver. The same figure as
// `agents/fraicheur.ts` and `orchestration/selection.ts`: the rule lives where
// a test can turn it red without opening an SQL engine. `catalogue.test.ts`
// checks it mechanically, by BLANKING the comments of this file before
// searching — otherwise this very sentence would be enough to make the check lie.
//
// 🔴 PAIRING IS DONE ON THE KEY, NEVER ON THE IDENTIFIER. The agent does not
// know the platform identifiers and has never seen them: it
// only announces keys, which are the digest of the triple (target, arguments,
// directory). Identifiers only leave here in the three write
// lists, to designate rows the platform already knows.
//
// 🔴 A ROW IS NEVER DELETED, only marked gone. An
// application installed on the browser side carries the identifier of its row;
// deleting and re-inserting it on reappearance would give it another one, and
// the installation would point into the void. It is for the same reason that a
// gone row that comes back is RESURRECTED rather than inserted.

import type { Application, CatalogueMessage } from '../../../proto/ts/plateforme';

/// What the platform already knows about an application of this VM.
///
/// ⚠️ THREE FIELDS, AND NOT ONE MORE. The merge needs nothing else:
/// the identity to designate the row, the key to pair it with what the agent
/// announces, and the disappearance state to tell an update from a
/// resurrection. Passing it the whole row would give it the means to
/// decide on fields the rule does not talk about.
export interface Connue {
    id: string;
    cle: string;
    /// `null` = alive. Non-null = the instant it stopped being seen.
    disparue_a: number | null;
}

/// What to write. Four lists, which the repository layer applies in the order
/// it wants: they are DISJOINT by construction on `aInserer`,
/// `aMarquerDisparues` and `aRessusciter`.
///
/// ⚠️ `aRessusciter` AND `aMettreAJour` OVERLAP DELIBERATELY: a row
/// that comes back is in both. Resurrecting resets `disparue_a` to NULL;
/// updating refreshes the name, the path and the three identity
/// fields. A resurrection alone would make visible again a row
/// with stale fields — the shortcut may have been renamed during its absence.
export interface Fusion {
    /// Whole applications: the platform does not know them yet and
    /// will assign them an identifier.
    aInserer: Application[];
    aMettreAJour: Array<{ id: string; app: Application }>;
    /// IDENTIFIERS, never keys — these are rows the
    /// platform knows, and a row we really do not want to confuse
    /// is designated by its identifier.
    aMarquerDisparues: string[];
    /// Identifiers too, for the same reason.
    aRessusciter: string[];
}

/// Merges what the platform knows with what the agent announces.
///
/// 🔴 `complet` DECIDES THE ONLY RULE THAT CAN LOSE DATA, and both
/// directions are dangerous in opposite ways:
///
///   - at `true`, EVERY known row absent from `applications` is marked
///     gone, and `disparues` is ignored. Without that, an application
///     uninstalled while the channel was down would stay in the catalogue
///     FOREVER: its key would appear in no delta, nobody having
///     seen it leave. It is this full resend at each (re)enrolment that
///     gives an END to the divergence, on a channel that guarantees no
///     delivery;
///   - at `false`, NO disappearance is invented: only the keys of
///     `disparues` are marked. Treating a delta as a complete state
///     would empty the catalogue at every message carrying only an appearance.
///
/// ⚠️ A ROW ALREADY GONE IS NOT MARKED AGAIN. `disparue_a` is set,
/// never moved: rewriting it at every round would make the column say
/// "gone thirty seconds ago" of an application gone for a
/// month, and the only possible reader of that date would be misled.
///
/// ⚠️ AN UNKNOWN KEY IN `disparues` IS IGNORED, never an exception:
/// the agent may announce the disappearance of an application the platform
/// never recorded — a single lost upstream message is enough. Raising
/// would take down the channel of an agent that is perfectly fine.
export function fusionner(connues: Connue[], message: CatalogueMessage): Fusion {
    const parCle = new Map(connues.map((c) => [c.cle, c]));
    const fusion: Fusion = {
        aInserer: [],
        aMettreAJour: [],
        aMarquerDisparues: [],
        aRessusciter: [],
    };

    const annoncees = new Set<string>();
    for (const app of message.applications) {
        annoncees.add(app.cle);
        const connue = parCle.get(app.cle);
        if (connue === undefined) {
            fusion.aInserer.push(app);
            continue;
        }
        fusion.aMettreAJour.push({ id: connue.id, app });
        if (connue.disparue_a !== null) fusion.aRessusciter.push(connue.id);
    }

    if (message.complet) {
        // ⚠️ THE ORDER IS THAT OF `connues`, and it is stable: the repository layer writes
        // in that order, and a test comparing arrays needs a
        // decided order rather than that of a `Set`.
        for (const c of connues) {
            if (annoncees.has(c.cle)) continue;
            if (c.disparue_a !== null) continue;
            fusion.aMarquerDisparues.push(c.id);
        }
        return fusion;
    }

    for (const cle of message.disparues) {
        const c = parCle.get(cle);
        // Unknown, or already gone: nothing to do, and above all no error.
        if (c === undefined || c.disparue_a !== null) continue;
        fusion.aMarquerDisparues.push(c.id);
    }
    return fusion;
}
