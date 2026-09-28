// The registry of live agent sockets, tested WITHOUT A SERVER.
//
// 🔴 THE SOCKET IS A DOUBLE, and it is what the structural boundary of
// `SocketAgent` buys: these seven cases play out without opening a port, without
// a handshake, and without waiting for the network. What the registry
// really does with a real `ws.WebSocket` is tested elsewhere — by
// `canal-apps.test.ts`, which mounts the channel for real.
//
// 🔴 EXPIRY PLAYS OUT ON FAKE TIMERS THAT THE TEST ADVANCES.
// A frozen clock would make the case inert: it would read a final state instead
// of seeing the TRANSITION, and it is the lesson of P3's criterion ④.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { encodeLancer, type IssueLancement } from '../../../proto/ts/plateforme';
import { DELAI_LANCEMENT_MS, RegistreAgents, type SocketAgent } from './registre';

/// A socket double: it keeps what is written to it, and knows how to close.
function socketFeint(readyState = 1): SocketAgent & { ecrits: string[]; fermetures: number[] } {
    const ecrits: string[] = [];
    const fermetures: number[] = [];
    return {
        ecrits,
        fermetures,
        get readyState() {
            return readyState;
        },
        send(data: string) {
            ecrits.push(data);
        },
        close(code?: number) {
            fermetures.push(code ?? 0);
        },
    };
}

afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
});

describe('RegistreAgents', () => {
    it('returns `agent-injoignable` IMMEDIATELY for an absent VM', async () => {
        // 🔴 Waiting for `DELAI_LANCEMENT_MS` for a VM we ALREADY know
        // has no socket would make a caller pay five seconds
        // for an answer known in advance. The timers are fake and are
        // NOT advanced: if the promise waited for anything at all, it
        // would never resolve and the case would time out.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        await expect(registre.lancer('v-absente', 'cle-1', 'd-1')).resolves.toBe('agent-injoignable');
    });

    it("sends a `Lancer` ENCODED BY THE PROTOCOL, never a copied shape", async () => {
        // 🔴 Copying the message's shape here would make it diverge SILENTLY from
        // `proto/ts/plateforme.ts`, and `plateforme-vectors.json` would
        // no longer compare anything the platform puts on the wire. It is
        // the rule `agents/canal.ts` has held itself to since P3; the assertion
        // compares with the encoder itself, character for character.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        const socket = socketFeint();
        registre.inscrire('v-1', socket);

        void registre.lancer('v-1', 'cle-1', 'd-1');
        expect(socket.ecrits).toEqual([encodeLancer('d-1', 'cle-1')]);
    });

    it('matches the answer on the REQUEST, never on the key', async () => {
        // 🔴 Pairing on the key would mix up two concurrent launches of the
        // SAME application — a perfectly ordinary case, two hub tabs
        // are enough. The request is the only unique identifier of an order.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        registre.inscrire('v-1', socketFeint());

        const premier = registre.lancer('v-1', 'meme-cle', 'd-1');
        const second = registre.lancer('v-1', 'meme-cle', 'd-2');
        registre.resoudre('d-2', 'cible');
        registre.resoudre('d-1', 'raccourci');

        expect(await premier).toBe('raccourci' satisfies IssueLancement);
        expect(await second).toBe('cible' satisfies IssueLancement);
    });

    it("IGNORES an unknown request, with its trace, and never THROWS", async () => {
        // 🔴 Throwing here would be serious: the caller of `resoudre` is the
        // `message` handler of a socket, and an exception crossing
        // it — or a rejected promise leaving it — takes down THE WHOLE Node
        // PROCESS. A failure mode that `relais.ts`, `trace.ts` and
        // `canal.ts` all three document.
        //
        // An unknown request is nothing abnormal: an agent can answer
        // an order whose wait has already expired.
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        const registre = new RegistreAgents();

        expect(() => registre.resoudre('jamais-emise', 'echec')).not.toThrow();
        expect(traces.join(' | ')).toContain('jamais-emise');
    });

    it('`retirer` REJECTS the in-flight requests of this VM', async () => {
        // 🔴 Doing nothing would make the route wait `DELAI_LANCEMENT_MS`
        // after an agent death KNOWN AT THAT VERY INSTANT: the socket has just
        // closed, and we would make it wait five seconds for an answer that
        // will never come.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        registre.inscrire('v-1', socketFeint());
        const enVol = registre.lancer('v-1', 'cle-1', 'd-1');

        registre.retirer('v-1');

        await expect(enVol).resolves.toBe('agent-injoignable');
    });

    it('a SECOND registration of the same VM closes the first and replaces it', async () => {
        // 🔴 Keeping both would raise the question that
        // `0003-agents.sql` already answers for the primary key of `agent_enrole`:
        // "which one would be the right one?". The last enrolment wins, and
        // the old socket is closed rather than left floating.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        const ancien = socketFeint();
        const neuf = socketFeint();
        registre.inscrire('v-1', ancien);
        registre.inscrire('v-1', neuf);

        expect(ancien.fermetures).toEqual([1008]);
        void registre.lancer('v-1', 'cle-1', 'd-1');
        // The order leaves on the NEW one, and the old one received nothing.
        expect(neuf.ecrits).toEqual([encodeLancer('d-1', 'cle-1')]);
        expect(ancien.ecrits).toEqual([]);
    });

    it("`retirer` of a socket ALREADY replaced does not unplug the new one", async () => {
        // ⚠️ A REAL RACE, AND AN ORDINARY ONE: an agent that restarts
        // registers BEFORE the close of its old socket is
        // notified. A bare `retirer(vmId)` would then erase the registration of the
        // NEW one, and the VM would become unreachable while it has just
        // reconnected — a silent failure, until the next enrolment.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        const ancien = socketFeint();
        const neuf = socketFeint();
        registre.inscrire('v-1', ancien);
        registre.inscrire('v-1', neuf);

        registre.retirer('v-1', ancien);

        void registre.lancer('v-1', 'cle-1', 'd-1');
        expect(neuf.ecrits).toEqual([encodeLancer('d-1', 'cle-1')]);
    });

    it('returns `delai` at `DELAI_LANCEMENT_MS`, and the test ADVANCES the time', async () => {
        // 🔴 THE CASE SEES THE TRANSITION, it does not read a final state: the
        // promise is still in flight before the deadline, and resolved after. A
        // frozen clock would make it inert.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        registre.inscrire('v-1', socketFeint());
        const enVol = registre.lancer('v-1', 'cle-1', 'd-1');

        let resolue: string | undefined;
        void enVol.then((i) => {
            resolue = i;
        });

        // One millisecond BEFORE the deadline: nothing.
        await vi.advanceTimersByTimeAsync(DELAI_LANCEMENT_MS - 1);
        expect(resolue).toBeUndefined();

        await vi.advanceTimersByTimeAsync(1);
        expect(resolue).toBe('delai');
    });

    it("does not write to a socket that is no longer OPEN", async () => {
        // ⚠️ A `send` on a closing socket THROWS, and that
        // exception would cross the caller. Same guard as `envoyer` in
        // `agents/canal.ts`. The launch then returns `agent-injoignable`:
        // the VM is registered, but its socket no longer carries anything.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        const mourant = socketFeint(2 /* CLOSING */);
        registre.inscrire('v-1', mourant);

        await expect(registre.lancer('v-1', 'cle-1', 'd-1')).resolves.toBe('agent-injoignable');
        expect(mourant.ecrits).toEqual([]);
    });
});
