import { afterEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import { createSpikeServer, type SpikeServer } from '../src/server.js';

let serveur: SpikeServer | undefined;

afterEach(async () => {
    await serveur?.close();
    serveur = undefined;
});

// Attend le premier message JSON reçu sur le socket, ou échoue après 2 s.
function premierMessage(socket: WebSocket): Promise<unknown> {
    return new Promise((resolve, reject) => {
        const minuteur = setTimeout(() => reject(new Error('aucun message reçu')), 2000);
        socket.once('message', (brut) => {
            clearTimeout(minuteur);
            resolve(JSON.parse(brut.toString()));
        });
    });
}

function ouvert(socket: WebSocket): Promise<void> {
    return new Promise((resolve) => socket.once('open', () => resolve()));
}

describe('serveur du spike', () => {
    it('sert la page principale', async () => {
        serveur = await createSpikeServer(0);
        const reponse = await fetch(`http://127.0.0.1:${serveur.port}/`);
        expect(reponse.status).toBe(200);
        expect(reponse.headers.get('content-type')).toContain('text/html');
    });

    it('diffuse un ordre de déclenchement à tous les clients WebSocket', async () => {
        serveur = await createSpikeServer(0);
        const socket = new WebSocket(`ws://127.0.0.1:${serveur.port}/ws`);
        await ouvert(socket);

        const attendu = premierMessage(socket);
        await fetch(`http://127.0.0.1:${serveur.port}/fire`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ variant: 2 }),
        });

        expect(await attendu).toMatchObject({ type: 'fire', variant: 2 });
        socket.close();
    });

    it('rediffuse le signal de vie de la fenêtre ouverte', async () => {
        serveur = await createSpikeServer(0);
        const socket = new WebSocket(`ws://127.0.0.1:${serveur.port}/ws`);
        await ouvert(socket);

        const attendu = premierMessage(socket);
        await fetch(`http://127.0.0.1:${serveur.port}/alive?variant=3`);

        expect(await attendu).toMatchObject({ type: 'alive', variant: 3 });
        socket.close();
    });

    it('refuse une variante hors de 1..4 sans planter', async () => {
        serveur = await createSpikeServer(0);
        const reponse = await fetch(`http://127.0.0.1:${serveur.port}/fire`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ variant: 99 }),
        });
        expect(reponse.status).toBe(400);
    });

    it('renvoie 404 sur un chemin inconnu', async () => {
        serveur = await createSpikeServer(0);
        const reponse = await fetch(`http://127.0.0.1:${serveur.port}/inexistant`);
        expect(reponse.status).toBe(404);
    });
});
