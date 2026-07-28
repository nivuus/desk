import { attachInput } from './input';
import { connectSession } from './webrtc';
import { attachStats } from './stats';
import { armerLeSon } from './audio';
import { creerStatut } from './status';
import { encodeResize } from '../../proto/ts/control';

const video = document.querySelector<HTMLVideoElement>('#remote')!;
const statusElement = document.querySelector<HTMLDivElement>('#status')!;
const statsElement = document.querySelector<HTMLDivElement>('#stats')!;

// Point d'écriture unique du bandeau de statut : protège un message TERMINAL
// (fin de session, échec) contre l'écrasement par un message ordinaire
// arrivant après lui. Voir `status.ts` pour la justification complète.
const statut = creerStatut(statusElement);

// La session et le signaling sont paramétrables par l'URL pour faciliter les
// essais : ?session=demo&signaling=ws://192.168.3.2:8080
const params = new URLSearchParams(window.location.search);
const sessionId = params.get('session') ?? 'demo';
const signalingUrl =
    params.get('signaling') ?? `ws://${window.location.hostname}:8080`;

// Minuteur du bandeau audio (« cliquez pour activer le son »), partagé entre
// `onControl` (câblé avant que la promesse de connexion résolve) et le
// `.then()` où `armerLeSon` est appelée (après). Il n'a plus besoin d'être
// gardé par un indicateur de fin de session : `statut` porte cette garde à
// la racine, pour tous les écrivains. On l'annule tout de même à la fin de
// session pour ne pas laisser un minuteur obsolète courir pour rien.
let bandeau: number | undefined;

connectSession({
    signalingUrl,
    sessionId,
    video,
    onStatus: (message) => statut.afficher(message),
    onControl(message) {
        if (message.type === 'ready') {
            statut.afficher(`prêt — ${message.width}×${message.height}`);
            setTimeout(() => statut.masquer(), 1500);
        } else if (message.type === 'session-end') {
            window.clearTimeout(bandeau);
            statut.afficher(`session terminée : ${message.reason}`, { terminal: true });
        }
    },
})
    .then((session) => {
        attachInput({ video, channel: session.inputChannel });
        attachStats(session.pc, statsElement);
        video.focus();

        // Le son démarre coupé et s'active au premier geste. Un bandeau ne
        // s'affiche que si aucun geste n'est venu au bout de quelques
        // secondes — inutile d'expliquer à qui a déjà cliqué.
        armerLeSon({
            media: video,
            cible: window,
            surEtat(actif) {
                if (actif) {
                    window.clearTimeout(bandeau);
                    statut.masquer();
                } else {
                    bandeau = window.setTimeout(() => {
                        statut.afficher('cliquez pour activer le son');
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
        statut.afficher(`échec : ${error instanceof Error ? error.message : String(error)}`, {
            terminal: true,
        });
    });
