// The session table of the signaling relay: who holds which role, and
// which offer is waiting for its recipient.
//
// This module is PURE — generic in `S`, no socket, no `ws`, no
// clock —, and that is what makes it testable without opening a single
// connection. The relay instantiates it as `Appariement<WebSocket>`.
//
// Extracted from the former `server.ts` — today `relais.ts` — by sub-block P1,
// BEFORE P2 (authentication guard), P3 (agent ↔ VM binding) and P4
// (session ownership) added anything to it. This repository established
// in D9 that an extraction done BEFORE the addition gives back its margin, and that one
// done after is paid for with a compression that `CLAUDE.md` explicitly forbids.
//
// ✅ P2 HAPPENED (19 August 2026), AND THE MARGIN WAS USED — but NOT here. This
// module did not gain a line: the guard lives in `identite/garde.ts`, the
// session ownership in `signaling/propriete.ts`, and it is `relais.ts`
// alone that grew from calling them. **The extraction therefore gave its margin back to the
// file that needed it, which is exactly what it promised.**
// ✅ P3 HAPPENED (19 August 2026), AND THE PROPERTY HOLDS A SECOND TIME: this
// module still did not gain a line. The session prefix lives in
// `agents/prefixe.ts`, the agent identity in `agents/enrolement.ts` and
// `identite/garde.ts`, the channel in `agents/canal.ts` — and `Appariement`
// knows nothing of all that: it pairs NAMES, and a prefixed name remains a
// name. **That is what makes the prefix so cheap here**; two VMs that
// both opened `bureau` stop meeting in the same entry
// without a single line of this file having moved.
//
// ✅ P4 HAPPENED (20 August 2026), AND THE PROPERTY HOLDS A THIRD TIME:
// this module still did not gain a line. Orchestration lives in
// `orchestration/`, the two new routes in `http/routes-vm.ts` and
// `http/routes-session.ts`, and neither talks to `Appariement` — the
// platform decides WHO is entitled to which VM before a single session name
// is paired. ⚠️ The sentence "P4 is still to come" that occupied this line
// was therefore WRONG from the merge of P4 on, and it is the P4 cross-cutting review
// that fixed it.


export type Role = 'agent' | 'client';

interface Session<S> {
    agent?: S;
    client?: S;
    /// Last offer received from the client, kept as long as no agent is there
    /// to take it.
    ///
    /// Sub-block D1 reverses the arrival order: the browser page opens
    /// and sends its offer BEFORE the supervisor has launched the agent of
    /// that window — it is the viewport of that page that decides the size
    /// of the virtual output, so nothing can be launched earlier. Without
    /// this memorisation, the offer would be silently lost and the session would
    /// never be established.
    offreEnAttente?: string;
}

// Type guard: needed so that TypeScript narrows `message.role` (typed
// `any`) to `Role` and allows indexing `Session` under `strict`.
export function isRole(value: unknown): value is Role {
    return value === 'agent' || value === 'client';
}

export class Appariement<S> {
    private readonly sessions = new Map<string, Session<S>>();

    /// `undefined` = accepted; otherwise the refusal reason, word for word the one
    /// the former `server.ts:119-125` produced. A peer reads it
    /// (`agent/src/signaling.rs:139`, `client/src/webrtc.ts:130-131`):
    /// changing it would be a protocol change.
    ///
    /// ❌ THESE TWO CITATIONS WERE BOTH WRONG, and one of them
    /// WAS MADE WRONG BY P3 ITSELF (cross-cutting review, 19 August 2026).
    /// They said `signaling.rs:130` and `webrtc.ts:109`: the second had
    /// drifted under P2 (l. 109 is **empty**; the real read lives at
    /// l. 130-131), the first was **right until commit `5fbc89b` of
    /// this branch**, which added the token to the handshake and pushed the
    /// line from 130 to 139. **It is the "487" wreck committed
    /// inside the very branch that denounces it.**
    ///
    /// ⚠️ The P3 plan (E11) declared `signaling.rs:130` "reread and right" —
    /// and it was **at the time the plan was written**. Rereading a citation
    /// before writing is therefore not enough: it must be reread **after having
    /// executed what moves it**.
    declarer(session: string, role: Role, socket: S): string | undefined {
        const entree = this.sessions.get(session) ?? {};
        if (entree[role]) {
            return `un ${role} est déjà connecté à la session ${session}`;
        }
        entree[role] = socket;
        this.sessions.set(session, entree);
        return undefined;
    }

    /// The socket OPPOSITE: `pair(s, 'client')` returns the agent, and vice versa.
    pair(session: string, role: Role): S | undefined {
        const entree = this.sessions.get(session);
        if (!entree) return undefined;
        return role === 'client' ? entree.agent : entree.client;
    }

    /// The last one overwrites the previous ones — a stale offer is useless,
    /// and keeping several would have no distinct recipient.
    retenirOffre(session: string, sdp: string): void {
        const entree = this.sessions.get(session) ?? {};
        entree.offreEnAttente = sdp;
        this.sessions.set(session, entree);
    }

    /// Returns the pending offer and forgets it: it must only be delivered
    /// once.
    prendreOffre(session: string): string | undefined {
        const entree = this.sessions.get(session);
        if (!entree) return undefined;
        const offre = entree.offreEnAttente;
        entree.offreEnAttente = undefined;
        return offre;
    }

    /// Removes a role, and says whether the session became empty — in which case it
    /// is forgotten, as `server.ts:190-192` did.
    retirer(session: string, role: Role): { vide: boolean } {
        const entree = this.sessions.get(session);
        if (!entree) return { vide: true };
        delete entree[role];
        const vide = !entree.agent && !entree.client;
        if (vide) this.sessions.delete(session);
        return { vide };
    }
}
