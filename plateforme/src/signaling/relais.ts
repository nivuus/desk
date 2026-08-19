// Serveur de signaling : met en relation un agent et un client par session et
// relaie l'offre et la réponse SDP. Aucun état persistant, aucune authentification
// (jalon 1, réseau local).

import { WebSocket, WebSocketServer } from 'ws';
import { Appariement, isRole, type Role } from './appariement';
import { configurationIce } from './ice';

// Types que le serveur relaie au pair. Tout le reste est refusé — un relais
// qui accepterait n'importe quoi deviendrait un canal de diffusion arbitraire
// sur un serveur sans authentification.
//
// `fenetre-ouverte`, `fenetre-fermee`, `refus` et `viewport` portent la
// session de contrôle du sous-bloc D1, entre le superviseur (rôle `agent`) et
// la page-shell (rôle `client`).
const TYPES_RELAYES = new Set([
    'offer',
    'answer',
    'fenetre-ouverte',
    'fenetre-fermee',
    'refus',
    'viewport',
]);

// Garde de type : un message JSON valide peut être `null`, un nombre, une chaîne
// ou un tableau (tous acceptés par JSON.parse), pas seulement un objet
// `{role, session}` ou `{type, sdp}`. `null` est le cas dangereux : contrairement
// aux nombres/chaînes/tableaux (dont l'accès de propriété retourne simplement
// `undefined` par auto-boxing), `null.role` lève une TypeError. Comme ce code
// tourne dans un handler d'événement `message` d'un WebSocket exposé sans
// authentification, une TypeError non interceptée y est fatale : elle abat tout
// le process Node (aucun `uncaughtException` n'est installé dans index.ts), donc
// toutes les sessions actives avec elle. On rejette explicitement tout ce qui
// n'est pas un objet simple avant d'accéder à la moindre propriété.
function isJsonObject(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export interface SignalingServer {
    port: number;
    close(): Promise<void>;
}

// Deux formes, à dessein. La forme `port` est celle qu'éprouve
// `server.test.ts` depuis le jalon 1 : la garder intacte est ce qui permet de
// dire que le déménagement du sous-bloc P1 n'a rien changé au relais. La forme
// `wss` est celle qu'emploie le service, où le serveur HTTP possède le port.
export function createSignalingServer(port: number): SignalingServer;
export function createSignalingServer(wss: WebSocketServer): SignalingServer;
export function createSignalingServer(portOuWss: number | WebSocketServer): SignalingServer {
    const port = typeof portOuWss === 'number' ? portOuWss : 0;
    const wss = typeof portOuWss === 'number' ? new WebSocketServer({ port }) : portOuWss;
    const sessions = new Appariement<WebSocket>();

    function send(socket: WebSocket | undefined, payload: unknown): void {
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify(payload));
        }
    }

    wss.on('connection', (socket) => {
        let role: Role | undefined;
        let sessionId: string | undefined;

        socket.on('message', (raw) => {
            let message: unknown;
            try {
                message = JSON.parse(raw.toString());
            } catch {
                send(socket, { type: 'error', reason: 'JSON invalide' });
                return;
            }

            // Rejet avant toute lecture de propriété : voir `isJsonObject` ci-dessus.
            // Le pair fautif reçoit une erreur mais sa connexion reste ouverte, pour
            // qu'il puisse retenter avec un message valide.
            if (!isJsonObject(message)) {
                send(socket, {
                    type: 'error',
                    reason: role
                        ? 'message invalide : objet JSON attendu'
                        : 'premier message invalide : {role, session} attendu',
                });
                return;
            }

            // Premier message : déclaration de rôle et de session.
            if (!role) {
                const declaredRole = message.role;
                const declaredSession = message.session;
                if (
                    !isRole(declaredRole) ||
                    typeof declaredSession !== 'string' ||
                    declaredSession.length === 0
                ) {
                    send(socket, {
                        type: 'error',
                        reason: 'premier message invalide : {role, session} attendu',
                    });
                    return;
                }

                const refus = sessions.declarer(declaredSession, declaredRole, socket);
                if (refus) {
                    send(socket, { type: 'error', reason: refus });
                    return;
                }

                role = declaredRole;
                sessionId = declaredSession;

                // Configuration ICE : envoyée à CHAQUE pair dès qu'il se
                // déclare, agent comme client. Les deux en ont besoin — le
                // relais TURN n'est utile que si les deux extrémités peuvent
                // l'employer.
                //
                // `Date.now()` est lu ici et non dans `configurationIce` :
                // cette dernière reste ainsi une fonction pure, testable
                // avec un instant fixé.
                const ice = configurationIce(process.env, declaredSession, Date.now());
                if (ice) {
                    send(socket, { type: 'ice-config', ...ice });
                } else {
                    // Trace explicite : une session sans relais qui échoue à
                    // se connecter depuis l'extérieur doit pouvoir être
                    // diagnostiquée sans relire le code.
                    console.warn(
                        'aucun serveur TURN configuré (TURN_URL/TURN_SECRET) : session sans relais',
                    );
                }

                // Une offre arrivée avant cet agent l'attend : la lui remettre
                // maintenant, sinon elle ne partira jamais.
                if (declaredRole === 'agent') {
                    const offre = sessions.prendreOffre(declaredSession);
                    if (offre) send(socket, { type: 'offer', sdp: offre });
                }
                return;
            }

            // Messages suivants : relais vers le pair.
            const peer = sessions.pair(sessionId!, role);

            if (TYPES_RELAYES.has(message.type as string)) {
                if (message.type === 'offer' && !peer) {
                    // Pas d'agent en face : on retient, plutôt que de perdre.
                    sessions.retenirOffre(sessionId!, message.sdp as string);
                    return;
                }
                send(peer, message);
            } else {
                send(socket, { type: 'error', reason: `type inconnu : ${message.type}` });
            }
        });

        socket.on('close', () => {
            if (!role || !sessionId) return;
            const peer = sessions.pair(sessionId, role);
            sessions.retirer(sessionId, role);
            send(peer, { type: 'peer-gone' });
        });
    });

    return {
        get port(): number {
            const address = wss.address();
            return typeof address === 'object' && address ? address.port : port;
        },
        close(): Promise<void> {
            return new Promise((resolve) => {
                for (const socket of wss.clients) socket.terminate();
                wss.close(() => resolve());
            });
        },
    };
}
