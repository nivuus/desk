// Le serveur HTTP du service, et le routage de la montée WebSocket.
//
// `noServer` plutôt que `{ server }` : le routage du chemin est explicite, et
// P3 y ajoutera `/agent` sans toucher au relais. Avec `{ server }`, `ws`
// accepterait toute montée sur tout chemin — c'est le comportement
// d'aujourd'hui (le `new WebSocketServer({ port })` de l'ex-`server.ts`, qui
// ne posait ni `host` ni `path`), et il n'est pas extensible.
//
// ⚠️ Router sur `/` RESTREINT ce qui était accepté hier. C'est délibéré, et le
// tableau des émetteurs réels a été relevé avant de le décider : la page de
// session (`client/src/main.ts`), la page-shell (`client/src/shell-page.ts`)
// et l'agent (`agent/src/signaling.rs`) visent tous les trois le chemin
// racine, sans aucun composant de chemin. Aucun pair connu n'en est affecté.

import { createServer, type Server } from 'node:http';
import { WebSocketServer } from 'ws';
import type { Config } from '../config';
import type { Pilote } from '../base/pilote';
import { createSignalingServer } from '../signaling/relais';
import { observateurDeSession } from '../signaling/trace';

export interface ServicePlateforme {
    port: number;
    close(): Promise<void>;
}

/// `base` est REQUISE, jamais optionnelle : un service qui apparierait sans
/// rien enregistrer serait indiscernable du bon fonctionnement (spec §6), et
/// c'est la classe exacte de panne muette contre laquelle tout ce dépôt est
/// écrit. `demarrage.ts` garantit par ailleurs que le port ne s'ouvre qu'après
/// la base et ses migrations.
export async function demarrerServeur(config: Config, base: Pilote): Promise<ServicePlateforme> {
    // Toute route HTTP répond 404 : P2 (authentification) et P4
    // (orchestration) en ajouteront, P1 n'en sert aucune.
    const http: Server = createServer((_requete, reponse) => {
        reponse.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
        reponse.end('introuvable\n');
    });

    const wssRacine = new WebSocketServer({ noServer: true });
    // `Date.now` est passée ICI, et une seule fois : c'est le seul endroit du
    // chemin de la trace qui lise une horloge réelle, tout le reste la reçoit.
    const relais = createSignalingServer(wssRacine, observateurDeSession(base, Date.now));

    http.on('upgrade', (requete, socket, tete) => {
        // `requete.url` peut porter une chaîne de requête ; seul le chemin
        // décide du routage.
        const chemin = new URL(requete.url ?? '/', 'http://placeholder').pathname;
        if (chemin !== '/') {
            // Refus explicite AVANT toute montée : le pair reçoit un 404 HTTP
            // et son socket se ferme, plutôt que de rester ouvert sur un
            // service qui ne l'écoutera jamais.
            socket.write('HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n');
            socket.destroy();
            return;
        }
        wssRacine.handleUpgrade(requete, socket, tete, (client) => {
            wssRacine.emit('connection', client, requete);
        });
    });

    await new Promise<void>((resolve, reject) => {
        http.once('error', reject);
        // Les DEUX arguments, toujours : sans `config.hote`, Node écoute sur
        // toutes les interfaces, ce que le critère ④ existe pour empêcher.
        http.listen(config.port, config.hote, () => {
            http.removeListener('error', reject);
            resolve();
        });
    });

    const adresse = http.address();
    const port = typeof adresse === 'object' && adresse ? adresse.port : config.port;

    return {
        port,
        async close(): Promise<void> {
            await relais.close();
            await new Promise<void>((resolve) => http.close(() => resolve()));
        },
    };
}
