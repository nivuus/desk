// Signaling server: pairs an agent and a client per session and
// relays the SDP offer and answer.
//
// ⚠️ "No persistent state" HAS NOT BEEN TRUE since sub-block P1: a
// paired session leaves a row in the database (`ObservateurDeSession` below,
// implemented by `trace.ts`). The relay itself stays without persistent state —
// it knows neither the database nor SQL —, but the SERVICE has some.
//
// 🔴 "No authentication" HAS NOT BEEN TRUE since sub-block P2, and
// has NOT BECOME FALSE EITHER — here is the exact half that remains true.
//
// A peer with the `client` role must now present a valid access token
// (`identite/garde.ts`), otherwise it is refused, logged and its socket
// closed — before any entry into the pairing table and before any sending
// of `ice-config`.
//
// ❌ WHAT FOLLOWED HERE BECAME FALSE IN SUB-BLOCK P3, and the statement is
// corrected rather than removed. It said that a peer declaring itself
// `{"role":"agent"}` was "ALWAYS ACCEPTED WITHOUT ANY IDENTITY" and
// received TURN credentials of 86 400 s — the "anonymous window",
// tolerated because the Rust agent had no identity and requiring one
// would have broken the ongoing effort D.
//
// ✅ IT IS CLOSED. The `agent` role now requires its token, of TYPE
// `agent`, and whose SUBJECT must prefix the requested session name
// (`identite/garde.ts`). The identity comes from the `/agent` channel
// (`agents/canal.ts`), which issues it against the VM's enrolment secret.
// TWO distinct tests hold it (`garde-fil.test.ts`): the refusal, and
// the absence of `ice-config` — a service that refused AFTER sending the
// ICE configuration would pass the first and let the second leak.
//
// ⚠️ What STILL holds of the original argument: listening bound to
// `PLATEFORME_HOTE` (`config.ts`) remains the first-rank defence of the
// service, and the `agent` window closing
// does not make it stop mattering.

import type { IncomingMessage } from 'node:http';
import { WebSocket, WebSocketServer } from 'ws';
import { Appariement, isRole, type Role } from './appariement';
import { messagePairPresent, prevenirLArrivant, prevenirLePairEnPlace } from './pair-present';
import type { Garde } from '../identite/garde';
import { configurationIce } from './ice';
import { adresseSource } from '../http/adresse-source';
import { ligne } from '../obs/journal';
import { BUDGET_REQUETES, cleRequetes, type Budget, type Frein } from '../securite/frein';

// Types the server relays to the peer. Everything else is refused — a relay
// that accepted anything would become an arbitrary broadcast channel.
//
// ⚠️ This sentence said "on a server without authentication" until
// sub-block P2, and that became HALF false within the branch itself: a
// `client` peer is now guarded, and thus only reaches this table authenticated.
// Bounding the TYPES still makes full sense, and for two reasons —
// it bounds what an `agent` peer can pass through — even authenticated
// since P3, it is allowed ONLY on the sessions its prefix carries, which
// says nothing about what it is entitled to relay there; and it bounds what an
// authenticated client can broadcast to another. An identity is not a permission to relay anything at all.

//
// `fenetre-ouverte`, `fenetre-fermee`, `refus` and `viewport` carry the
// control session of sub-block D1, between the supervisor (`agent` role) and
// the shell page (`client` role).
const TYPES_RELAYES = new Set([
    'offer',
    'answer',
    'fenetre-ouverte',
    'fenetre-fermee',
    'refus',
    'viewport',
]);

// Type guard: a valid JSON message can be `null`, a number, a string
// or an array (all accepted by JSON.parse), not only a
// `{role, session}` or `{type, sdp}` object. `null` is the dangerous case: unlike
// numbers/strings/arrays (whose property access simply returns
// `undefined` through auto-boxing), `null.role` throws a TypeError. As this code
// runs in a `message` event handler of a WebSocket exposed without
// authentication, an uncaught TypeError there is fatal: it takes down the whole

