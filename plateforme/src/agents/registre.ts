// The registry of LIVE agent sockets, and the launch requests in flight.
//
// 🔴 WHY IT EXISTS. `agents/canal.ts` keeps its state in the CLOSURE of
// the connection (`vmId`, `prefixe`): before this module, there was no
// way to find the socket of a given VM. `POST /application/:id/lancer`
// needs it — the order leaves from the HTTP route and the answer comes back through the
// socket, two paths that nothing linked.
//
// 🔴 IT IS A SERVICE OBJECT, on the exact pattern of `ProprieteDeSession`
// (`signaling/propriete.ts`): built ONCE in `startServer`, passed
// to its two consumers, and alive for the duration of the process.
//
// ⚠️ IT HAS THE SAME COST AS `ProprieteDeSession`, AND IT IS WRITTEN HERE FOR THE
// SAME REASON: IT DOES NOT SURVIVE A RESTART. After a restart, no
// agent is in it until it has re-enrolled, and every launch returns
// `agent-injoignable` — loudly, never silently. This cannot be caught up
// here: a socket lives in one process and one only. The real catch-up is the
// reconnection of the agent, with exponential backoff, which rebuilds the registry
// without anyone having to persist it.
//
// ⚠️ NO PROMISE IS LEFT WITHOUT AN OUTCOME. Every request registered here
// ends up resolved: by the agent's answer, by the death of its socket, or by
// the deadline. A promise no path resolves would hold an HTTP request
// open until the end of the world.

import type { IssueLancement } from '../../../proto/ts/plateforme';
import { encodeLancer } from '../../../proto/ts/plateforme';

/// The delay after which an unanswered order is abandoned.
///
/// ⚠️ NOT CALIBRATED. It is a rough upper bound: a `ShellExecuteExW` returns
/// without waiting for the application to be visible, so the agent answers in
/// a few milliseconds in the nominal case. Five seconds cover a loaded
/// VM without making a human wait forever. No measurement
/// grounds it, and saying so is more honest than presenting it as tuned.
export const DELAI_LANCEMENT_MS = 5_000;

/// The WebSocket close code 1008, "policy violation" — the same
/// that `agents/canal.ts` uses on a refusal. A peer that reads both
/// channels does not have to know two conventions.
const FERMETURE_POLITIQUE = 1008;

/// What the registry demands of a socket, and NOTHING MORE.
///
/// 🔴 A STRUCTURAL BOUNDARY, NOT A `ws` TYPE. A `ws.WebSocket`
/// satisfies it without declaring anything, and `registre.test.ts` can pass a double
/// — which makes the seven registry cases playable without opening a port. It is
/// the same figure as the clock as a parameter: the dependency is named instead
/// of being imported.
export interface SocketAgent {
    /// 1 = OPEN, in the `ws` convention as in the browser one.
    readonly readyState: number;
    send(data: string): void;
    close(code?: number, raison?: string): void;
}

/// What `lancer` can return, on top of the protocol outcomes.
///
/// ⚠️ `agent-injoignable` AND `delai` ARE NOT `IssueLancement`s, and must
/// never become ones: an `IssueLancement` is what the AGENT did,
/// and these two say the agent said nothing at all. Mixing them up would
/// answer "launch failed" where the truth is "we do not know".
export type EchecLancement = 'agent-injoignable' | 'delai';

interface EnVol {
    vmId: string;
    resoudre(issue: IssueLancement | EchecLancement): void;
}

export class RegistreAgents {
    /// The VM and its current socket. At most ONE per VM.
    private readonly sockets = new Map<string, SocketAgent>();
    /// The emitted orders whose answer has not arrived yet, by request.
    private readonly enVol = new Map<string, EnVol>();

    /// Registers the socket of a VM. THE LAST ONE WINS.
    ///
    /// 🔴 THE OLD ONE IS CLOSED, never left floating. Two sockets for the
    /// same VM would raise the question that the primary key
    /// of `agent_enrole` already answers (`0003-agents.sql`): "which one would be the
    /// right one?". The case is ordinary — a restarted agent reconnects before
    /// the close of its old socket is notified.
    inscrire(vmId: string, socket: SocketAgent): void {
        const ancien = this.sockets.get(vmId);
        this.sockets.set(vmId, socket);
        if (ancien !== undefined && ancien !== socket) {
            ancien.close(FERMETURE_POLITIQUE, 'remplace');
        }
    }

