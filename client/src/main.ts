import { attachInput } from './input';
import { connectSession } from './webrtc';
import { attachStats } from './stats';
import { armerLeSon } from './audio';
import { creerStatut } from './status';
import { attachPointerAuDOM } from './pointer';
import { attachGamepadAuDOM } from './gamepad';
import { attachFullscreenAuDOM } from './fullscreen';
import { encodeResize } from '../../proto/ts/control';

const video = document.querySelector<HTMLVideoElement>('#remote')!;
const statusElement = document.querySelector<HTMLDivElement>('#status')!;
const statsElement = document.querySelector<HTMLDivElement>('#stats')!;
const fullscreenElement = document.querySelector<HTMLButtonElement>('#fullscreen')!;

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

// Comme `bandeau` ci-dessus : `onControl` est câblé avant que la promesse de
// `connectSession` résolve, donc ces variables doivent exister avant l'appel,
// sous peine d'être dans la zone morte temporelle au premier message reçu.
let pointeur: ReturnType<typeof attachPointerAuDOM> | undefined;
let manette: ReturnType<typeof attachGamepadAuDOM> | undefined;
let manetteAnnoncee = false;
let bandeauManette: number | undefined;

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
            window.clearTimeout(bandeauManette);
            statut.afficher(`session terminée : ${message.reason}`, { terminal: true });
        } else if (message.type === 'pointer') {
            pointeur?.surMessagePointeur(message.visible, message.shape);
        } else if (message.type === 'rumble') {
            manette?.surVibration(message.left, message.right);
        } else if (message.type === 'capabilities') {
            // `gamepad: false` signifie que la machine distante ne peut offrir
            // AUCUNE manette, pas que le client n'en a pas branché : un
            // message distinct de celui du bandeau manette ci-dessous, sans
            // quoi l'utilisateur croirait sa manette en cause.
            if (!message.gamepad) {
                statut.afficher('manette indisponible sur cette machine');
                setTimeout(() => statut.masquer(), 4000);
            }
        }
    },
})
    .then((session) => {
        attachInput({ video, channel: session.inputChannel });
        attachStats(session.pc, statsElement);
        video.focus();

        const envoyer = (payload: Uint8Array): void => {
            if (session.inputChannel.readyState === 'open') {
                // Même assertion que dans input.ts : `RTCDataChannel.send`
                // exige un `Uint8Array<ArrayBuffer>`, or les tampons produits
                // par `proto/ts/input.ts` sont toujours adossés à un vrai
                // `ArrayBuffer` en pratique — seul le typage est trop large.
                session.inputChannel.send(payload as Uint8Array<ArrayBuffer>);
            }
        };

        pointeur = attachPointerAuDOM({
            envoyer,
            surEchec: () => statut.afficher('cliquez dans l\'image pour prendre la souris'),
        });

        manette = attachGamepadAuDOM({
            envoyer,
            surPresence: (present) => {
                if (present && !manetteAnnoncee) {
                    manetteAnnoncee = true;
                    window.clearTimeout(bandeauManette);
                    statut.afficher('manette détectée');
                    setTimeout(() => statut.masquer(), 1500);
                }
            },
        });

        // La Gamepad API n'expose AUCUNE manette avant un appui sur l'une de
        // ses touches : une manette branchée et silencieuse est indiscernable
        // d'une absence de manette. On le dit, plutôt que de laisser conclure
        // à une panne — même patron que le bandeau audio ci-dessous, y compris
        // le délai : inutile de l'expliquer à qui a déjà appuyé.
        bandeauManette = window.setTimeout(() => {
            if (!manetteAnnoncee) statut.afficher('manette : appuyez sur un bouton pour l\'activer');
        }, 4000);

        attachFullscreenAuDOM({ bouton: fullscreenElement, cible: document.documentElement });

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