// Node process (no `uncaughtException` is installed in the entry
// point — RECHECKED in sub-block P1, which MOVED it: it is no longer
// `signaling/src/index.ts` but `plateforme/src/index.ts`, and it still only
// installs a `SIGINT` there), hence
// all active sessions with it. We explicitly reject anything that
// is not a plain object before accessing any property.
//
// ⚠️ "exposed without authentication" STAYS TRUE after sub-block P2, and it
// must be said WHY, otherwise a successor will think the sentence stale and
// loosen the type guard. This check runs on the FIRST message, which
// arrives BEFORE the guard could see any token: in this file,
// `isJsonObject` is called some thirty lines before `garde.verifier`.
// The handshake is therefore, at that precise instant, open to anyone who
// reaches the port — exactly as before P2.

function isJsonObject(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/// Records the connection on the "any request" budget, and logs IF
/// AND ONLY IF the brake has just bitten — same rule and same reason as
/// `http/routes-auth.ts::compterLEchec`: the next connection will be refused
/// at the very top of the `connection` handler, before ever calling this
/// function again.
///
/// ⚠️ **THIS LINE LOGS ON THE TRANSITION, AND NOT ON EVERY ADMITTED
/// CONNECTION — that is what sets it apart from a per-connection trace.** A line
/// on EVERY connection, even after the brake has started refusing, would make
/// the service write at a rate the attacker controls at no further
/// cost — the rule of the TURN work (`CLAUDE.md`): "count or
/// sample, never trace per packet". Logging on the transition
/// closes that: a hammered address writes ONE line, never one per
/// connection.
function compterLaConnexion(
    frein: Frein,
    cles: readonly (readonly [string, Budget])[],
    adresse: string,
): void {
    const instant = Date.now();
    frein.echec(cles, instant);
    const apres = frein.consulter(cles, instant);
    if (!apres.freine) return;
    console.warn(
        ligne('frein-requetes', {
            route: '/signal',
            adresse,
            retry_apres_s: apres.retryApresS,
            entrees: frein.taille(),
            evictions: frein.evictions(),
        }),
    );
}

export interface SignalingServer {
    port: number;
    close(): Promise<void>;
}

/// What the relay REPORTS about a session, without knowing anything of what is
/// done with it. It is a port, not a dependency: the production implementation
/// is `trace.ts`, which writes to the database, and the relay stays unaware of the database
/// as it was.
///
/// 🔴 Both methods are SYNCHRONOUS and return nothing, on purpose. The
/// `message` handler of a `ws` socket is synchronous, and a promise
/// rejected there takes down the whole Node process (see `isJsonObject` above). A
/// signature returning a promise would invite a caller to await it —
/// hence to make signaling depend on its own trace. The trace is an
/// OBSERVATION of signaling, never a condition of its operation.
export interface ObservateurDeSession {
    /// BOTH roles are now present on this session.
    ///
    /// `utilisateurId` is the CLIENT's when the guard established one;
    /// it is absent when the second peer to arrive is the agent. ⚠️ THE REASON
    /// CHANGED IN SUB-BLOCK P3 without the consequence moving: it is no longer
    /// that the agent has "no identity" — it has had one since the
    /// `/agent` channel —, it is that it still CLAIMS nothing, its session having to
    /// stay claimable by the human client that will join it
    /// (`identite/garde.ts`). That is what makes the word "recorded" of
    /// criterion ③ literally true in the database.
    apparie(nomSession: string, utilisateurId?: string): void;
    /// The session has emptied: no role occupies it any more.
    separe(nomSession: string): void;
}

// Two shapes, on purpose. The `port` shape is the one exercised by
// `server.test.ts` since milestone 1: keeping it intact is what allows
// saying that the move of sub-block P1 changed nothing in the relay. The
// `wss` shape is the one the service uses, where the HTTP server owns the port.
//
// 🔴 `garde` is a REQUIRED parameter, never optional, and never permissive
// by default. Three test files delivered by P1 had to change for this
// (their HARNESS, none of their assertions). The alternative — an optional
// guard meaning "accept" — would have left them green without a single line of
// change, AND would have let a miswired service authenticate NOBODY
// AT ALL without any test turning red. It is the same argument that
// `http/serveur.ts` already makes for `base`: "REQUIRED, never optional".
//
// Besides, there is no path that produces an open guard outside
// a test: the only guard factory requires a secret, and
// `PLATEFORME_SECRET_JETON` has NO default (`config.ts`).
//
// 🔴 `frein` AND `proxyDeConfiance` ARE REQUIRED, NEVER OPTIONAL — same
// argument as `garde` just above: a permissive default (no brake,
// or an open trust set) would let a miswired service bound
// NO connection without any test turning red. See
// `securite/frein.ts::BUDGET_REQUETES`: this module shares the SAME brake as
// `http/routes-vm.ts` and `http/routes-session.ts`, never a second one.
export function createSignalingServer(
    port: number,
    garde: Garde,
    frein: Frein,
    proxyDeConfiance: ReadonlySet<string>,
    trace?: ObservateurDeSession,
): SignalingServer;
export function createSignalingServer(
    wss: WebSocketServer,
    garde: Garde,
    frein: Frein,
    proxyDeConfiance: ReadonlySet<string>,
    trace?: ObservateurDeSession,
): SignalingServer;
export function createSignalingServer(
    portOuWss: number | WebSocketServer,
    garde: Garde,
    frein: Frein,
    proxyDeConfiance: ReadonlySet<string>,
    trace?: ObservateurDeSession,
): SignalingServer {
    const port = typeof portOuWss === 'number' ? portOuWss : 0;
    const wss = typeof portOuWss === 'number' ? new WebSocketServer({ port }) : portOuWss;
    const sessions = new Appariement<WebSocket>();

    function send(socket: WebSocket | undefined, payload: unknown): void {
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify(payload));
        }
    }

    wss.on('connection', (socket: WebSocket, requete?: IncomingMessage) => {
        // 🔴 THE "ANY REQUEST" BRAKE IS CONSULTED HERE, ON CONNECTION —
        // BEFORE THE FIRST MESSAGE, hence before `isJsonObject` and before
        // `garde.verifier`. A WebSocket connection is here the equivalent
        // of a request: it is what costs the pairing and, if it
        // succeeds, a database row (`ObservateurDeSession`).
        // `TRAME_MAX_OCTETS` (`http/serveur.ts`) bounds the size of a
        // message; NOTHING, before this batch, bounded the NUMBER of connections
        // one address could open on THIS path.
        //
        // 🔴 **IT IS THIS BATCH THAT CLOSES THE `/signal` HALF OF THE LEGACY THAT
        // `http/serveur.ts` named — the SOCKETS, not their SILENCE.** A
        // connection is now counted whether it sends a message or not:
        // it is the `connection` event itself that costs, not the first
        // message. See the corrected note of `TRAME_MAX_OCTETS` in
        // `http/serveur.ts`: it now distinguishes `/signal` (bounded HERE)
        // and `/agent` (`agents/canal.ts`, where the brake is STILL only consulted
        // on message — a silent peer stays uncounted there).
        //
        // ⚠️ `requete?.socket.remoteAddress` MAY BE ABSENT: the
        // `port` shape of this function (`server.test.ts` since milestone 1)
        // issues no upgrade request. `adresseSource` then returns
        // `ADRESSE_INCONNUE`, a budget SHARED by all address-less peers
        // — same behaviour as `agents/canal.ts`.
        const adresse = adresseSource(
            requete?.socket.remoteAddress,
            Array.isArray(requete?.headers['x-forwarded-for'])
                ? requete.headers['x-forwarded-for'].join(',')
                : requete?.headers['x-forwarded-for'],
            proxyDeConfiance,
        );
        // 🔴 A WRONGLY SET `PLATEFORME_PROXY_DE_CONFIANCE` MAKES THIS
        // BRAKE DEGENERATE INTO A GLOBAL BRAKE, AND ITS SEVERITY CHANGED WITH THIS BATCH — see the
        // full paragraph in `http/routes-vm.ts` (same key
        // `BUDGET_REQUETES`, same witness: the `frein-requetes` line that
        // names the retained address), never copied so as not to diverge.
        const clesRequetes: readonly (readonly [string, Budget])[] = [
            [cleRequetes(adresse), BUDGET_REQUETES],
        ];
        // `Date.now()` read here, as for `configurationIce` further down in this
        // same file: this module receives no injected clock.
        const verdictRequetes = frein.consulter(clesRequetes, Date.now());
        if (verdictRequetes.freine) {
            // ⚠️ SEND THEN CLOSE, never the reverse — same rule as on
            // a handshake refusal further down: an immediate
            // `terminate()` would truncate the message.
            //
            // 🔴 `retryApresS` IS NOW CARRIED ON THE WIRE (correction
            // round 1, criticism ②) — THE TWO BRAKED HTTP ROUTES
            // (`routes-vm.ts`, `routes-session.ts`) ALREADY SET IT, AS
            // `Retry-After`, SINCE THIS SAME BATCH; ONLY THIS WebSocket REFUSAL
            // LACKED IT. Without it, the agent cannot guess how long to
            // wait — it is the cheaper half of the remedy for the
            // lockout documented by `surveillance_pont.rs`, the other
            // half being the exponential backoff it already applies.
            send(socket, {
                type: 'error',
                reason: 'trop de requêtes',
                motif: 'trop-de-requetes',
                retryApresS: verdictRequetes.retryApresS,
            });
            socket.close(1008, 'trop-de-requetes');
            return;
        }
        compterLaConnexion(frein, clesRequetes, adresse);

        let role: Role | undefined;
        let sessionId: string | undefined;

        socket.on('message', (raw) => {
            let message: unknown;
            try {
                message = JSON.parse(raw.toString());
            } catch {
                send(socket, { type: 'error', reason: 'JSON invalide' });
                return;
            }

            // Rejection before any property read: see `isJsonObject` above.
            // The faulty peer receives an error but its connection stays open, so
            // that it can retry with a valid message.
            if (!isJsonObject(message)) {
                send(socket, {
                    type: 'error',
                    reason: role
                        ? 'message invalide : objet JSON attendu'
                        : 'premier message invalide : {role, session} attendu',
                });
                return;
            }

            // First message: role and session declaration.
            if (!role) {
                const declaredRole = message.role;
                const declaredSession = message.session;
                if (
                    !isRole(declaredRole) ||
                    typeof declaredSession !== 'string' ||
                    declaredSession.length === 0
                ) {
                    send(socket, {
                        type: 'error',
                        reason: 'premier message invalide : {role, session} attendu',
                    });
                    return;
                }

                // 🔴 THE GUARD RUNS BEFORE `declarer`, AND THE ORDER IS NOT
                // IRRELEVANT. A refused peer that had entered the pairing
                // table would occupy the role there and keep the LEGITIMATE
                // peer from arriving: a denial of service open to anyone,
                // obtained precisely by refusing to authenticate.
                const verdict = garde.verifier({
                    role: declaredRole,
                    session: declaredSession,
                    jeton: message.jeton,
                });
                if (!verdict.ok) {
                    // The log carries the session name and the requester's
                    // identifier; the message sent on the wire carries neither
                    // one nor the other (`identite/garde.ts`).
                    console.warn(`poignée de main refusée : ${verdict.journal}`);
                    // ⚠️ SEND THEN CLOSE, never the reverse: an
                    // immediate `terminate()` would truncate the message, and the
                    // peer would see a close with no reason.
                    send(socket, { type: 'error', reason: verdict.message, motif: verdict.motif });
                    // 🔴 The socket is CLOSED, whereas it stays OPEN after a
                    // malformed message (see above, deliberate since
                    // milestone 1). Spec §6: "typed refusal on the handshake,
                    // connection closed — unlike the malformed message,
                    // which the relay lets the peer retry on purpose".
                    socket.close(1008, verdict.motif);
                    return;
                }

                const refus = sessions.declarer(declaredSession, declaredRole, socket);
                if (refus) {
                    // 🔴 `motif` IS TYPED, `reason` IS A SENTENCE. Added on
                    // 31 August 2026: the hub elects a carrier tab through Web
                    // Locks, and its FALLBACK (browser without that API) must
                    // tell "the seat is taken" — to swallow silently,
                    // a second tab not being the user's fault —
                    // from a refusal with another cause, which must be shown.
                    // Deciding on `reason` would force the client to compare a
                    // FRENCH sentence, which gets reworded: that is the F1 trap,
                    // paid for with nine minutes on two messages that shared a
                    // substring.
                    //
                    // ⚠️ STRICTLY ADDITIVE: the field is added, none is
                    // removed (rule §10.2). A client from yesterday does not read `motif`
                    // and keeps reading `reason`.
                    send(socket, { type: 'error', reason: refus, motif: 'role-occupe' });
                    return;
                }

                // ONLY NOW: `declarer` has accepted. Claiming
                // earlier would leave a phantom membership behind a
                // peer refused because the role was already taken.
                garde.revendiquer(declaredSession, verdict.utilisateurId);

                role = declaredRole;
                sessionId = declaredSession;

                // THE PAIRING, and not the declaration: the opposite peer
                // exists, so both roles are there. `pair` returns the socket
                // OPPOSITE — if it is defined, this peer is the second one.
                const pairEnFace = sessions.pair(declaredSession, declaredRole);
                if (pairEnFace) {
                    trace?.apparie(declaredSession, verdict.utilisateurId);
                    // The symmetric twin of the `peer-gone` sent at the bottom of the
                    // file: the relay could say "your peer has left"
                    // and could not say "your peer has arrived". The whole
                    // reason for being — and the production measurement that
                    // forced it — lives in `pair-present.ts`, never copied
                    // here so as not to diverge.
                    if (prevenirLePairEnPlace(declaredRole)) send(pairEnFace, messagePairPresent());
                    // And the SYMMETRIC half: the agent that ARRIVES on a
                    // session where a client is already waiting. It did not exist
                    // as long as the agent opened its control session
                    // only once; it becomes the ordinary case since
                    // it REOPENS it. Full reasoning in
                    // `pair-present.ts::prevenirLArrivant`, never copied
                    // here so as not to diverge.
                    if (prevenirLArrivant(declaredRole)) send(socket, messagePairPresent());
                }

                // ICE configuration: sent to EACH peer as soon as it
                // declares itself, agent and client alike. Both need it — the
                // TURN relay is only useful if both ends can
                // use it.
                //
                // `Date.now()` is read here and not in `configurationIce`:
                // the latter thus stays a pure function, testable
                // with a fixed instant.
                const ice = configurationIce(process.env, declaredSession, Date.now());
                if (ice) {
                    send(socket, { type: 'ice-config', ...ice });
                } else {
                    // Explicit trace: a session without a relay that fails to
                    // connect from outside must be
                    // diagnosable without rereading the code.
                    console.warn(
                        'aucun serveur TURN configuré (TURN_URL/TURN_SECRET) : session sans relais',
                    );
                }

                // An offer that arrived before this agent is waiting for it: hand it over
                // now, otherwise it will never leave.
                if (declaredRole === 'agent') {
                    const offre = sessions.prendreOffre(declaredSession);
                    if (offre) send(socket, { type: 'offer', sdp: offre });
                }
                return;
            }

            // Following messages: relayed to the peer.
            const peer = sessions.pair(sessionId!, role);

            if (TYPES_RELAYES.has(message.type as string)) {
                if (message.type === 'offer' && !peer) {
                    // No agent opposite: we hold on to it, rather than lose it.
                    sessions.retenirOffre(sessionId!, message.sdp as string);
                    return;
                }
                send(peer, message);
            } else {
                send(socket, { type: 'error', reason: `type inconnu : ${message.type}` });
            }
        });

        socket.on('close', () => {
            if (!role || !sessionId) return;
            const peer = sessions.pair(sessionId, role);
            const { vide } = sessions.retirer(sessionId, role);
            send(peer, { type: 'peer-gone' });
            // The exact instant the session is forgotten from the table: that is
            // the one that closes the row, and not the departure of the first peer.
            //
            // ⚠️ `garde.liberer` is called HERE and not in `http/serveur.ts`
            // by the observer: the relay is ALSO used in its
            // `port` form, without an observer (`server.test.ts` since milestone 1).
            // Hooking the release onto the trace would mean a session name
            // would stay taken for life in that setup, with nothing
            // saying so.
            if (vide) {
                garde.liberer(sessionId);
                trace?.separe(sessionId);
            }
        });
    });

    return {
        get port(): number {
            const address = wss.address();
            return typeof address === 'object' && address ? address.port : port;
        },
        close(): Promise<void> {
            return new Promise((resolve) => {
                for (const socket of wss.clients) socket.terminate();
                wss.close(() => resolve());
            });
        },
    };
}
