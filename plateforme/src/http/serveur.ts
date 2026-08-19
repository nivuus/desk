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
import { garde } from '../identite/garde';
import { servirAuth } from './routes-auth';
import { createSignalingServer } from '../signaling/relais';
import { ProprieteDeSession } from '../signaling/propriete';
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
    // Les routes d'authentification d'abord ; si elles ne reconnaissent pas
    // le chemin, le 404 de P1 est conservé MOT POUR MOT. ⚠️ Ne pas changer son
    // corps : rien ne le testait avant P2, et le changer serait un effet de
    // bord non déclaré. `routes-auth.test.ts` le fige désormais.
    const http: Server = createServer((requete, reponse) => {
        void servirAuth(requete, reponse, {
            base,
            secretJeton: config.secretJeton,
            origineClient: config.origineClient,
            maintenant: Date.now,
        })
            .then((servie) => {
                if (servie) return;
                reponse.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
                reponse.end('introuvable\n');
            })
            .catch((cause) => {
                // Une promesse rejetée sans `catch` dans un gestionnaire
                // d'évènement Node abat tout le process — c'est le mode de
                // défaillance que `signaling/relais.ts` documente déjà. La
                // cause est journalisée SANS le corps de la requête, qui
                // porterait le mot de passe (critère ④).
                console.error(`route d'authentification en échec : ${String(cause)}`);
                if (!reponse.headersSent) {
                    reponse.writeHead(500, { 'content-type': 'application/json; charset=utf-8' });
                    reponse.end(JSON.stringify({ refus: 'interne' }));
                }
            });
    });

    const wssRacine = new WebSocketServer({ noServer: true });
    // La garde est construite ICI, à partir du secret de configuration, et
    // c'est le SEUL endroit du service qui en fabrique une. Elle est REQUISE
    // par le relais : il n'existe aucun chemin qui produise une garde ouverte
    // hors d'un test, `PLATEFORME_SECRET_JETON` n'ayant aucun défaut.
    //
    // Le registre d'appartenance vit ici aussi, donc pour la durée du service.
    // Son coût — il ne survit pas à un redémarrage — est écrit dans
    // `signaling/propriete.ts`.
    const gardeDuService = garde(config.secretJeton, Date.now, new ProprieteDeSession());
    // `Date.now` est passée ICI, et une seule fois pour la trace : c'est le
    // seul endroit du chemin de la trace qui lise une horloge réelle, tout le
    // reste la reçoit.
    const relais = createSignalingServer(
        wssRacine,
        gardeDuService,
        observateurDeSession(base, Date.now),
    );

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
