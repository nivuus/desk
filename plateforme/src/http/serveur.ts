// Le serveur HTTP du service, et le routage de la montée WebSocket.
//
// `noServer` plutôt que `{ server }` : le routage du chemin est explicite.
// ✅ P3 Y A AJOUTÉ `/agent` SANS TOUCHER AU RELAIS — la branche d'abord, puis
// la BOUCLE du canal qu'elle sert (`agents/canal.ts`) —, ce que cette phrase
// annonçait : la branche est une seconde comparaison, et `wssRacine` n'a pas
// bougé d'une ligne. Avec `{ server }`, `ws` accepterait toute montée sur tout
// chemin — c'est le comportement d'avant P1 (le `new WebSocketServer({ port })`
// de l'ex-`server.ts`, qui ne posait ni `host` ni `path`), et il n'est pas
// extensible.
//
// ⚠️ DEUX CHEMINS, DEUX COMPARAISONS, PAS DE TABLE DE ROUTAGE. Une table pour
// deux entrées serait de l'abstraction non payée, et elle rendrait moins
// visible ce qui compte ici : le refus par DÉFAUT. Tout chemin qui n'est ni
// `/` ni `/agent` reçoit un `404` et son socket se ferme — c'est une liste
// blanche, jamais une liste noire, et c'est ce qui fait qu'un chemin ajouté un
// jour par mégarde n'ouvre rien.
//
// ⚠️ Router sur `/` RESTREINT ce qui était accepté hier. C'est délibéré, et le
// tableau des émetteurs réels a été relevé avant de le décider : la page de
// session (`client/src/main.ts`), la page-shell (`client/src/shell-page.ts`)
// et l'agent (`agent/src/signaling.rs`) visent tous les trois le chemin
// racine, sans aucun composant de chemin. Aucun pair connu n'en est affecté.

import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http';
import { WebSocketServer } from 'ws';
import type { Config } from '../config';
import type { Pilote } from '../base/pilote';
import { garde } from '../identite/garde';
import { servirAuth } from './routes-auth';
import { servirVm } from './routes-vm';
import { servirSession } from './routes-session';
import { createSignalingServer } from '../signaling/relais';
import { ProprieteDeSession } from '../signaling/propriete';
import { observateurDeSession } from '../signaling/trace';
import { servirLeCanalAgent } from '../agents/canal';
import { RegistreAgents } from '../agents/registre';

