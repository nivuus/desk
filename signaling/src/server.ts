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
            let message: any;
            try {
                message = JSON.parse(raw.toString());
            } catch {
                send(socket, { type: 'error', reason: 'JSON invalide' });
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
