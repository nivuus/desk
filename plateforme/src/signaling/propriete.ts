// The session ownership registry: who is entitled to join which name.
//
// This module is PURE — a `Map`, no socket, no clock, no database —,
// on the exact pattern of `appariement.ts`, and for the same reason: that is what
// makes it testable without opening a single connection.
//
// 🔴 WHY THE DECISION IS IN MEMORY AND NOT IN THE DATABASE. The `ws`
// `message` handler is SYNCHRONOUS (`relais.ts`), and `trace.ts` explains in
// so many words why a promise rejected there takes down the whole Node process —
// that is the reason P1 writes its trace without awaiting it. Querying
// the database to decide whether to accept a peer would bring back exactly what P1
// deliberately avoided.
//
// ⚠️ THE COST, named and NOT fixed: this registry DOES NOT SURVIVE a
// restart of the service. After a restart, a freed session name can
// be claimed by another user. It cannot be caught up here: the
// spec §3.1 says a WebSocket lives in one process and one only, and its §6
// says media sessions SURVIVE the restart — so the routing state
// is lost while the media goes on. The real answer is the OPAQUE
// PREFIX of P3 (spec §3.4), which makes a session name unguessable.
//
// ✅ THIS PREFIX HAS EXISTED SINCE 19 AUGUST 2026 (sub-block P3), and the statement
// above stops being a prediction: `agents/prefixe.ts` draws 128 bits from
// `randomBytes`, and `agent_enrole.prefixe_session` makes them DURABLE —
// unlike this registry, the prefix therefore survives a restart of the
// service, which is exactly what was missing.
//
// ⚠️ WHAT THE PREFIX DOES NOT REPAIR, and it must be said so as not to read the
// line above as a closure: it is PER VM, not per session. After a
// restart, two human clients of the SAME VM get the same prefix back,
// and the in-memory registry that decided which one owns `<préfixe>:w-1` is
// still lost. The prefix closes guessability between VMs; it does not
// close the claiming of a session within a VM. The cost named
// above therefore stays OPEN, reduced and not removed.
//
// The durable RECORD, for its part, does exist: `session.utilisateur_id` is
// filled in at pairing by the trace (task 13). That is what makes the word
// "recorded" of criterion ③ literally true, and it is what P4 will
// need. The DECISION and the RECORD are two distinct layers.
//
// ✅ P4 NEEDED IT, AND READS IT (20 August 2026): `depot/session.ts::
// compterOuvertesDe` counts the non-closed rows of a user, and
// `GET /vm` returns it as the `sessions_ouvertes` field per VM. The future "will
// need" is therefore past. ⚠️ BUT THIS READER DOES NOT SAY WHAT ONE MIGHT
// MAKE IT SAY: it counts open ROWS, not live media
// sessions — the media survives a service restart while the row
// is closed by the sweep, and an open row can correspond to a peer
// that left without the disconnection being seen. The field is called
// `sessions_ouvertes` and not `sessions_actives`: the NAME carries the caveat.
//
// ⚠️ AND THE PARAGRAPH ABOVE — the prefix PER VM, this IN-MEMORY registry —
// is NOT settled by P4, which only wired the SOURCE of the prefix
// (`client/src/connexion.ts` -> `poserPrefixe`). Its SCOPE has not changed:
// two human clients of the same VM still get the same prefix back after
// a service restart, and `<préfixe>:w-1` becomes claimable again.

export class ProprieteDeSession {
    private readonly proprietaires = new Map<string, string>();

    /// The owner's identifier, or `undefined` if the session is free.
    proprietaire(session: string): string | undefined {
        return this.proprietaires.get(session);
    }

    /// Claims, or renews. The same user may claim twice
    /// without error: a reconnection would otherwise break their own session.
    revendiquer(session: string, utilisateurId: string): void {
        this.proprietaires.set(session, utilisateurId);
    }

    /// Hands the session back to whoever asks for it next. Never releasing would lose
    /// a session name for good.
    liberer(session: string): void {
        this.proprietaires.delete(session);
    }
}
