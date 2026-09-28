// The "your peer has arrived" message: that it leaves, to whom, and to whom NOT.
//
// 🔴 THIS FILE IS THE WITNESS OF `nextMessage` (`server.test.ts`,
// `resilience.test.ts`), WHICH FILTERS `pair-present` SINCE THIS BATCH. Without a
// measurement establishing that the message IS emitted, this filter would be indistinguishable
// from sweeping under the rug: the four tests it repairs would pass just
// as well if the relay no longer sent anything at all.
//
// 🔴 WHAT IT DEFENDS: without this message, a supervisor that announced its
// windows BEFORE the shell page arrived never learns it must
// say them again, and the user finds an empty desktop on a VM full of
// windows — the defect measured in production on 30 August 2026 (see
// `pair-present.ts`).

import { afterEach, beforeAll, afterAll, beforeEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import type { Garde } from '../identite/garde';
import { Frein } from '../securite/frein';
import { createSignalingServer } from './relais';
import { poserTurnAmbiant } from './turn-harnais';
import { prevenirLArrivant, prevenirLePairEnPlace, TYPE_PAIR_PRESENT } from './pair-present';

/// Local to this file, never exported by production code — same
/// argument and same shape as `server.test.ts`: these tests test the
/// RELAY, not authentication, which has its own file.
const GARDE_OUVERTE: Garde = {
    verify: () => ({ ok: true }),
    revendiquer: () => {},
    liberer: () => {},
};

/// A relay WITHOUT TURN: `ice-config` would otherwise slip into the
/// streams read here. Same harness and same reason as `server.test.ts`.
let restaurerTurn: () => void;
beforeAll(() => {
    restaurerTurn = poserTurnAmbiant();
});
afterAll(() => {
    restaurerTurn();
});

let server: ReturnType<typeof createSignalingServer>;

beforeEach(() => {
    server = createSignalingServer(0, GARDE_OUVERTE, new Frein(), new Set());
});

afterEach(async () => {
    await server.close();
});

function connecter(role: 'agent' | 'client', session: string): Promise<WebSocket> {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
        ws.on('error', reject);
        ws.on('open', () => {
            ws.send(JSON.stringify({ role, session }));
            resolve(ws);
        });
    });
}

/// Collects EVERYTHING that arrives on a socket, from its opening.
///
/// ⚠️ **A `once('message')` would not be enough**: it only listens from
/// its attachment, so a message emitted earlier would be lost and the test
/// would pass or not depending on scheduling. We accumulate, then wait for the
/// FACT (a checked predicate), never a duration — rule of the repository.
function recueillir(ws: WebSocket): any[] {
    const recus: any[] = [];
    ws.on('message', (raw) => recus.push(JSON.parse(raw.toString())));
    return recus;
}

async function attendre(predicat: () => boolean, quoi: string, msMax = 2000): Promise<void> {
    const fin = Date.now() + msMax;
    while (Date.now() < fin) {
        if (predicat()) return;
        await new Promise((r) => setTimeout(r, 10));
    }
    throw new Error(`never obtained: ${quoi}`);
}

