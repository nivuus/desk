// The `session` repository: open a row on pairing, close it when
// both peers leave, sweep those that an abrupt stop left open.
//
// 🔴 THE CLOCK IS A PARAMETER, never read here. It is the rule of the
// portable subset (spec §3.2: timestamps are "always written
// by the application") AND the precedent of the repository: `src/signaling/ice.ts:32-42`
// already takes `maintenant` as a parameter for the same reason. What makes the
// choice checkable rather than declarative: `session.test.ts` asserts
// EXACT VALUES, which a hidden `Date.now()` would make all fail.
//
// ⚠️ WHAT THIS TABLE IS NOT, and it must be read before relying on it: a
// `session` row is a trace of the PAIRING, not a state of truth of the
// media. The WebRTC stream no longer depends on signaling once the offer and the
// answer are exchanged (`agent/src/demarrage.rs` logs it explicitly):
// a session can be ALIVE while the startup sweep has just
// closed its row. The sweep therefore LIES about the sessions that really
// survived. It is a limit of P1, named, not a defect to fix here.

import { randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';

export interface LigneSession {
    id: string;
    nom_session: string;
    utilisateur_id: string | null; // policy: allow-fr - frozen wire key or SQLite column
    vm_id: string | null;
    ouverte_a: number;
    fermee_a: number | null;
    motif: string | null;
}

/// Reason set on the rows that an abrupt stop left open.
///
/// It is passed as a PARAMETER of the query, never written into the SQL: a
/// literal value would make `rendreMarqueurs` throw on the Postgres side.
export const MOTIF_BALAYAGE = 'platform restarted';

/// Opens a row and returns its identifier.
///
/// The identifier is a v4 UUID, never the session name: the latter is
/// NOT unique over time — `bureau` comes back at every agent startup.
///
/// ⚠️ `userId` is OPTIONAL, and it must stay so. Making it required
/// would break the P1 callers, and above all it does not always exist: a
/// session paired by an `agent` peer alone — the `bureau` control
/// session at the startup of a VM — has nobody to record. ⚠️ THE REASON
/// CHANGED IN SUB-BLOCK P3, the consequence did not: the agent now has an
/// identity (the `/agent` channel hands it out), but it still CLAIMS
/// nothing — its session must stay claimable by the human client
/// who will join it (`identite/garde.ts`). The column is therefore born NULL,
/// exactly as P1 wrote it.
///
/// It is this argument that makes the word "recorded" of criterion ③
/// literally true: the DECISION is made by the in-memory registry
/// (`signaling/propriete.ts`), the durable RECORDING happens here, and it is
/// what P4 will need.
///
/// ✅ P4 DID NEED IT: `compterOuvertesDe`, further down in this file, is
/// its production reader, and `GET /vm` returns `sessions_ouvertes` from it. The
/// future tense of the sentence above has been past since 20 August 2026.
///
/// ⚠️ `vmId` is OPTIONAL FOR THE SAME REASON, and it comes AFTER
/// `userId` so as to move no existing caller. It is the trace
/// (`signaling/trace.ts`) that resolves it, by cutting the prefix off the
/// session name then looking it up in `agent_enrole`. A `bureau` session
/// WITHOUT a prefix — the local trial mode that spec §10 sets as legitimate —
/// has no honest VM to record, and a session with an UNKNOWN prefix neither:
/// in both cases the column stays `null`, never an empty string
/// that would lie about what we know.
export async function ouvrirSession(
    p: Pilote,
    nomSession: string,
    maintenant: number,
    userId?: string,
    vmId?: string,
): Promise<string> {
    const id = randomUUID();
    // The columns are ALWAYS named, and their values ALWAYS passed as a
    // parameter — `null` included. Writing two queries depending on the presence of
    // the identifier would make one of them diverge the day the table changed.
    await p.executer(
        'INSERT INTO session(id, nom_session, utilisateur_id, vm_id, ouverte_a) VALUES(?, ?, ?, ?, ?)',
        [id, nomSession, userId ?? null, vmId ?? null, maintenant],
    );
    return id;
}

/// Closes a row. The `fermee_a IS NULL` clause makes the call IDEMPOTENT: a
/// second closing moves neither the instant nor the reason of the first — without
/// it, a late disconnection would rewrite an already correct trace.
export async function clore(
    p: Pilote,
    id: string,
    maintenant: number,
    motif: string | null,
): Promise<void> {
    await p.executer(
        'UPDATE session SET fermee_a = ?, motif = ? WHERE id = ? AND fermee_a IS NULL',
        [maintenant, motif, id],
    );
}

/// Closes all the rows left open and returns their NUMBER.
///
/// See the header warning: this sweep lies about the sessions that
/// really survived the service stop.
export async function balayerLesOuvertes(p: Pilote, maintenant: number): Promise<number> {
    const r = await p.executer(
        'UPDATE session SET fermee_a = ?, motif = ? WHERE fermee_a IS NULL',
        [maintenant, MOTIF_BALAYAGE],
    );
    return r.lignes;
}

/// All the rows carrying this session name, from the oldest to the most
/// recent. There can be several: the name is not an identity.
export async function lireParNom(p: Pilote, nomSession: string): Promise<LigneSession[]> {
    return p.interroger<LigneSession>(
        'SELECT id, nom_session, utilisateur_id, vm_id, ouverte_a, fermee_a, motif FROM session WHERE nom_session = ? ORDER BY ouverte_a',
        [nomSession],
    );
}

/// How many sessions of this user are OPEN.
///
/// 🔴 IT IS THE FIRST PRODUCTION READER OF `session.utilisateur_id`. The (policy: allow-fr - frozen wire key or SQLite column)
/// column has been written since P2 by the chain `identite/garde.ts` →
/// `signaling/relais.ts` → `signaling/trace.ts` → `ouvrirSession` above, and
/// the only `SELECT` that brought it back was `lireParNom`, none of whose callers
/// is production code. It is legacy item no. 4 of P2 / no. 3 of P3.
///
/// ⚠️ WHAT THIS COUNT DOES NOT ESTABLISH, and that is why the field the route
/// derives from it is called `sessions_ouvertes` and not `sessions_actives`: it counts
/// unclosed ROWS, never live media sessions. The header warning
/// of this file states the gap in both directions — the media survives the
/// restart of the service while `balayerLesOuvertes` has closed its row, and
/// a row can stay open for a peer that left without its disconnection
/// having been seen. The name carries the caveat; do not rename it without lifting it.
///
/// ⚠️ A ROW WITH A NULL `utilisateur_id` IS COUNTED FOR NOBODY. It is the (policy: allow-fr - frozen wire key or SQLite column)
/// NOMINAL case of a control session paired by the agent alone
/// (`identite/garde.ts`, and the comment of `ouvrirSession` above):
/// SQL equality with `NULL` never yields true, and this property is held
/// by a test rather than left to the semantics of the engine.
export async function compterOuvertesDe(p: Pilote, userId: string): Promise<number> {
    const lignes = await p.interroger<{ n: number }>(
        'SELECT COUNT(*) AS n FROM session WHERE utilisateur_id = ? AND fermee_a IS NULL',
        [userId],
    );
    // ⚠️ NO `Number(...)` HERE, DELIBERATELY. A `COUNT(*)` is an `int8` on
    // Postgres, which `pg` would return as a STRING without the `setTypeParser` of
    // `base/pilote-postgres.ts` — measured through the service driver:
    // `[{"n":1}] typeof = number`. Wrapping it in a `Number()` would make the
    // count right AND would mask the disappearance of the parser, on which
    // seven columns elsewhere depend. The test therefore asserts the TYPE, not just the
    // value, and it is this assertion that holds the driver remedy.
    return lignes[0].n;
}
