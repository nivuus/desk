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
import { prevenirLArrivant, prevenirLePairEnPlace, TYPE_PAIR_PRESENT } from './pair-present';

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

    it("prévient l'agent qui ARRIVE quand un client l'attendait déjà", async () => {
        // 🔴 LE CAS DE LA RECONNEXION, ET IL N'EXISTAIT PAS AVANT CE LOT.
        // L'agent n'ouvrait sa session de contrôle qu'une fois, au démarrage :
        // il était donc toujours le premier arrivé. Depuis qu'il la ROUVRE
        // après une chute, l'ordre s'inverse dès que la page-shell revient
        // avant lui — le cas ordinaire après un redémarrage du service, le
        // navigateur étant rechargé à la main en quelques secondes là où
        // l'agent respecte un repli qui peut atteindre trente secondes.
        const client = await connecter('client', 'bureau');
        const agent = await connecter('agent', 'bureau');
        const recus = recueillir(agent);
        await attendre(
            () => recus.some((m) => m.type === TYPE_PAIR_PRESENT),
            "le pair-present que l'agent reconnecté doit recevoir",
        );

        agent.close();
        client.close();
    });

    it("ne prévient PAS l'agent qui arrive le PREMIER", async () => {
        // 🔴 LE TÉMOIN NÉGATIF DU TEST CI-DESSUS : un agent qui arrive SEUL
        // ne doit recevoir aucun pair-present, donc ne doit pas réannoncer
        // ses fenêtres dans le vide — le geste même que ce mécanisme évite.
        //
        // ⚠️ **CE QU'IL N'ÉTABLIT PAS, ET LA PREMIÈRE RÉDACTION DE CE
        // COMMENTAIRE LE PRÉTENDAIT À TORT** — corrigé après l'avoir mesuré
        // par mutation. Il ne tient PAS la garde d'appariement
        // `if (pairEnFace)` de `relais.ts` : la retirer laisse ce test VERT,
        // parce que `send(undefined, …)` est déjà un no-op par la garde de
        // nullité de `send` lui-même. La mutation qui retire `if (pairEnFace)`
        // rougit le test voisin « ne se laisse pas FABRIQUER » (deux
        // pair-present au lieu d'un), et c'est LUI qui tient cette propriété.
        // Ce test-ci ne tient que la règle `prevenirLArrivant` telle qu'elle
        // est CÂBLÉE — vérifié : rendre `prevenirLArrivant` toujours `true`
        // ne le rougit pas non plus, pour la même raison.
        const agent = await connecter('agent', 'bureau');
        const recus = recueillir(agent);

        // Une chose CONNUE POUR ARRIVER borne l'attente : l'arrivée du client
        // déclenche, elle, un pair-present LÉGITIME (l'autre moitié de la
        // règle). S'il n'y en a qu'UN, c'est que l'arrivée de l'agent seul
        // n'en a produit aucun. Un zéro nu ne prouverait rien : il serait
        // aussi celui d'un relais entièrement muet.
        const client = await connecter('client', 'bureau');
        await attendre(
            () => recus.filter((m) => m.type === TYPE_PAIR_PRESENT).length >= 1,
            "le pair-present légitime, celui de l'arrivée du client",
        );
        expect(recus.filter((m) => m.type === TYPE_PAIR_PRESENT)).toHaveLength(1);

        agent.close();
        client.close();
    });

    it("la règle pure de l'arrivant dit oui à l’agent, non au client", () => {
        // Le symétrique exact de la règle voisine, éprouvé SÉPARÉMENT du
        // socket pour la même raison : c'est elle qui porte l'asymétrie.
        //
        // 🔴 `prevenirLArrivant('client')` DOIT ÊTRE FAUX, et ce n'est pas
        // une redondance avec la règle d'à côté : le vrai enverrait le
        // message à la page navigateur de CHAQUE session `w-N`, qui ne le
        // reconnaît pas — le bruit mesurable que le lot 17 a nommément
        // écarté.
        expect(prevenirLArrivant('agent')).toBe(true);
        expect(prevenirLArrivant('client')).toBe(false);
    });

    it("les deux règles ne prévient JAMAIS deux fois le même socket", () => {
        // 🔴 ELLES SONT MUTUELLEMENT EXCLUSIVES PAR CONSTRUCTION, et c'est ce
        // qui garantit qu'un appariement produit UN pair-present, jamais deux.
        // Deux annonces feraient réannoncer l'agent deux fois, donc
        // `AnnoncerOuverture` deux fois par fenêtre en attente, donc une
        // page-shell qui RECHARGE la fenêtre qu'elle vient d'ouvrir
        // (`window.open(url, "guac-<session>")` vise une fenêtre NOMMÉE).
        for (const role of ['agent', 'client'] as const) {
            expect(prevenirLePairEnPlace(role) && prevenirLArrivant(role)).toBe(false);
            expect(prevenirLePairEnPlace(role) || prevenirLArrivant(role)).toBe(true);
        }
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