describe("the arrival of a peer on a session already held", () => {
    it("warns the agent in place when a client joins the session", async () => {
        const agent = await connecter('agent', 'bureau');
        const recus = recueillir(agent);

        const client = await connecter('client', 'bureau');
        await attendre(
            () => recus.some((m) => m.type === TYPE_PAIR_PRESENT),
            'the pair-present expected by the supervisor',
        );

        agent.close();
        client.close();
    });

    it("does NOT warn the client in place when the agent joins the session", async () => {
        // 🔴 THE NEGATIVE WITNESS, and it carries the most fragile half of the
        // rule: it is the NORMAL order on every `w-N` window session —
        // the browser page connects first, the child arrives
        // next. Notifying here would send the message to every session
        // page, where `client/src/webrtc.ts` does not recognise it.
        const client = await connecter('client', 'w-1');
        const recus = recueillir(client);

        const agent = await connecter('agent', 'w-1');
        // A thing KNOWN TO ARRIVE serves as the bound: the relay hands
        // the agent the held offer, the agent answers, and that answer does
        // go through. Without this witness, "no pair-present" would be just as true
        // of an entirely silent relay.
        agent.send(JSON.stringify({ type: 'answer', sdp: 'v=0 answer' }));
        await attendre(
            () => recus.some((m) => m.type === 'answer'),
            'the SDP answer, which proves that this socket does receive something',
        );
        expect(recus.filter((m) => m.type === TYPE_PAIR_PRESENT)).toEqual([]);

        agent.close();
        client.close();
    });

    it("cannot be FORGED by a peer: the type is not relayed", async () => {
        // 🔴 If it entered `TYPES_RELAYES`, an authenticated client could
        // make the agent re-announce at will — an amplifier offered to whoever
        // has a token.
        const agent = await connecter('agent', 'bureau');
        const recusAgent = recueillir(agent);
        const client = await connecter('client', 'bureau');
        const recusClient = recueillir(client);

        // ⚠️ **WE COUNT, WE DO NOT REQUIRE ZERO** — and this detail was paid for in the
        // first draft of this test: the agent LEGITIMATELY receives a
        // `pair-present` at the client's arrival, a few milliseconds after
        // `connecter` returned control. A `toEqual([])` turned red there
        // for the right value and the wrong reason.
        await attendre(
            () => recusAgent.filter((m) => m.type === TYPE_PAIR_PRESENT).length === 1,
            "the LEGITIMATE pair-present, the one of the client's arrival",
        );

        client.send(JSON.stringify({ type: TYPE_PAIR_PRESENT }));
        await attendre(
            () => recusClient.some((m) => m.type === 'error'),
            "the unknown-type refusal returned to the sender",
        );
        expect(recusAgent.filter((m) => m.type === TYPE_PAIR_PRESENT)).toHaveLength(1);

        agent.close();
        client.close();
    });

    it("warns the agent that ARRIVES when a client was already waiting for it", async () => {
        // 🔴 THE RECONNECTION CASE, AND IT DID NOT EXIST BEFORE THIS BATCH.
        // The agent only opened its control session once, at startup:
        // it was therefore always the first to arrive. Since it REOPENS it
        // after a drop, the order is reversed as soon as the shell page comes back
        // before it — the ordinary case after a service restart, the
        // browser being reloaded by hand within a few seconds where
        // the agent respects a backoff that can reach thirty seconds.
        const client = await connecter('client', 'bureau');
        const agent = await connecter('agent', 'bureau');
        const recus = recueillir(agent);
        await attendre(
            () => recus.some((m) => m.type === TYPE_PAIR_PRESENT),
            "the pair-present that the reconnected agent must receive",
        );

        agent.close();
        client.close();
    });

    it("does NOT warn the agent that arrives FIRST", async () => {
        // 🔴 THE NEGATIVE WITNESS OF THE TEST ABOVE: an agent that arrives ALONE
        // must receive no pair-present, hence must not re-announce
        // its windows into the void — the very gesture this mechanism avoids.
        //
        // ⚠️ **WHAT IT DOES NOT ESTABLISH, AND THE FIRST DRAFT OF THIS
        // COMMENT WRONGLY CLAIMED IT** — fixed after measuring it
        // by mutation. It does NOT hold the pairing guard
        // `if (pairEnFace)` of `relais.ts`: removing it leaves this test GREEN,
        // because `send(undefined, …)` is already a no-op through the null guard
        // of `send` itself. The mutation that removes `if (pairEnFace)`
        // turns the neighbouring test "cannot be FORGED" red (two
        // pair-present instead of one), and it is THAT one that holds this property.
        // This test only holds the `prevenirLArrivant` rule as it
        // is WIRED — checked: making `prevenirLArrivant` always `true`
        // does not turn it red either, for the same reason.
        const agent = await connecter('agent', 'bureau');
        const recus = recueillir(agent);

        // A thing KNOWN TO ARRIVE bounds the wait: the client's arrival
        // triggers, for its part, a LEGITIMATE pair-present (the other half of the
        // rule). If there is only ONE, the agent's arrival alone
        // produced none. A bare zero would prove nothing: it would
        // also be that of an entirely silent relay.
        const client = await connecter('client', 'bureau');
        await attendre(
            () => recus.filter((m) => m.type === TYPE_PAIR_PRESENT).length >= 1,
            "the legitimate pair-present, the one of the client's arrival",
        );
        expect(recus.filter((m) => m.type === TYPE_PAIR_PRESENT)).toHaveLength(1);

        agent.close();
        client.close();
    });

    it("the pure rule of the newcomer says yes to the agent, no to the client", () => {
        // The exact symmetrical of the neighbouring rule, tested SEPARATELY from the
        // socket for the same reason: it is what carries the asymmetry.
        //
        // 🔴 `prevenirLArrivant('client')` MUST BE FALSE, and it is not
        // redundant with the rule next to it: true would send the
        // message to the browser page of EACH `w-N` session, which does not
        // recognise it — the measurable noise batch 17 explicitly
        // ruled out.
        expect(prevenirLArrivant('agent')).toBe(true);
        expect(prevenirLArrivant('client')).toBe(false);
    });

    it("the two rules NEVER warn the same socket twice", () => {
        // 🔴 THEY ARE MUTUALLY EXCLUSIVE BY CONSTRUCTION, and it is what
        // guarantees that a pairing produces ONE pair-present, never two.
        // Two announcements would make the agent re-announce twice, hence
        // `AnnoncerOuverture` twice per pending window, hence a
        // shell page that RELOADS the window it has just opened
        // (`window.open(url, "guac-<session>")` targets a NAMED window).
        for (const role of ['agent', 'client'] as const) {
            expect(prevenirLePairEnPlace(role) && prevenirLArrivant(role)).toBe(false);
            expect(prevenirLePairEnPlace(role) || prevenirLArrivant(role)).toBe(true);
        }
    });

    it('the pure rule says yes to the client, no to the agent', () => {
        // The rule is tested SEPARATELY from the socket: it is what carries
        // the asymmetry, and an end-to-end test would measure it through
        // three other mechanisms.
        expect(prevenirLePairEnPlace('client')).toBe(true);
        expect(prevenirLePairEnPlace('agent')).toBe(false);
    });

    it('the name on the wire is the one the Rust agent expects', () => {
        // 🔴 THE CONTRACT LIVES IN TWO WORD STORES: here, and in
        // `agent/src/superviseur/protocole.rs` (`#[serde(rename =
        // "pair-present")]`, test `l_arrivee_d_un_pair_se_lit_sur_la_session_
        // de_controle`). A drift of this name makes the mechanism SILENT on both
        // sides, without any error.
        expect(TYPE_PAIR_PRESENT).toBe('pair-present');
    });
});
