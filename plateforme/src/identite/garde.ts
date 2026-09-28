// The handshake rule: who passes, who is refused, with which reason.
//
// 🔴 THIS MODULE IS PURE AND SYNCHRONOUS, and it is not a convenience detail. The
// `ws` `message` handler is synchronous (`signaling/relais.ts`), and
// `signaling/trace.ts` explains why a promise rejected there takes down the whole
// Node process. A signature returning a promise would INVITE a caller
// to await it — exactly what P1 forbade for the trace. The guard therefore
// touches neither the database nor a real clock: both are injected into it.
//
// 🔴 `verify` AND `revendiquer` ARE TWO DISTINCT CALLS, and the distinction
// is not cosmetic: the claim must only happen AFTER
// `Appariement::declarer` has accepted. Otherwise a peer refused because the role
// was already taken would leave behind a GHOST ownership, and the legitimate
// peer would be refused its own session. The two calls are in the
// same synchronous block of the relay: there is no race between them, and that is
// what allows separating them.
//
// ✅ WHAT P2 DID NOT CLOSE IS CLOSED (sub-block P3). P2 accepted here any
// peer declaring `{"role":"agent"}` WITHOUT a token, which therefore received
// TURN credentials valid for 86 400 s (`signaling/ice.ts`) without presenting the
// slightest identity — the `agent` half of the hole, which the wording of P2's criterion ①
// explicitly declared. The `agent` role now requires its token,
// exactly like the `client` role, plus two more things:
//
//   1. the token must be OF TYPE `agent` (`identite/jeton.ts`, claim `sty`) —
//      otherwise a stolen human token would open an `agent` role; and
//      conversely an agent token can NOT open a `client` role,
//      which would bypass the session ownership set by P2;
//   2. the token SUBJECT must PREFIX the requested session name — otherwise
//      an enrolled agent would occupy the session of any other VM, and
//      enrolment would only authenticate the existence of a VM, never
//      WHICH ONE.
//
// 🔴 THE GUARD DOES NOT READ `agent_enrole`, AND THAT IS STRUCTURAL, not a
// saving. It is PURE AND SYNCHRONOUS (see the warning above), and
// a database read would require an `await` on the handshake
// path. The identity was established ELSEWHERE — on the `/agent` channel, which is
// asynchronous without bothering anyone and which issues the token; the guard only
// reads it back from that token.

import { verifyToken, type TypeSujet } from './jeton';
import { SEPARATEUR } from '../agents/prefixe';
import type { ProprieteDeSession } from '../signaling/propriete';
import type { Role } from '../signaling/appariement';

export type MotifRefus = 'jeton-absent' | 'jeton-invalide' | 'jeton-expire' | 'session-refusee';

/// ⚠️ The refusal carries TWO texts, and that is deliberate: `message` goes ON THE
/// WIRE, `journal` stays with us.
///
/// The spec requires a "typed refusal, logged, with the requested identifier"
/// (criterion ③) — but telling the requester that the session belongs to someone else,
/// or even that it exists, would be an ORACLE: they would learn by trial and error
/// which session names are taken. The `journal` field therefore carries the session
/// name and the requester's identifier; `message` carries neither the one nor
/// the other. That is what separates a diagnosis from an oracle.
///
/// ⚠️ `journal` is an EXTENSION of the interface fixed by the plan (§
/// "Shared interfaces"), owned here: the alternative would have been to
/// log INSIDE the guard, which would give it an input / output side effect
/// and contradict the word "pure" of its own specification. The
/// relay writes the line (`signaling/relais.ts`).
export type Verdict =
    | { ok: true; userId?: string }
    | { ok: false; motif: MotifRefus; message: string; journal: string };

export interface Garde {
    /// WITHOUT SIDE EFFECTS: it decides, it records nothing.
    verify(poignee: { role: Role; session: string; jeton?: unknown }): Verdict;
    /// Called AFTER `Appariement::declarer` has accepted, and only then.
    revendiquer(session: string, userId: string | undefined): void;
    liberer(session: string): void;
}

