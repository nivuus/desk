import { connectSession } from './webrtc';

const video = document.querySelector<HTMLVideoElement>('#remote')!;
const statusElement = document.querySelector<HTMLDivElement>('#status')!;

function setStatus(message: string): void {
    statusElement.textContent = message;
    statusElement.dataset.hidden = 'false';
}

// La session et le signaling sont paramétrables par l'URL pour faciliter les
// essais : ?session=demo&signaling=ws://192.168.3.2:8080
const params = new URLSearchParams(window.location.search);
const sessionId = params.get('session') ?? 'demo';
const signalingUrl =
    params.get('signaling') ?? `ws://${window.location.hostname}:8080`;

connectSession({
    signalingUrl,
    sessionId,
    video,
    onStatus: setStatus,
    onControl(message) {
        if (message.type === 'ready') {
            setStatus(`prêt — ${message.width}×${message.height}`);
            setTimeout(() => {
                statusElement.dataset.hidden = 'true';
            }, 1500);
        } else if (message.type === 'session-end') {
            setStatus(`session terminée : ${message.reason}`);
        }
    },
}).catch((error: unknown) => {
    setStatus(`échec : ${error instanceof Error ? error.message : String(error)}`);
});
