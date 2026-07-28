import { attachInput } from './input';
import { connectSession } from './webrtc';
import { attachStats } from './stats';
import { armerLeSon } from './audio';
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

// Minuteur du bandeau audio (« cliquez pour activer le son ») et indicateur
// de fin de session, partagés entre `onControl` (câblé avant que la promesse
// de connexion résolve) et le `.then()` où `armerLeSon` est appelée (après).
// Le bandeau audio est le message le MOINS important de l'interface : il ne
// doit jamais reprendre la main sur un message terminal comme
// « session terminée ». D'où `sessionTerminee`, qui coupe le bandeau à la
// racine plutôt que de le laisser s'afficher puis se faire recouvrir — et qui
// couvre aussi le cas où `session-end` arrive avant même que `armerLeSon`
// n'ait été appelée (le minuteur n'existe alors pas encore : `bandeau` reste
// `undefined`, et `clearTimeout(undefined)` ne fait rien).
let bandeau: number | undefined;
let sessionTerminee = false;

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
            sessionTerminee = true;
            window.clearTimeout(bandeau);
            setStatus(`session terminée : ${message.reason}`);
        }
    },
})
    .then((session) => {
        attachInput({ video, channel: session.inputChannel });
        attachStats(session.pc, statsElement);
        video.focus();

        // Le son démarre coupé et s'active au premier geste. Un bandeau ne
        // s'affiche que si aucun geste n'est venu au bout de quelques
        // secondes — inutile d'expliquer à qui a déjà cliqué. Ni le geste ni
        // le minuteur ne doivent agir une fois la session terminée : voir
        // `sessionTerminee` ci-dessus.
        armerLeSon({
            media: video,
            cible: window,
            surEtat(actif) {
                if (sessionTerminee) return;
                if (actif) {
                    window.clearTimeout(bandeau);
                    statusElement.dataset.hidden = 'true';
                } else {
                    bandeau = window.setTimeout(() => {
                        if (sessionTerminee) return;
                        setStatus('cliquez pour activer le son');
                    }, 4000);
                }
            },
        });

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