/// The only text a peer refused on ownership grounds receives. It says
/// neither whom the session belongs to, nor whether it exists.
const MESSAGE_SESSION_REFUSEE = 'access refused to the requested session';

export function garde(
    secret: string,
    maintenant: () => number,
    proprietes: ProprieteDeSession,
): Garde {
    return {
        verify({ role, session, jeton }): Verdict {
            if (jeton === undefined || jeton === null || jeton === '') {
                return {
                    ok: false,
                    motif: 'jeton-absent',
                    message: 'authentication required',
                    journal: `handshake without a token on session ${session}`,
                };
            }

            const verdict = verifyToken(jeton, secret, maintenant());
            if (!verdict.ok) {
                const expire = verdict.motif === 'expire';
                return {
                    ok: false,
                    motif: expire ? 'jeton-expire' : 'jeton-invalide',
                    // The peer needs to know whether to REFRESH or to
                    // reconnect: the expired / invalid distinction is not
                    // an oracle, it is about ITS OWN token.
                    message: expire ? 'token expired' : 'invalid token',
                    journal: `token refused (${verdict.motif}) on session ${session}`,
                };
            }

            // 🔴 THE EXPECTED TYPE DEPENDS ON THE ROLE, AND BOTH DIRECTIONS ARE
            // GUARDED. Guarding one direction only would leave the other confusion
            // open, and each is serious in its own way — see the header.
            const attendu: TypeSujet = role === 'agent' ? 'agent' : 'utilisateur';
            if (verdict.type !== attendu) {
                return {
                    ok: false,
                    motif: 'session-refusee',
                    message: MESSAGE_SESSION_REFUSEE,
                    journal:
                        `session ${session} refused to ${verdict.sujet}: ` +
                        `token of type ${verdict.type} presented for the role ${role}`,
                };
            }

            if (role === 'agent') {
                // The subject of an agent token IS the prefix of its VM. The
                // comparison includes the SEPARATOR, and that is not cosmetic:
                // a bare `startsWith(sujet)` would let an agent with prefix `AB`
                // occupy the sessions of VM `ABC`, whose prefix
                // extends it — a collision that would only happen between two
                // specific VMs, hence never in testing and always in production.
                if (!session.startsWith(verdict.sujet + SEPARATEUR)) {
                    return {
                        ok: false,
                        motif: 'session-refusee',
                        message: MESSAGE_SESSION_REFUSEE,
                        journal:
                            `session ${session} refused to agent ${verdict.sujet}: ` +
                            `it does not carry its prefix`,
                    };
                }
                // ⚠️ THE AGENT STILL CLAIMS NOTHING, and `verify` therefore does
                // NOT return a `userId` here. Its session must remain
                // claimable by the human client that will join it — that is
                // what `revendiquer` documents just below, and returning it
                // would make the agent the owner of its own session, hence
                // forbid anyone from connecting to it.
                return { ok: true };
            }

            const proprietaire = proprietes.proprietaire(session);
            if (proprietaire !== undefined && proprietaire !== verdict.sujet) {
                return {
                    ok: false,
                    motif: 'session-refusee',
                    message: MESSAGE_SESSION_REFUSEE,
                    journal: `session ${session} refused to ${verdict.sujet}: it belongs to another user`,
                };
            }

            return { ok: true, userId: verdict.sujet };
        },

        revendiquer(session, userId): void {
            // An `agent` now has an identity (P3), but it STILL claims
            // nothing: its session must remain claimable by the human
            // client that will join it. `verify` returns no `userId`
            // for the `agent` role, and that is what lets this path through.
            if (userId === undefined) return;
            proprietes.revendiquer(session, userId);
        },

        liberer(session): void {
            proprietes.liberer(session);
        },
    };
}