    /// Pushes a raw message to the agent of a VM, and says whether it went out.
    ///
    /// 🔴 THIS REGISTRY IS THE ONLY ONE THAT KNOWS THE SOCKETS, hence the only one
    /// able to answer "is this VM reachable AT THIS INSTANT". Exposing
    /// the `Map` instead would have let each caller redo the
    /// `readyState` test, and one of them would have forgotten.
    ///
    /// ⚠️ **`false` IS NOT AN ERROR**: it says "no open socket for
    /// this VM", which is the ordinary state of a VM that is off. The caller decides
    /// what to do with it — and for the installation order, the answer is "nothing":
    /// the row stays `en_attente` in the database, and `reemettreLesInstallations` will
    /// deliver it at the next enrolment. That is the safety net that already existed.
    pousser(vmId: string, brut: string): boolean {
        const socket = this.sockets.get(vmId);
        if (socket === undefined || socket.readyState !== 1) return false;
        socket.send(brut);
        return true;
    }

    /// Removes a VM and REJECTS its requests in flight.
    ///
    /// 🔴 THE REJECTION IS NOT A CONVENIENCE. Without it, the route would wait
    /// `DELAI_LANCEMENT_MS` after an agent death known at that very instant —
    /// five seconds for an answer we already know will never
    /// arrive.
    ///
    /// ⚠️ `socket` IS OPTIONAL AND IT MATTERS: when passed, the removal only happens
    /// if it is indeed THIS socket that is registered. An agent that restarts
    /// registers BEFORE the close of the previous one is notified; a
    /// bare removal would then erase the registration of the NEW one, and the VM would become
    /// unreachable although it has just reconnected.
    retirer(vmId: string, socket?: SocketAgent): void {
        const current = this.sockets.get(vmId);
        if (socket !== undefined && current !== socket) return;
        this.sockets.delete(vmId);
        for (const [demande, attente] of [...this.enVol]) {
            if (attente.vmId !== vmId) continue;
            this.enVol.delete(demande);
            attente.resoudre('agent-injoignable');
        }
    }

    /// Emits a launch order and waits for its outcome.
    ///
    /// 🔴 THE MESSAGE IS ENCODED BY `proto/ts/plateforme.ts`, never copied
    /// here: a copy would diverge silently, and `plateforme-vectors.json` would no longer
    /// compare anything the platform really puts on the wire.
    /// It is the rule `agents/canal.ts` has imposed on itself since P3.
    ///
    /// ⚠️ PAIRING IS DONE ON THE REQUEST, NEVER ON THE KEY. Two
    /// concurrent launches of the same application are ordinary — two
    /// hub tabs are enough — and pairing on the key would mix them up.
    lancer(
        vmId: string,
        cle: string,
        demande: string,
    ): Promise<IssueLancement | EchecLancement> {
        const socket = this.sockets.get(vmId);
        // Immediate, and without a timer: waiting for an answer known
        // in advance would be paying five seconds for nothing.
        if (socket === undefined || socket.readyState !== 1 /* OPEN */) {
            return Promise.resolve('agent-injoignable');
        }

        return new Promise((resoudre) => {
            const minuteur = setTimeout(() => {
                this.enVol.delete(demande);
                resoudre('delai');
            }, DELAI_LANCEMENT_MS);
            // ⚠️ `unref` is NOT called: a detached timer would let the
            // process exit with an HTTP request pending. It is on the other hand
            // ALWAYS cancelled, on the two other exit paths.
            this.enVol.set(demande, {
                vmId,
                resoudre: (issue) => {
                    clearTimeout(minuteur);
                    resoudre(issue);
                },
            });
            socket.send(encodeLancer(demande, cle));
        });
    }

    /// Delivers the outcome the agent reported.
    ///
    /// 🔴 AN UNKNOWN REQUEST IS IGNORED WITH ITS TRACE, NEVER AN
    /// EXCEPTION. Its caller is the `message` handler of a socket, and
    /// an exception crossing it takes down THE WHOLE Node PROCESS — a failure
    /// mode that `relais.ts`, `trace.ts` and `canal.ts` all three
    /// document. The case is moreover nothing abnormal: an agent can
    /// answer an order whose wait has already expired.
    resoudre(demande: string, issue: IssueLancement): void {
        const attente = this.enVol.get(demande);
        if (attente === undefined) {
            console.warn(
                `lancement sans attente : la demande ${demande} a rendu ${issue}, `
                    + `mais plus personne ne l'attendait (expiration, ou agent remplacé)`,
            );
            return;
        }
        this.enVol.delete(demande);
        attente.resoudre(issue);
    }
}