/// Le chemin du canal plateforme <-> agent (P3). ⚠️ Il est comparé
/// EXACTEMENT : voir le routage plus bas.
const CHEMIN_AGENT = '/agent';

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
    // Les routeurs sont essayés DANS L'ORDRE ; si aucun ne reconnaît le
    // chemin, le 404 de P1 est conservé MOT POUR MOT. ⚠️ Ne pas changer son
    // corps : rien ne le testait avant P2, et le changer serait un effet de
    // bord non déclaré. `routes-auth.test.ts` le fige désormais.
    //
    // 🔴 LE CHAÎNAGE SE FAIT ICI, DANS UNE FONCTION LOCALE, ET LA FORME
    // `void … .then(servie => …).catch(…)` EST CONSERVÉE TELLE QUELLE. C'est
    // ce que ce fichier s'impose depuis P1 : le `.catch` est la seule chose qui
    // empêche une promesse rejetée dans un gestionnaire d'évènement Node
    // d'abattre tout le processus, et une réécriture de ce corps le perdrait
    // sans que rien ne le dise. Le diff sur le corps du `createServer` est
    // ainsi d'une seule ligne — l'appel remplacé.
    //
    // ⚠️ LES TROIS ROUTEURS PARTAGENT LEURS DÉPENDANCES, et `Date.now` est
    // passée ici comme à la garde, à la trace et au canal : aucun module du
    // service ne lit d'horloge lui-même. C'est ce qui rend la borne de
    // fraîcheur assertable sur une valeur exacte dans les tests de route.
    const deps = {
        base,
        secretJeton: config.secretJeton,
        origineClient: config.origineClient,
        maintenant: Date.now,
    };

    /// Essaie les routeurs dans l'ordre, et rend `false` si aucun n'a servi.
    ///
    /// ⚠️ L'ORDRE EST SIGNIFIANT MAIS NON CONTRAIGNANT ICI : les trois jeux de
    /// chemins sont DISJOINTS (`/auth/*`, `/vm*`, `/session`), et chacun compare
    /// exactement plutôt que par préfixe. Un `await` de plus ne coûte donc rien
    /// à personne — mais le jour où deux routeurs se disputeraient un chemin,
    /// c'est cet ordre qui trancherait, en silence.
    async function servirTout(
        requete: IncomingMessage,
        reponse: ServerResponse,
    ): Promise<boolean> {
        if (await servirAuth(requete, reponse, deps)) return true;
        if (await servirVm(requete, reponse, deps)) return true;
        return servirSession(requete, reponse, deps);
    }

    const http: Server = createServer((requete, reponse) => {
        void servirTout(requete, reponse)
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
                // ⚠️ LE LIBELLÉ NE NOMME PLUS « l'authentification » : depuis
                // P4 ce `catch` couvre les TROIS routeurs, et un message qui
                // désignerait le mauvais ferait chercher au mauvais endroit.
                // C'est la seule ligne de ce bloc que P4 change, et elle est
                // changée parce qu'elle serait devenue FAUSSE autrement.
                console.error(`route HTTP en échec : ${String(cause)}`);
                if (!reponse.headersSent) {
                    reponse.writeHead(500, { 'content-type': 'application/json; charset=utf-8' });
                    reponse.end(JSON.stringify({ refus: 'interne' }));
                }
            });
    });

    const wssRacine = new WebSocketServer({ noServer: true });
    // Le canal `/agent` (P3) : son propre `WebSocketServer`, qui ne partage
    // avec le relais ni garde, ni registre d'appartenance, ni observateur de
    // session. C'est la conséquence directe d'E4 : l'enrôlement est
    // ASYNCHRONE (il lit `agent_enrole`), et la garde du relais est PURE et
    // SYNCHRONE. Les faire cohabiter dans le même serveur obligerait l'un des
    // deux à céder.
    const wssAgent = new WebSocketServer({ noServer: true });
    // La garde est construite ICI, à partir du secret de configuration, et
    // c'est le SEUL endroit du service qui en fabrique une. Elle est REQUISE
    // par le relais : il n'existe aucun chemin qui produise une garde ouverte
    // hors d'un test, `PLATEFORME_SECRET_JETON` n'ayant aucun défaut.
    //
    // Le registre d'appartenance vit ici aussi, donc pour la durée du service.
    // Son coût — il ne survit pas à un redémarrage — est écrit dans
    // `signaling/propriete.ts`.
    const gardeDuService = garde(config.secretJeton, Date.now, new ProprieteDeSession());
    // Le registre des sockets d'agent vivants, construit UNE FOIS et partagé
    // entre le canal (qui y inscrit) et les routes (qui y lancent). C'est le
    // seul endroit du service qui en fabrique un.
    //
    // ⚠️ IL A LE MÊME COÛT QUE `ProprieteDeSession`, ET IL EST NOMMÉ AU MÊME
    // ENDROIT : il ne survit pas à un redémarrage. Après un redémarrage, aucun
    // agent n'y figure tant qu'il ne s'est pas ré-enrôlé, et tout lancement
    // rend `agent-injoignable` — bruyamment. Le rattrapage est la reconnexion
    // de l'agent, qui le reconstitue sans que personne ne le persiste.
    const registreAgents = new RegistreAgents();
    // 🔴 LE CANAL EST BRANCHÉ ICI, ET C'EST LA SEULE LIGNE QUI LE FAIT VIVRE.
    // Sans elle, `wssAgent` accepterait toujours la montée sur `/agent` et
    // n'écouterait RIEN : le pair verrait une connexion réussie, puis un
    // silence — la panne muette exacte que ce fichier invoque déjà pour rendre
    // `base` REQUISE. `canal.test.ts` la tient par un test dédié.
    //
    // `Date.now` est passée ici, comme à la garde et à la trace : les trois la
    // reçoivent de cette fonction, et aucun module du service ne lit d'horloge
    // lui-même. C'est ce qui rend l'expiration d'un jeton d'agent assertable
    // sur une valeur EXACTE dans `canal.test.ts`.
    servirLeCanalAgent(wssAgent, {
        base,
        secretJeton: config.secretJeton,
        maintenant: Date.now,
        registre: registreAgents,
    });

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
        // Comparaison EXACTE, jamais un `startsWith` : `/agentaire` n'est pas
        // `/agent`, et un préfixe ouvrirait une famille entière de chemins que
        // personne n'a décidés.
        const wss = chemin === '/' ? wssRacine : chemin === CHEMIN_AGENT ? wssAgent : undefined;
        if (wss === undefined) {
            // Refus explicite AVANT toute montée : le pair reçoit un 404 HTTP
            // et son socket se ferme, plutôt que de rester ouvert sur un
            // service qui ne l'écoutera jamais.
            socket.write('HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n');
            socket.destroy();
            return;
        }
        wss.handleUpgrade(requete, socket, tete, (client) => {
            wss.emit('connection', client, requete);
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
            // ⚠️ Le second serveur se ferme AUSSI, et explicitement. Un
            // `WebSocketServer` en `noServer` ne s'arrête pas avec le serveur
            // HTTP : ses sockets déjà montés survivraient, et `close()`
            // rendrait la main sur un service qui écoute encore.
            await new Promise<void>((resolve) => wssAgent.close(() => resolve()));
            await new Promise<void>((resolve) => http.close(() => resolve()));
        },
    };
}
