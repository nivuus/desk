import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import { createSignalingServer } from './server';

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
});
