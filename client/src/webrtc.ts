// Établissement de la session WebRTC. Le navigateur est l'offrant : il déclare
// la piste vidéo en réception seule et les deux canaux de données, puis attend
// la réponse de l'agent relayée par le signaling.

import { parseAgentControl, type AgentControl } from '../../proto/ts/control';

export interface SessionOptions {
    signalingUrl: string;
    sessionId: string;
    video: HTMLVideoElement;
    onControl?: (message: AgentControl) => void;
    onStatus?: (message: string) => void;
}

export interface SessionHandle {
    pc: RTCPeerConnection;
    inputChannel: RTCDataChannel;
    controlChannel: RTCDataChannel;
    close(): void;
}

// Délai maximal d'attente de la réponse de l'agent, après l'envoi de
// l'offre. Si aucun agent n'est connecté à la session demandée, le serveur
// de signaling relaie l'offre vers un pair inexistant et ne renvoie jamais
// rien au client : sans ce délai, la promesse d'attente ne se résoudrait
// jamais et l'utilisateur resterait bloqué indéfiniment.
const ANSWER_TIMEOUT_MS = 15_000;

/// Attend le SDP de réponse de l'agent, relayé par le socket de signaling.
/// Rejette si : un message d'erreur ou « peer-gone » est reçu, le socket se
/// ferme avant la réponse, ou le délai maximal est dépassé. Un message JSON
/// illisible est journalisé et ignoré plutôt que de faire planter l'attente
/// (avec une exception non interceptée) : d'autres messages valides peuvent
/// encore arriver, notamment la réponse elle-même.
function waitForAnswer(socket: WebSocket): Promise<string> {
    return new Promise((resolve, reject) => {
        let settled = false;

        // Quelle que soit l'issue (succès, erreur, fermeture, délai), les
        // écouteurs et le minuteur doivent être retirés une seule fois : pas
        // de fuite, pas de résolution/rejet en double.
        const finish = (action: () => void) => {
            if (settled) return;
            settled = true;
            socket.removeEventListener('message', onMessage);
            socket.removeEventListener('close', onClose);
            clearTimeout(timer);
            action();
        };

        const onMessage = (event: MessageEvent) => {
            let message;
            try {
                message = JSON.parse(String(event.data));
            } catch (error) {
                console.warn('message de signaling illisible, ignoré', error);
                return;
            }
            if (message.type === 'answer') {
                finish(() => resolve(message.sdp));
            } else if (message.type === 'error') {
                finish(() => reject(new Error(message.reason ?? 'erreur de signaling')));
            } else if (message.type === 'peer-gone') {
                finish(() => reject(new Error('agent déconnecté')));
            }
        };

        const onClose = () => {
            finish(() =>
                reject(new Error("connexion au serveur de signaling perdue avant la réponse de l'agent")),
            );
        };

        const timer = setTimeout(() => {
            // Message orienté utilisateur : pas de détail interne (pas de
            // mention du serveur de signaling ni du protocole), juste de
            // quoi diagnostiquer sans recharger la page à l'aveugle.
            finish(() =>
                reject(
                    new Error(
                        "l'agent n'a pas répondu — vérifiez qu'il est bien lancé et connecté à cette session",
                    ),
                ),
            );
        }, ANSWER_TIMEOUT_MS);

        socket.addEventListener('message', onMessage);
        socket.addEventListener('close', onClose);
    });
}

/// Attend que la collecte ICE soit terminée : sans trickle, le SDP doit déjà
/// contenir tous les candidats.
function waitForIceGathering(pc: RTCPeerConnection): Promise<void> {
    if (pc.iceGatheringState === 'complete') return Promise.resolve();
    return new Promise((resolve) => {
        const check = () => {
            if (pc.iceGatheringState === 'complete') {
                pc.removeEventListener('icegatheringstatechange', check);
                resolve();
            }
        };
        pc.addEventListener('icegatheringstatechange', check);
        // Filet de sécurité : ne jamais bloquer indéfiniment sur un réseau lent.
        setTimeout(() => {
            pc.removeEventListener('icegatheringstatechange', check);
            resolve();
        }, 3000);
    });
}

export async function connectSession(options: SessionOptions): Promise<SessionHandle> {
    const status = options.onStatus ?? (() => {});
    // Réseau local : aucun serveur STUN/TURN nécessaire au jalon 1.
    const pc = new RTCPeerConnection({ iceServers: [] });

    pc.addTransceiver('video', { direction: 'recvonly' });

    // Entrées : non fiable et non ordonné — une position de souris périmée n'a
    // aucune valeur, mieux vaut la perdre que retarder les suivantes.
    const inputChannel = pc.createDataChannel('input', {
        ordered: false,
        maxRetransmits: 0,
    });
    const controlChannel = pc.createDataChannel('control', { ordered: true });

    controlChannel.addEventListener('message', (event) => {
        try {
            options.onControl?.(parseAgentControl(String(event.data)));
        } catch (error) {
            console.warn('message de contrôle invalide', error);
        }
    });

    pc.addEventListener('track', (event) => {
        options.video.srcObject = event.streams[0] ?? new MediaStream([event.track]);
        status('flux reçu');
    });

    pc.addEventListener('connectionstatechange', () => {
        status(`connexion : ${pc.connectionState}`);
    });

    const socket = new WebSocket(options.signalingUrl);
    await new Promise<void>((resolve, reject) => {
        socket.addEventListener('open', () => resolve(), { once: true });
        socket.addEventListener('error', () => reject(new Error('signaling injoignable')), {
            once: true,
        });
    });
    socket.send(JSON.stringify({ role: 'client', session: options.sessionId }));

    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    await waitForIceGathering(pc);

    status('offre envoyée, attente de l\'agent…');
    socket.send(JSON.stringify({ type: 'offer', sdp: pc.localDescription!.sdp }));

    const answerSdp = await waitForAnswer(socket);
    await pc.setRemoteDescription({ type: 'answer', sdp: answerSdp });
    status('réponse reçue');

    return {
        pc,
        inputChannel,
        controlChannel,
        close() {
            socket.close();
            pc.close();
        },
    };
}
