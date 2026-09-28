// Server of the multi-window spike. Serves `public/` and relays three events:
// `fire` (open order, emitted by POST /fire), `alive` (sign of life of the
// opened window, emitted by GET /alive) and `bloque` (the service worker reports
// that clients.openWindow() opened nothing, emitted by GET /bloque). No
// persistent state.
//
// Triggering deliberately goes through the network and not through a click in the
// page: that is the whole condition under test — a server message is not a
// transient user activation.

import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { WebSocketServer, WebSocket } from 'ws';

const RACINE_PUBLIQUE = fileURLToPath(new URL('../public/', import.meta.url));

const TYPES_MIME: Record<string, string> = {
    '.html': 'text/html; charset=utf-8',
    '.js': 'text/javascript; charset=utf-8',
    '.json': 'application/json; charset=utf-8',
    '.png': 'image/png',
    '.ico': 'image/x-icon',
};

// Listen address. See the comment on `listen` below: it is not a
// setting, it is a guard.
const HOTE = '127.0.0.1';

export interface SpikeServer {
    port: number;
    /** Address actually bound — exposed so the guard can be checked. */
    hote: string;
    close(): Promise<void>;
}

function estVarianteValide(valeur: unknown): valeur is number {
    return typeof valeur === 'number' && Number.isInteger(valeur) && valeur >= 1 && valeur <= 4;
}

// Run identifier forged by the page. The server does not interpret it — it
// copies it as is into the broadcast, the page is the one that decides. It bounds it
// all the same: without that anyone could have an arbitrarily long string
// broadcast to all WebSocket clients.
const MOTIF_NONCE = /^[A-Za-z0-9_-]{1,64}$/;

// Reads the body of a request, bounded to 4 KiB: the server is exposed via Pomerium,
// and an endless request would tie up the process.
function lireCorps(requete: IncomingMessage): Promise<string> {
    return new Promise((resolve, reject) => {
        let corps = '';
        requete.on('data', (morceau) => {
            corps += morceau;
            if (corps.length > 4096) {
                reject(new Error('corps trop volumineux'));
                requete.destroy();
            }
        });
        requete.on('end', () => resolve(corps));
        requete.on('error', reject);
    });
}

export async function createSpikeServer(port: number): Promise<SpikeServer> {
    const clients = new Set<WebSocket>();

    // Returns the number of clients actually reached: POST /fire sends it back to the
    // page, otherwise a dropped socket would produce a silently
    // empty measurement — a reassuring 204 without anyone having received the order.
    function diffuser(charge: Record<string, unknown>): number {
        const texte = JSON.stringify({ ...charge, at: Date.now() });
        let touches = 0;
        for (const client of clients) {
            if (client.readyState === WebSocket.OPEN) {
                client.send(texte);
                touches += 1;
            }
        }
        return touches;
    }

    async function servirFichier(chemin: string, reponse: ServerResponse): Promise<void> {
        // `normalize` then prefix check: without it, `/../.env` would escape
        // public/. The server is exposed on the internet via Pomerium.
        const absolu = normalize(join(RACINE_PUBLIQUE, chemin));
        if (!absolu.startsWith(RACINE_PUBLIQUE)) {
            reponse.writeHead(403).end('interdit');
            return;
        }
        try {
            const contenu = await readFile(absolu);
            const type = TYPES_MIME[extname(absolu)] ?? 'application/octet-stream';
            // No cache: a spike rerun several times with changed pages
            // would otherwise yield verdicts obtained on stale code.
            reponse.writeHead(200, { 'content-type': type, 'cache-control': 'no-store' });
            reponse.end(contenu);
        } catch {
            reponse.writeHead(404).end('introuvable');
        }
    }

    const http = createServer(async (requete, reponse) => {
        const url = new URL(requete.url ?? '/', `http://${requete.headers.host ?? 'localhost'}`);

        if (url.pathname === '/fire') {
            // Explicit method check: a documented route that accepts
            // any verb is a route that can be triggered by
            // accident, and an accidental trigger skews the measurement.
            if (requete.method !== 'POST') {
                reponse.writeHead(405, { allow: 'POST' }).end('méthode non autorisée');
                return;
            }
            let variante: unknown;
            try {
                variante = (JSON.parse(await lireCorps(requete)) as { variant?: unknown }).variant;
            } catch {
                reponse.writeHead(400).end('JSON invalide');
                return;
            }
            if (!estVarianteValide(variante)) {
                reponse.writeHead(400).end('variante invalide');
                return;
            }
            const touches = diffuser({ type: 'fire', variant: variante });
            reponse.writeHead(200, { 'content-type': 'application/json; charset=utf-8' });
            reponse.end(JSON.stringify({ clients: touches }));
            return;
        }

        // `/alive` and `/bloque` differ only by the type broadcast: the first
        // says "the window exists", the second "the service worker could not
        // open anything". Same input contract, hence the same guard.
        if (url.pathname === '/alive' || url.pathname === '/bloque') {
            if (requete.method !== 'GET') {
                reponse.writeHead(405, { allow: 'GET' }).end('méthode non autorisée');
                return;
            }
            const variante = Number(url.searchParams.get('variant'));
            if (!estVarianteValide(variante)) {
                reponse.writeHead(400).end('variante invalide');
                return;
            }
            const nonce = url.searchParams.get('nonce');
            if (nonce !== null && !MOTIF_NONCE.test(nonce)) {
                reponse.writeHead(400).end('nonce invalide');
                return;
            }
            diffuser({ type: url.pathname === '/alive' ? 'alive' : 'bloque', variant: variante, nonce });
            reponse.writeHead(204).end();
            return;
        }

        await servirFichier(url.pathname === '/' ? 'index.html' : url.pathname, reponse);
    });

    const wss = new WebSocketServer({ server: http, path: '/ws' });
    wss.on('connection', (socket) => {
        clients.add(socket);
        socket.on('close', () => clients.delete(socket));
        // A socket in error that is not removed from the Set would make the
        // broadcast grow indefinitely.
        socket.on('error', () => clients.delete(socket));
    });

    // Explicit binding to the loopback. The spike runs on a machine
    // deliberately exposed to the internet for the duration of the test, and it is only meant to be
    // reachable through the authentication proxy that reaches it on
    // 127.0.0.1: listening on 0.0.0.0 would make it reachable outside the proxy, whereas
    // the WebSocket is subject to no CORS and checks no origin.
    await new Promise<void>((resolve) => http.listen(port, HOTE, resolve));
    const adresse = http.address();
    const portEffectif = typeof adresse === 'object' && adresse ? adresse.port : port;
    const hoteEffectif = typeof adresse === 'object' && adresse ? adresse.address : HOTE;

    return {
        port: portEffectif,
        hote: hoteEffectif,
        close(): Promise<void> {
            return new Promise((resolve) => {
                for (const client of clients) client.terminate();
                wss.close(() => http.close(() => resolve()));
            });
        },
    };
}
