import { attachInput } from './input';
import { connectSession } from './webrtc';
import { attachStats } from './stats';
import { armerLeSon } from './audio';
import { creerStatut } from './status';
import { attachPointerAuDOM } from './pointer';
import { attachGamepadAuDOM } from './gamepad';
import { armerPleinEcranAuDOM, attachFullscreenAuDOM } from './fullscreen';
import { texteLien } from './lien';
import { viewportPair } from './viewport';
import { attachVisibilite } from './visibilite';
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

// Annonce du viewport à la page-shell qui nous a ouverts.
//
// C'est cette taille qui décide de la résolution de la sortie virtuelle, donc
// de la résolution native du flux : rien ne peut être créé côté agent avant
// qu'elle soit connue. L'annonce part donc AVANT toute connexion WebRTC.
//
// `window.opener` est nul quand la page est ouverte à la main (essais,
// rechargement direct) : dans ce cas l'agent tourne déjà et il n'y a rien à
// demander — on ne fait rien plutôt que d'échouer.
if (window.opener && !window.opener.closed) {
    const { largeur, hauteur } = viewportPair(window.innerWidth, window.innerHeight);
    window.opener.postMessage(
        { type: 'viewport', session: sessionId, largeur, hauteur },
        window.location.origin,
    );
}

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
let detacherPleinEcran: ReturnType<typeof attachFullscreenAuDOM> | undefined;
let detacherArmement: (() => void) | undefined;
let detacherVisibilite: ReturnType<typeof attachVisibilite> | undefined;
let manetteAnnoncee = false;
let bandeauManette: number | undefined;

// Minuteur du bandeau réseau : seul un message SANS alerte s'auto-masque
// (même patron que le bandeau « prêt » ci-dessous). Un message d'alerte reste
// affiché tant que la condition dure ; l'annuler avant d'en armer un nouveau
// évite qu'un masquage obsolète n'efface un avertissement arrivé entre-temps.
let bandeauLien: number | undefined;

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
            window.clearTimeout(bandeauLien);
            // Sans ces trois détachements, le `setInterval` à 4 ms de la
            // manette (et les écouteurs de pointeur/plein écran) continuent
            // de tourner après la fin de session — rien d'autre ne les
            // arrête, la page reste ouverte tant que l'utilisateur ne la
            // ferme pas lui-même.
            pointeur?.detacher();
            manette?.detacher();
            detacherPleinEcran?.();
            detacherArmement?.();
            detacherVisibilite?.();
            statut.afficher(`session terminée : ${message.reason}`, { terminal: true });
        } else if (message.type === 'pointer') {
            pointeur?.surMessagePointeur(message.visible, message.shape);
        } else if (message.type === 'rumble') {
            manette?.surVibration(message.left, message.right);
        } else if (message.type === 'asleep') {
            if (message.asleep) {
                const texte =
                    message.reason === 'evincee'
                        ? 'image figée : trop de fenêtres actives'
                        : 'image figée : fenêtre masquée';
                // `persistant` : l'état dure tant que la fenêtre dort, il ne
                // doit pas être effacé par la minuterie d'un bandeau voisin.
                statut.afficher(texte, { persistant: true });
            } else {
                // `masquer()` protège délibérément un message persistant : le
                // réveil doit donc lever explicitement cette persistance,
                // sans quoi le bandeau « image figée : … » resterait affiché
                // pour toujours après le réveil réel (voir status.ts).
                statut.expirer();
            }
        } else if (message.type === 'fullscreen') {
            // Sens UNIQUE : l'application Windows décide, le navigateur suit.
            // Sortir n'exige aucune activation utilisateur ; entrer, si — d'où
            // l'armement.
            detacherArmement?.();
            detacherArmement = undefined;
            if (message.active) {
                detacherArmement = armerPleinEcranAuDOM(document.documentElement);
            } else {
                void document.exitFullscreen().catch(() => {
                    // Sortir d'un plein écran qu'on n'a pas est sans
                    // conséquence : l'utilisateur a pu en sortir lui-même.
                });
            }
        } else if (message.type === 'link') {
            const t = texteLien(message);
            window.clearTimeout(bandeauLien);
            if (t.alerte) {
                // `persistant` protège ce message contre le `masquer()` d'une
                // minuterie VOISINE (bandeau « prêt », « manette détectée »,
                // etc.) dont ce module n'a — et ne doit pas avoir — à
                // connaître l'existence. Sans quoi une alerte affichée dans
                // la fenêtre de tir d'un de ces bandeaux disparaîtrait alors
                // que le réseau est toujours dégradé. Le bandeau de statut
                // protège aussi déjà les messages terminaux : un
                // avertissement réseau n'écrasera pas une fin de session.
                statut.afficher(t.resume, { persistant: true });
            } else {
                // Information de routine : elle s'efface d'elle-même, comme
                // le bandeau « prêt ». Afficher un message ordinaire lève la
                // persistance d'une alerte précédente (voir status.ts), donc
                // un retour à `bonne` la fait cesser d'elle-même.
                statut.afficher(t.resume);
                bandeauLien = window.setTimeout(() => statut.masquer(), 1500);
            }
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

        detacherPleinEcran = attachFullscreenAuDOM({ bouton: fullscreenElement, cible: document.documentElement });

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

        // `document` ne porte pas `focus`/`blur` : ils vont sur `window`. La
        // cible réunit les deux sources sous l'interface que le module attend.
        //
        // Attacher n'a lieu qu'une fois `controlChannel` réellement ouvert.
        // `connectSession` résout juste après `setRemoteDescription` : à cet
        // instant le canal est encore `connecting` (ICE/DTLS/SCTP n'ont pas
        // fini), et `attachVisibilite` envoie son annonce initiale de façon
        // SYNCHRONE à l'attache. Attacher trop tôt ferait donc échouer ce tout
        // premier envoi — et si la fenêtre reste ensuite visible et focalisée
        // sans qu'aucun `focus`/`blur`/`visibilitychange` ne se déclenche
        // jamais (le cas courant d'une fenêtre qui s'ouvre au premier plan et
        // y reste), rien ne réémettrait ensuite : exactement le mode de
        // défaillance silencieux — fenêtre jamais réveillée, aucun `WARN`
        // côté agent — que la mémorisation prudente de `dernier` dans
        // visibilite.ts atténue mais ne peut pas, à elle seule, éliminer si
        // aucun second déclenchement n'a jamais lieu.
        const demarrerAnnonceVisibilite = () => {
            detacherVisibilite = attachVisibilite(
                {
                    get hidden() {
                        return document.hidden;
                    },
                    get focalisee() {
                        return document.hasFocus();
                    },
                    addEventListener(nom, rappel) {
                        if (nom === 'visibilitychange') document.addEventListener(nom, rappel);
                        else window.addEventListener(nom, rappel);
                    },
                    removeEventListener(nom, rappel) {
                        if (nom === 'visibilitychange') document.removeEventListener(nom, rappel);
                        else window.removeEventListener(nom, rappel);
                    },
                },
                (charge) => {
                    // Cette garde n'est plus le rempart principal contre la
                    // perte de l'annonce initiale (assurée par l'attente
                    // ci-dessus) : elle reste utile pour le cas résiduel où le
                    // canal se refermerait entre deux changements d'état.
                    if (session.controlChannel.readyState !== 'open') return false;
                    session.controlChannel.send(charge);
                    return true;
                },
            );
        };
        if (session.controlChannel.readyState === 'open') {
            demarrerAnnonceVisibilite();
        } else {
            session.controlChannel.addEventListener('open', demarrerAnnonceVisibilite, { once: true });
        }

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
