// Le message « ton pair est arrivé » : qu'il parte, à qui, et à qui PAS.
//
// 🔴 CE FICHIER EST LE TÉMOIN DE `nextMessage` (`server.test.ts`,
// `resilience.test.ts`), QUI FILTRE `pair-present` DEPUIS CE LOT. Sans une
// mesure qui établit que le message EST émis, ce filtre serait indiscernable
// d'une mise sous le tapis : les quatre tests qu'il répare passeraient tout
// aussi bien si le relais n'envoyait plus rien du tout.
//
// 🔴 CE QU'IL DÉFEND : sans ce message, un superviseur qui a annoncé ses
// fenêtres AVANT l'arrivée de la page-shell n'apprend jamais qu'il faut les
// redire, et l'utilisateur trouve un bureau vide sur une VM pleine de
// fenêtres — le défaut mesuré en production le 30 août 2026 (voir
// `pair-present.ts`).

import { afterEach, beforeAll, afterAll, beforeEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import type { Garde } from '../identite/garde';
import { Frein } from '../securite/frein';
import { createSignalingServer } from './relais';
import { poserTurnAmbiant } from './turn-harnais';
import { prevenirLePairEnPlace, TYPE_PAIR_PRESENT } from './pair-present';

/// Locale à ce fichier, jamais exportée par du code de production — même
/// argument et même forme que `server.test.ts` : ces tests éprouvent le
/// RELAIS, pas l'authentification, qui a son propre fichier.
const GARDE_OUVERTE: Garde = {
    verifier: () => ({ ok: true }),
    revendiquer: () => {},
    liberer: () => {},
};

/// Un relais SANS TURN : `ice-config` viendrait sinon s'intercaler dans les
/// flux qu'on lit ici. Même harnais et même raison que `server.test.ts`.
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

/// Recueille TOUT ce qui arrive sur un socket, depuis son ouverture.
///
/// ⚠️ **Un `once('message')` ne suffirait pas** : il n'écoute qu'à partir de
/// son attachement, donc un message émis plus tôt serait perdu et le test
/// passerait ou non selon l'ordonnancement. On accumule, puis on attend le
/// FAIT (une prédicat vérifié), jamais une durée — règle du dépôt.
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
    throw new Error(`jamais obtenu : ${quoi}`);
}

describe("l'arrivée d'un pair sur une session déjà tenue", () => {
    it("prévient l'agent en place quand un client rejoint la session", async () => {
        const agent = await connecter('agent', 'bureau');
        const recus = recueillir(agent);

        const client = await connecter('client', 'bureau');
        await attendre(
            () => recus.some((m) => m.type === TYPE_PAIR_PRESENT),
            'le pair-present attendu par le superviseur',
        );

        agent.close();
        client.close();
    });

    it("ne prévient PAS le client en place quand l'agent rejoint la session", async () => {
        // 🔴 LE TÉMOIN NÉGATIF, et il porte la moitié la plus fragile de la
        // règle : c'est l'ordre NORMAL sur toute session de fenêtre `w-N` —
        // la page navigateur se connecte la première, l'enfant arrive
        // ensuite. Prévenir ici enverrait le message à toutes les pages de
        // session, où `client/src/webrtc.ts` ne le reconnaît pas.
        const client = await connecter('client', 'w-1');
        const recus = recueillir(client);

        const agent = await connecter('agent', 'w-1');
        // Une chose CONNUE POUR ARRIVER sert de borne : le relais remet à
        // l'agent l'offre retenue, l'agent répond, et cette réponse-là passe
        // bien. Sans ce témoin, « aucun pair-present » serait aussi vrai
        // d'un relais entièrement muet.
        agent.send(JSON.stringify({ type: 'answer', sdp: 'v=0 réponse' }));
        await attendre(
            () => recus.some((m) => m.type === 'answer'),
            'la réponse SDP, qui prouve que ce socket reçoit bien quelque chose',
        );
        expect(recus.filter((m) => m.type === TYPE_PAIR_PRESENT)).toEqual([]);

        agent.close();
        client.close();
    });

    it("ne se laisse pas FABRIQUER par un pair : le type n'est pas relayé", async () => {
        // 🔴 S'il entrait dans `TYPES_RELAYES`, un client authentifié pourrait
        // faire réannoncer l'agent à volonté — un amplificateur offert à qui
        // a un jeton.
        const agent = await connecter('agent', 'bureau');
        const recusAgent = recueillir(agent);
        const client = await connecter('client', 'bureau');
        const recusClient = recueillir(client);

        // ⚠️ **ON COMPTE, ON N'EXIGE PAS ZÉRO** — et ce détail a été payé au
        // premier jet de ce test : l'agent reçoit LÉGITIMEMENT un
        // `pair-present` à l'arrivée du client, quelques millisecondes après
        // que `connecter` a rendu la main. Un `toEqual([])` y rougissait
        // pour la bonne valeur et la mauvaise raison.
        await attendre(
            () => recusAgent.filter((m) => m.type === TYPE_PAIR_PRESENT).length === 1,
            "le pair-present LÉGITIME, celui de l'arrivée du client",
        );

        client.send(JSON.stringify({ type: TYPE_PAIR_PRESENT }));
        await attendre(
            () => recusClient.some((m) => m.type === 'error'),
            "le refus de type inconnu rendu à l'expéditeur",
        );
        expect(recusAgent.filter((m) => m.type === TYPE_PAIR_PRESENT)).toHaveLength(1);

        agent.close();
        client.close();
    });

    it('la règle pure dit oui au client, non à l’agent', () => {
        // La règle est éprouvée SÉPARÉMENT du socket : c'est elle qui porte
        // l'asymétrie, et un test de bout en bout la mesurerait à travers
        // trois autres mécanismes.
        expect(prevenirLePairEnPlace('client')).toBe(true);
        expect(prevenirLePairEnPlace('agent')).toBe(false);
    });

    it('le nom sur le fil est celui que l’agent Rust attend', () => {
        // 🔴 LE CONTRAT VIT DANS DEUX DÉPÔTS DE MOTS : ici, et dans
        // `agent/src/superviseur/protocole.rs` (`#[serde(rename =
        // "pair-present")]`, test `l_arrivee_d_un_pair_se_lit_sur_la_session_
        // de_controle`). Une dérive de ce nom rend le mécanisme MUET des deux
        // côtés, sans aucune erreur.
        expect(TYPE_PAIR_PRESENT).toBe('pair-present');
    });
});
