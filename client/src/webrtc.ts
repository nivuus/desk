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

    const answerReceived = new Promise<string>((resolve, reject) => {
        socket.addEventListener('message', (event) => {
            const message = JSON.parse(String(event.data));
            if (message.type === 'answer') resolve(message.sdp);
            else if (message.type === 'error') reject(new Error(message.reason));
            else if (message.type === 'peer-gone') reject(new Error('agent déconnecté'));
        });
    });

    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    await waitForIceGathering(pc);

    status('offre envoyée, attente de l\'agent…');
    socket.send(JSON.stringify({ type: 'offer', sdp: pc.localDescription!.sdp }));

    const answerSdp = await answerReceived;
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
