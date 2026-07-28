// Serveur du spike multi-fenêtres. Sert `public/` et relaie trois événements :
// `fire` (ordre d'ouverture, émis par POST /fire), `alive` (signal de vie de la
// fenêtre ouverte, émis par GET /alive) et `bloque` (le service worker rapporte
// que clients.openWindow() n'a rien ouvert, émis par GET /bloque). Aucun état
// persistant.
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

// Adresse d'écoute. Voir le commentaire sur `listen` plus bas : ce n'est pas un
// réglage, c'est une garde.
const HOTE = '127.0.0.1';

export interface SpikeServer {
    port: number;
    /** Adresse effectivement liée — exposée pour que la garde soit vérifiable. */
    hote: string;
    close(): Promise<void>;
}

function estVarianteValide(valeur: unknown): valeur is number {
    return typeof valeur === 'number' && Number.isInteger(valeur) && valeur >= 1 && valeur <= 4;
}

// Identifiant de passage forgé par la page. Le serveur ne l'interprète pas — il
// le recopie tel quel dans la diffusion, c'est la page qui tranche. Il le borne
// tout de même : sans cela n'importe qui pourrait faire diffuser une chaîne
// arbitrairement longue à tous les clients WebSocket.
const MOTIF_NONCE = /^[A-Za-z0-9_-]{1,64}$/;

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

    // Rend le nombre de clients réellement touchés : POST /fire le renvoie à la
    // page, faute de quoi un socket tombé produirait une mesure silencieusement
    // vide — un 204 rassurant sans que personne n'ait reçu l'ordre.
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

        if (url.pathname === '/fire') {
            // Contrôle de méthode explicite : une route documentée qui accepte
            // n'importe quel verbe est une route qu'on peut déclencher par
            // accident, et un déclenchement accidentel fausse la mesure.
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

        // `/alive` et `/bloque` ne diffèrent que par le type diffusé : le premier
        // dit « la fenêtre existe », le second « le service worker n'a rien pu
        // ouvrir ». Même contrat d'entrée, donc même garde.
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
        // Un socket en erreur qui n'est pas retiré du Set ferait grossir la
        // diffusion indéfiniment.
        socket.on('error', () => clients.delete(socket));
    });

    // Liaison explicite à la boucle locale. Le spike tourne sur un poste
    // délibérément exposé à internet le temps du test, et il n'est censé être
    // joignable qu'à travers le proxy d'authentification qui l'atteint sur
    // 127.0.0.1 : écouter sur 0.0.0.0 le rendrait accessible hors du proxy, alors
    // que le WebSocket n'est soumis à aucun CORS et ne vérifie aucune origine.
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
