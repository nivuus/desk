import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import { createSignalingServer } from './relais';

let server: ReturnType<typeof createSignalingServer>;

function connect(role: 'agent' | 'client', session: string): Promise<WebSocket> {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
        ws.on('error', reject);
        ws.on('open', () => {
            ws.send(JSON.stringify({ role, session }));
            resolve(ws);
        });
    });
}

function nextMessage(ws: WebSocket): Promise<any> {
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('aucun message reçu')), 2000);
        ws.once('message', (raw) => {
            clearTimeout(timer);
            resolve(JSON.parse(raw.toString()));
        });
    });
}

// Ferme le socket et attend que le serveur ait traité l'évènement `close`
// (le handler `close` du serveur est synchrone mais s'exécute après un aller-
// retour réseau local ; le petit délai laisse cet aller-retour se terminer
// avant que le test n'interroge l'état côté serveur via une nouvelle connexion).
function closeAndWait(ws: WebSocket): Promise<void> {
    return new Promise((resolve) => {
        ws.once('close', () => setTimeout(resolve, 50));
        ws.close();
    });
}

beforeEach(() => {
    server = createSignalingServer(0);
});

afterEach(async () => {
    await server.close();
});

describe('serveur de signaling', () => {
    it('relaie une offre du client vers l\'agent', async () => {
        const agent = await connect('agent', 's1');
        const client = await connect('client', 's1');

        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 offre' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 offre' });

        agent.close();
        client.close();
    });

    it('relaie une réponse de l\'agent vers le client', async () => {
        const agent = await connect('agent', 's2');
        const client = await connect('client', 's2');

        agent.send(JSON.stringify({ type: 'answer', sdp: 'v=0 reponse' }));
        expect(await nextMessage(client)).toEqual({ type: 'answer', sdp: 'v=0 reponse' });

        agent.close();
        client.close();
    });

    it('isole les sessions entre elles', async () => {
        const agentA = await connect('agent', 'sa');
        const clientB = await connect('client', 'sb');

        clientB.send(JSON.stringify({ type: 'offer', sdp: 'pour sb' }));
        await expect(nextMessage(agentA)).rejects.toThrow(/aucun message/);

        agentA.close();
        clientB.close();
    });

    it('signale la disparition du pair', async () => {
        const agent = await connect('agent', 's3');
        const client = await connect('client', 's3');

        agent.close();
        expect(await nextMessage(client)).toEqual({ type: 'peer-gone' });

        client.close();
    });

    it('rejette un premier message invalide', async () => {
        const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
        await new Promise((resolve) => ws.on('open', resolve));
        ws.send(JSON.stringify({ bonjour: true }));
        expect(await nextMessage(ws)).toEqual({
            type: 'error',
            reason: 'premier message invalide : {role, session} attendu',
        });
        ws.close();
    });

    it('rejette un second agent sur la même session', async () => {
        const first = await connect('agent', 's4');
        const second = await connect('agent', 's4');
        expect(await nextMessage(second)).toEqual({
            type: 'error',
            reason: 'un agent est déjà connecté à la session s4',
        });
        first.close();
        second.close();
    });

    // Ronde de correction 1 : un premier message JSON valide mais non-objet
    // (`null`) faisait planter le handler (`null.role` lève une TypeError non
    // interceptée). Ces deux tests verrouillent le correctif : le pair fautif
    // reçoit une erreur et reste connecté, ET une session indépendante ouverte
    // en parallèle continue de fonctionner normalement après l'incident — ce
    // qui prouve que le serveur (au sens du process qui l'héberge dans ce même
    // test, voir la mise en garde ci-dessous) n'a pas été affecté globalement.
    //
    // Attention : vitest installe son propre gestionnaire d'exceptions non
    // interceptées, qui peut faire échouer le test sans pour autant faire
    // planter le process. Ces deux tests prouvent donc le comportement de la
    // fonction exportée `createSignalingServer`, mais pas celui du process
    // réel lancé via `index.ts` : c'est `resilience.test.ts` (processus enfant
    // séparé, hors du runtime vitest) qui apporte cette preuve-là.
    it('rejette un message racine `null` en premier message sans planter, et permet de retenter', async () => {
        const faulty = new WebSocket(`ws://127.0.0.1:${server.port}`);
        await new Promise((resolve) => faulty.on('open', resolve));

        const errorReceived = nextMessage(faulty);
        faulty.send('null');
        expect(await errorReceived).toEqual({
            type: 'error',
            reason: 'premier message invalide : {role, session} attendu',
        });

        // La connexion fautive reste utilisable : un second essai, valide cette
        // fois, s'enregistre normalement et relaie comme n'importe quelle session.
        faulty.send(JSON.stringify({ role: 'agent', session: 'retry' }));
        const client = await connect('client', 'retry');
        client.send(JSON.stringify({ type: 'offer', sdp: 'apres retry' }));
        expect(await nextMessage(faulty)).toEqual({ type: 'offer', sdp: 'apres retry' });

        // Une session totalement indépendante, déjà ouverte pendant l'incident,
        // continue elle aussi de relayer normalement.
        const agent = await connect('agent', 'temoin');
        const clientTemoin = await connect('client', 'temoin');
        clientTemoin.send(JSON.stringify({ type: 'offer', sdp: 'session temoin' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'session temoin' });

        faulty.close();
        client.close();
        agent.close();
        clientTemoin.close();
    });

    it('rejette un message racine `null` en message suivant sans planter, et la session continue de fonctionner', async () => {
        const agent = await connect('agent', 's5');
        const client = await connect('client', 's5');

        // Session témoin ouverte en parallèle, avant l'envoi du message
        // malveillant, pour prouver qu'elle n'est pas affectée par l'incident.
        const agentTemoin = await connect('agent', 'temoin2');
        const clientTemoin = await connect('client', 'temoin2');

        const errorReceived = nextMessage(client);
        client.send('null');
        expect(await errorReceived).toEqual({
            type: 'error',
            reason: 'message invalide : objet JSON attendu',
        });

        // La session s5 reste fonctionnelle après l'incident.
        client.send(JSON.stringify({ type: 'offer', sdp: 'apres null' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'apres null' });

        // La session témoin, indépendante, fonctionne toujours normalement.
        clientTemoin.send(JSON.stringify({ type: 'offer', sdp: 'temoin toujours vivant' }));
        expect(await nextMessage(agentTemoin)).toEqual({ type: 'offer', sdp: 'temoin toujours vivant' });

        agent.close();
        client.close();
        agentTemoin.close();
        clientTemoin.close();
    });

    // Sous-bloc D1 : la session de contrôle réutilise les rôles agent/client
    // existants, sur un session_id réservé, pour dialoguer entre le
    // superviseur et la page « bureau » du navigateur.
    it('relaie les messages de la session de contrôle entre superviseur et shell', async () => {
        const agent = await connect('agent', 'bureau');
        const client = await connect('client', 'bureau');

        agent.send(JSON.stringify({ type: 'fenetre-ouverte', session: 'w-1', titre: 'Bloc-notes' }));
        expect(await nextMessage(client)).toEqual({
            type: 'fenetre-ouverte',
            session: 'w-1',
            titre: 'Bloc-notes',
        });

        client.send(JSON.stringify({ type: 'viewport', session: 'w-1', largeur: 1600, hauteur: 900 }));
        expect(await nextMessage(agent)).toEqual({
            type: 'viewport',
            session: 'w-1',
            largeur: 1600,
            hauteur: 900,
        });

        agent.close();
        client.close();
    });

    // Le cas de D1 : la page ouvre sa connexion et envoie son offre AVANT que
    // le superviseur n'ait lancé son enfant (c'est le viewport de cette page
    // qui décide de la taille de la sortie virtuelle, donc rien ne peut être
    // lancé plus tôt). Sans mémorisation, l'offre tombait dans le vide et la
    // session ne s'établissait jamais.
    it("délivre à l'agent l'offre arrivée avant lui", async () => {
        const client = await connect('client', 'w-tardive');
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 offre-du-client' }));

        // L'agent arrive après coup.
        const agent = await connect('agent', 'w-tardive');
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 offre-du-client' });

        agent.close();
        client.close();
    });

    it('ne délivre que la dernière offre, pas toutes celles reçues', async () => {
        const client = await connect('client', 'w-rejeu');
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 premiere' }));
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 seconde' }));

        const agent = await connect('agent', 'w-rejeu');
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 seconde' });

        agent.close();
        client.close();
    });

    // Sans cet oubli, un agent qui se reconnecterait sur un identifiant
    // réutilisé recevrait l'offre d'une session morte.
    it("oublie l'offre mémorisée quand la session se vide", async () => {
        const client = await connect('client', 'w-videe');
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 perimee' }));
        await closeAndWait(client);

        const agent = await connect('agent', 'w-videe');
        await expect(nextMessage(agent)).rejects.toThrow(/aucun message/);

        agent.close();
    });
});
