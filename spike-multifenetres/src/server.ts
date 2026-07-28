// Serveur du spike multi-fenêtres. Sert `public/` et relaie deux événements :
// `fire` (ordre d'ouverture, émis par POST /fire) et `alive` (signal de vie de la
// fenêtre ouverte, émis par GET /alive). Aucun état persistant.
//
// Le déclenchement passe délibérément par le réseau et non par un clic dans la
// page : c'est toute la condition testée — un message serveur n'est pas une
// activation utilisateur transitoire.

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

export interface SpikeServer {
    port: number;
    close(): Promise<void>;
}

function estVarianteValide(valeur: unknown): valeur is number {
    return typeof valeur === 'number' && Number.isInteger(valeur) && valeur >= 1 && valeur <= 4;
}

// Lit le corps d'une requête, borné à 4 Kio : le serveur est exposé via Pomerium,
// et une requête sans fin immobiliserait le process.
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

    function diffuser(charge: Record<string, unknown>): void {
        const texte = JSON.stringify({ ...charge, at: Date.now() });
        for (const client of clients) {
            if (client.readyState === WebSocket.OPEN) client.send(texte);
        }
    }

    async function servirFichier(chemin: string, reponse: ServerResponse): Promise<void> {
        // `normalize` puis vérification du préfixe : sans cela, `/../.env` sortirait
        // de public/. Le serveur est exposé sur internet via Pomerium.
        const absolu = normalize(join(RACINE_PUBLIQUE, chemin));
        if (!absolu.startsWith(RACINE_PUBLIQUE)) {
            reponse.writeHead(403).end('interdit');
            return;
        }
        try {
            const contenu = await readFile(absolu);
            const type = TYPES_MIME[extname(absolu)] ?? 'application/octet-stream';
            // Aucun cache : un spike relancé plusieurs fois avec des pages modifiées
            // donnerait sinon des verdicts obtenus sur du code périmé.
            reponse.writeHead(200, { 'content-type': type, 'cache-control': 'no-store' });
            reponse.end(contenu);
        } catch {
            reponse.writeHead(404).end('introuvable');
        }
    }

    const http = createServer(async (requete, reponse) => {
        const url = new URL(requete.url ?? '/', `http://${requete.headers.host ?? 'localhost'}`);

        if (requete.method === 'POST' && url.pathname === '/fire') {
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
            diffuser({ type: 'fire', variant: variante });
            reponse.writeHead(204).end();
            return;
        }

        if (url.pathname === '/alive') {
            const variante = Number(url.searchParams.get('variant'));
            if (!estVarianteValide(variante)) {
                reponse.writeHead(400).end('variante invalide');
                return;
            }
            diffuser({ type: 'alive', variant: variante });
            reponse.writeHead(204).end();
            return;
        }

        await servirFichier(url.pathname === '/' ? 'index.html' : url.pathname, reponse);
    });

    const wss = new WebSocketServer({ server: http, path: '/ws' });
    wss.on('connection', (socket) => {
        clients.add(socket);
        socket.on('close', () => clients.delete(socket));
        // Un socket en erreur qui n'est pas retiré du Set ferait grossir la
        // diffusion indéfiniment.
        socket.on('error', () => clients.delete(socket));
    });

    await new Promise<void>((resolve) => http.listen(port, resolve));
    const adresse = http.address();
    const portEffectif = typeof adresse === 'object' && adresse ? adresse.port : port;

    return {
        port: portEffectif,
        close(): Promise<void> {
            return new Promise((resolve) => {
                for (const client of clients) client.terminate();
                wss.close(() => http.close(() => resolve()));
            });
        },
    };
}
