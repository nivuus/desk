// Serveur de signaling : met en relation un agent et un client par session et
// relaie l'offre et la réponse SDP. Aucun état persistant, aucune authentification
// (jalon 1, réseau local).

import { WebSocket, WebSocketServer } from 'ws';

type Role = 'agent' | 'client';

interface Session {
    agent?: WebSocket;
    client?: WebSocket;
}

// Garde de type : nécessaire pour que TypeScript affine `message.role` (typé
// `any`) en `Role` et autorise l'indexation de `Session` sous `strict`.
function isRole(value: unknown): value is Role {
    return value === 'agent' || value === 'client';
}

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

export function createSignalingServer(port: number): SignalingServer {
    const wss = new WebSocketServer({ port });
    const sessions = new Map<string, Session>();

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

                const session = sessions.get(declaredSession) ?? {};
                if (session[declaredRole]) {
                    send(socket, {
                        type: 'error',
                        reason: `un ${declaredRole} est déjà connecté à la session ${declaredSession}`,
                    });
                    return;
                }

                role = declaredRole;
                sessionId = declaredSession;
                session[declaredRole] = socket;
                sessions.set(declaredSession, session);
                return;
            }

            // Messages suivants : relais vers le pair.
            const session = sessions.get(sessionId!);
            if (!session) return;
            const peer = role === 'client' ? session.agent : session.client;

            if (message.type === 'offer' || message.type === 'answer') {
                send(peer, { type: message.type, sdp: message.sdp });
            } else {
                send(socket, { type: 'error', reason: `type inconnu : ${message.type}` });
            }
        });

        socket.on('close', () => {
            if (!role || !sessionId) return;
            const session = sessions.get(sessionId);
            if (!session) return;

            delete session[role];
            const peer = role === 'client' ? session.agent : session.client;
            send(peer, { type: 'peer-gone' });

            if (!session.agent && !session.client) {
                sessions.delete(sessionId);
            }
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
