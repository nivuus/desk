import { attachInput } from './input';
import { connectSession } from './webrtc';
import { attachStats } from './stats';
import { encodeResize } from '../../proto/ts/control';

const video = document.querySelector<HTMLVideoElement>('#remote')!;
const statusElement = document.querySelector<HTMLDivElement>('#status')!;
const statsElement = document.querySelector<HTMLDivElement>('#stats')!;

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
})
    .then((session) => {
        attachInput({ video, channel: session.inputChannel });
        attachStats(session.pc, statsElement);
        video.focus();

        // Le redimensionnement reconstruit la chaîne d'encodage côté agent :
        // on n'émet donc qu'une fois le geste terminé, pas à chaque pixel
        // parcouru pendant que l'utilisateur tire un bord.
        let resizeTimer: number | undefined;
        const observer = new ResizeObserver(() => {
            window.clearTimeout(resizeTimer);
            resizeTimer = window.setTimeout(() => {
                if (session.controlChannel.readyState !== 'open') return;
                const width = Math.round(video.clientWidth * window.devicePixelRatio);
                const height = Math.round(video.clientHeight * window.devicePixelRatio);
                session.controlChannel.send(encodeResize(width, height));
            }, 200);
        });
        observer.observe(video);
    })
    .catch((error: unknown) => {
        setStatus(`échec : ${error instanceof Error ? error.message : String(error)}`);
    });
