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

    // I5 — le spike tourne sur un poste exposé à internet, derrière un proxy
    // d'authentification qui l'atteint sur la boucle locale. Une écoute sur
    // 0.0.0.0 le rendrait joignable hors du proxy.
    it("n'écoute que sur la boucle locale", async () => {
        serveur = await createSpikeServer(0);
        expect(serveur.hote).toBe('127.0.0.1');
    });

    // I3 — sans ce compte, un socket tombé donnerait un déclenchement « réussi »
    // que personne n'a reçu : une mesure silencieusement vide.
    it('rapporte le nombre de clients touchés par /fire', async () => {
        serveur = await createSpikeServer(0);

        const sansClient = await fetch(`http://127.0.0.1:${serveur.port}/fire`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ variant: 1 }),
        });
        expect(sansClient.status).toBe(200);
        expect(await sansClient.json()).toEqual({ clients: 0 });

        const premier = new WebSocket(`ws://127.0.0.1:${serveur.port}/ws`);
        const second = new WebSocket(`ws://127.0.0.1:${serveur.port}/ws`);
        await Promise.all([ouvert(premier), ouvert(second)]);

        const avecClients = await fetch(`http://127.0.0.1:${serveur.port}/fire`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ variant: 1 }),
        });
        expect(await avecClients.json()).toEqual({ clients: 2 });

        premier.close();
        second.close();
    });

    // I7 — le nonce est l'identité du passage. Le serveur ne l'interprète pas,
    // mais il doit le recopier fidèlement, sans quoi la page ne peut pas
    // distinguer le signal de sa fenêtre de celui d'une fenêtre étrangère.
    it('recopie le nonce du signal de vie dans la diffusion', async () => {
        serveur = await createSpikeServer(0);
        const socket = new WebSocket(`ws://127.0.0.1:${serveur.port}/ws`);
        await ouvert(socket);

        const attendu = premierMessage(socket);
        await fetch(`http://127.0.0.1:${serveur.port}/alive?variant=2&nonce=abc123`);

        expect(await attendu).toMatchObject({ type: 'alive', variant: 2, nonce: 'abc123' });
        socket.close();
    });

    it('refuse un nonce hors du format attendu', async () => {
        serveur = await createSpikeServer(0);
        const reponse = await fetch(
            `http://127.0.0.1:${serveur.port}/alive?variant=2&nonce=${'x'.repeat(65)}`,
        );
        expect(reponse.status).toBe(400);
    });

    // I4 — le service worker rapporte ici que clients.openWindow() n'a rien
    // ouvert : c'est ce qui distingue un blocage d'une fenêtre partie sur l'IdP.
    it('diffuse le blocage rapporté par le service worker', async () => {
        serveur = await createSpikeServer(0);
        const socket = new WebSocket(`ws://127.0.0.1:${serveur.port}/ws`);
        await ouvert(socket);

        const attendu = premierMessage(socket);
        await fetch(`http://127.0.0.1:${serveur.port}/bloque?variant=4&nonce=deadbeef`);

        expect(await attendu).toMatchObject({ type: 'bloque', variant: 4, nonce: 'deadbeef' });
        socket.close();
    });

    it('refuse une variante invalide sur /bloque', async () => {
        serveur = await createSpikeServer(0);
        const reponse = await fetch(`http://127.0.0.1:${serveur.port}/bloque?variant=9`);
        expect(reponse.status).toBe(400);
    });

    // I7 — une route documentée en GET qui accepte POST est une route qu'on peut
    // déclencher par accident, et un déclenchement accidentel fausse la mesure.
    it('refuse les méthodes non documentées sur /alive, /bloque et /fire', async () => {
        serveur = await createSpikeServer(0);
        const base = `http://127.0.0.1:${serveur.port}`;

        const alivePost = await fetch(`${base}/alive?variant=1`, { method: 'POST' });
        expect(alivePost.status).toBe(405);
        expect(alivePost.headers.get('allow')).toBe('GET');

        const bloquePost = await fetch(`${base}/bloque?variant=4`, { method: 'POST' });
        expect(bloquePost.status).toBe(405);

        const fireGet = await fetch(`${base}/fire`);
        expect(fireGet.status).toBe(405);
        expect(fireGet.headers.get('allow')).toBe('POST');
    });
});
