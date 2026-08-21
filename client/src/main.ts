import { attachInput } from './input';
import { connectSession } from './webrtc';
import { attachStats } from './stats';
import { armerLeSon } from './audio';
import { creerStatut } from './status';
import { creerEcranTerminalAuDOM } from './ecran-terminal';
import { attachPointerAuDOM } from './pointer';
import { attachGamepadAuDOM } from './gamepad';
import { armerPleinEcranAuDOM, attachFullscreenAuDOM } from './fullscreen';
import { texteLien } from './lien';
import { viewportPair } from './viewport';
import { attachVisibilite } from './visibilite';
import { attacherBoutonMicro } from './micro';
import { attacherAccentAuDOM } from './accent-dom';
import { attacherPressePapierAuDOM } from './presse-papier-dom';
import type { Recu } from './presse-papier';
import { attacherResizeAuDOM } from './resize-dom';
import { adresseSignaling } from './adresse-plateforme';

const video = document.querySelector<HTMLVideoElement>('#remote')!;
const statusElement = document.querySelector<HTMLDivElement>('#status')!;
const statsElement = document.querySelector<HTMLDivElement>('#stats')!;
const fullscreenElement = document.querySelector<HTMLButtonElement>('#fullscreen')!;
const microElement = document.querySelector<HTMLButtonElement>('#micro')!;

// Point d'écriture unique du bandeau de statut : protège un message TERMINAL
// (fin de session, échec) contre l'écrasement par un message ordinaire
// arrivant après lui. Voir `status.ts` pour la justification complète.
// La SECONDE cible est l'écran plein cadre des états terminaux (S4, tâche 9) :
// `creerStatut` la lève pour un message `terminal` et pour lui seul.
const statut = creerStatut(statusElement, creerEcranTerminalAuDOM());

// La session et le signaling sont paramétrables par l'URL pour faciliter les
// essais : ?session=demo&signaling=ws://192.168.3.2:8080
//
// 🔴 SANS PARAMÈTRE, L'ADRESSE SUIT LE PROTOCOLE DE LA PAGE — `wss:` si la page
// est en `https:`, `ws:` sinon —, ET SON PORT. Le littéral d'avant,
// `ws://<hôte>:8080`, était du CONTENU MIXTE derrière le proxy TLS : le
// navigateur refusait la connexion, la page se chargeait quand même, et le
// média ne s'établissait jamais. La règle vit dans `adresse-plateforme.ts`,
// qui est PUR et testé — aucun test Node ne peut voir un refus de contenu mixte.
const params = new URLSearchParams(window.location.search);
const sessionId = params.get('session') ?? 'demo';
const signalingUrl = adresseSignaling(window.location, params.get('signaling'));

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
    // MÊME UNITÉ que le `Resize` émis plus bas (`clientWidth × devicePixelRatio`).
    //
    // ⚠️ **La raison écrite ici à l'origine était déjà périmée quand elle a été
    // écrite, et c'est la revue TRANSVERSE de fin de branche D9 qui l'a
    // rattrapée.** Elle disait : « sans ce facteur, CHAQUE connexion de CHAQUE
    // fenêtre déclencherait un changement de mode, avec 25 à 100 % d'écart
    // (leg 7 du sous-bloc D8) ». Or la tâche 3 du même sous-bloc D9 — un commit
    // AVANT celui qui a écrit cette phrase — avait retiré le changement de mode
    // de sortie sur mesure (voir le constat en tête de
    // `agent/src/capteur/plein_ecran.rs`). Il n'y a donc plus aucun changement
    // de mode à déclencher : `WindowsSource::resize` retourne avant tout en
    // mode `SortieEntiere`, et le court-circuit « taille inchangée » qu'on
    // invoquait n'est même plus atteint.
    //
    // ✅ **Ce que le facteur corrige RÉELLEMENT, et qui justifie de le garder** :
    // l'annonce de viewport DÉCIDE la taille de la sortie virtuelle créée par le
    // superviseur (`superviseur/boucle.rs::creer_sortie`). Sans dpr, un client
    // HiDPI recevait une sortie plus PETITE que sa surface d'affichage réelle,
    // donc une image mise à l'échelle vers le haut par le navigateur. Avec, la
    // sortie naît en pixels périphériques, l'unité dans laquelle le `Resize` de
    // routine parle déjà.
    //
    // ⚠️ **Conséquence non mesurée, et déclarée comme telle (legs de D9)** : à
    // `devicePixelRatio = 2`, une fenêtre de 1280×720 CSS demande désormais une
    // sortie de 2560×1440, soit QUATRE fois les pixels à capturer et à encoder,
    // et **rien ne borne cette demande** — `windows_source/sortie.rs::
    // borner_a_la_taille_max` (1920×1080) a perdu son dernier appelant avec le
    // changement de mode et n'est plus branchée nulle part. D6 a relevé le
    // décodeur du navigateur saturé dès huit fenêtres de 1280×720.
    //
    // Il n'y a qu'un `devicePixelRatio` en jeu : c'est CETTE page qui annonce, et
    // c'est son propre `ResizeObserver` qui émettra le `Resize`.
    //
    // Multiplier PUIS arrondir en pair — `viewportPair` a un plancher à 2, et
    // l'ordre inverse laisserait passer une hauteur impaire à dpr impair.
    const dpr = window.devicePixelRatio;
    const { largeur, hauteur } = viewportPair(
        Math.round(window.innerWidth * dpr),
        Math.round(window.innerHeight * dpr),
    );
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
// Le micro (chantier E). Déclaré ici pour la même raison que ses voisins :
// `onControl` est câblé AVANT que la promesse de `connectSession` ne résolve,
// et le message `ready` — qui décide si le bouton paraît — peut arriver avant
// le `.then()` qui construit le contrôle.
let micro: ReturnType<typeof attacherBoutonMicro> | undefined;
// Le presse-papier de la VM (sous-bloc P1). Déclaré ici pour la même raison
// que ses voisins : `onControl` est câblé AVANT que la promesse de
// `connectSession` ne résolve. Ce qu'un message arrivé avant l'attache
// devient est écrit sur `PressePapierAttache.recevoir`.
let pressePapier: ReturnType<typeof attacherPressePapierAuDOM> | undefined;
// La couleur d'accent de la fenêtre (sous-bloc A1). ⚠️ **Un `const`, là où ses
// voisins sont des `let | undefined`**, et la raison est qu'il ne dépend de RIEN
// : ni de la session, ni du `.then()`, ni d'un message antérieur — seulement du
// document, qui existe déjà. Il n'y a donc aucune fenêtre pendant laquelle un
// message pourrait arriver sans destinataire, et pas de `?.` à écrire.
const accent = attacherAccentAuDOM({
    lireToken: (nom) =>
        getComputedStyle(document.documentElement).getPropertyValue(nom).trim(),
    // 🔴 `documentElement`, JAMAIS `document.body` : un token posé sur `body`
    // est invisible à `getComputedStyle(document.documentElement)`, et c'est
    // la rouge du critère ② de la recette.
    poserToken: (nom, valeur) => document.documentElement.style.setProperty(nom, valeur),
});
/// L'agent a-t-il annoncé `Capabilities.clipboard` ?
///
/// **Un `let` relu par une fermeture, jamais une valeur passée à l'attache** :
/// `Capabilities` arrive avant `Ready` mais rien ne garantit qu'il précède
/// `attachInput`, et un booléen figé au montage vaudrait `false` à jamais.
/// C'est la même indépendance à l'ordre que le reste de ce fichier.
///
/// `undefined` sur un agent d'avant P2 : `Boolean(undefined)` vaut `false`,
/// donc rien n'est armé, et le `Ctrl+V` garde son comportement d'avant.
let collageArme = false;
// `mic` tel que l'agent l'a annoncé, `undefined` compris. Mémorisé parce que
// `ready` peut précéder la construction du bouton : sans cela, une annonce
// arrivée tôt serait perdue et le bouton resterait caché pour toujours,
// SANS RIEN pour le dire — le mode de défaillance silencieux que ce dépôt
// a payé sur l'annonce de visibilité (voir plus bas).
let micAnnonce: boolean | undefined;
// Le dernier `clipboard` reçu, mémorisé pour la même raison exactement que
// `micAnnonce` juste au-dessus : `onControl` est câblé AVANT que la promesse de
// `connectSession` ne résolve, donc un message arrivé dans cet intervalle
// serait perdu SANS RIEN pour le dire. Et depuis P3 l'agent émet l'état courant
// à l'INSCRIPTION de la fenêtre, ce qui tombe précisément dans cet intervalle.
//
// ⚠️ Ces deux lignes sont du CÂBLAGE, et elles ne sont couvertes par AUCUN
// TEST — `main.ts` n'en a aucun et ne peut pas en avoir. La RÈGLE qu'elles
// routent — « rejouer le mémorisé au montage » — vit dans
// `presse-papier-dom.ts`, où quatre tests la tiennent. Leur seul contrôle de
// bout en bout est le critère ① de la recette : une fenêtre attachée APRÈS la
// copie.
let dernierPressePapier: Recu | undefined;
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
            // Le bouton micro ne paraît QUE si l'agent dit avoir le câble
            // (spec §10). `message.mic` est passé TEL QUEL : la règle « son
            // absence vaut false » vit dans `micro.ts`, où un test la garde,
            // plutôt que dans un `if` d'ici que rien n'exercerait.
            micAnnonce = message.mic;
            micro?.annoncerDisponibilite(micAnnonce);
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
            // Fin de session : l'extinction du micro doit être RÉELLE, et
            // `session.close()` n'est pas appelé sur ce chemin. Sans ceci
            // l'indicateur de Chrome resterait allumé après la fin (spec §9).
            micro?.detacher();
            // Même raison que les détachements ci-dessus : l'écouteur
            // `focus` survivrait sinon à la fin de session et écrirait le
            // presse-papier local pour une session morte.
            pressePapier?.detacher();
            // `neutre` : l'utilisateur a fermé l'application distante, ce
            // n'est pas une erreur. Le ton ne sert QU'À l'écran terminal.
            statut.afficher(`session terminée : ${message.reason}`, {
                terminal: true,
                ton: 'neutre',
            });
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
            // Aucune logique ici non plus : ce drapeau ne fait que GATER
            // l'exception clavier de `input.ts`. Sans lui, `PRESSE_PAPIER=0`
            // donnerait le pire des deux mondes — le client retiendrait le
            // `Ctrl+V` alors que personne ne l'injecterait côté VM.
            collageArme = Boolean(message.clipboard);
        } else if (message.type === 'clipboard') {
            // Aucune logique ici : toute la décision — écrire ou différer,
            // dire un refus, crier au deuxième échec — vit dans le module
            // attaché, lui-même adossé à `presse-papier.ts`, pur et testé.
            // La mémoire est posée AVANT le `?.`, jamais dans une branche
            // `else` : la poser dans le `else` ferait diverger les deux chemins
            // le jour où l'un changerait.
            dernierPressePapier = { texte: message.text, octets: message.bytes };
            pressePapier?.recevoir(dernierPressePapier);
        } else if (message.type === 'accent') {
            // Aucune logique ici : conformer, refuser, poser — tout vit dans
            // `accent-dom.ts`, adossé à `accent.ts`, pur et testé.
            accent.recevoir(message.couleur);
        }
    },
})
    .then((session) => {
        attachInput({
            video,
            channel: session.inputChannel,
            // `window` et non `video` : c'est là que les écouteurs clavier
            // vivaient déjà avant P2.
            clavier: window,
            collageArme: () => collageArme,
        });
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

        // Le micro. `micSender` vient du transceiver `sendonly` déclaré sans
        // piste dans l'offre initiale : allumer n'est qu'un `replaceTrack`, et
        // ne demande aucune renégociation.
        micro = attacherBoutonMicro({
            bouton: microElement,
            sender: session.micSender,
            // La permission n'est demandée qu'ICI, au clic, jamais à
            // l'ouverture de session (spec §9).
            demanderFlux: (contraintes) => navigator.mediaDevices.getUserMedia(contraintes),
            // Les deux états d'échec seulement portent un message ; il dit
            // COMMENT rétablir la permission, pas seulement qu'elle manque.
            // `persistant` : l'utilisateur doit avoir le temps de le lire et
            // d'aller le suivre, une minuterie voisine ne doit pas l'effacer.
            surMessage: (texte) => statut.afficher(texte, { persistant: true }),
        });
        // `ready` a pu arriver AVANT ce point : le rejouer est le seul moyen
        // que le bouton paraisse dans ce cas. Le rappel est inoffensif si
        // l'annonce n'est pas encore venue (`undefined` laisse caché).
        micro.annoncerDisponibilite(micAnnonce);

        // Le presse-papier de la VM. `writeText` SEULE — jamais `readText` :
        // voir l'en-tête de `presse-papier-dom`. `window` porte le `focus`
        // que `document` ne porte pas, comme pour `armerLeSon` en dessous.
        pressePapier = attacherPressePapierAuDOM({            ecrire: (texte) => navigator.clipboard.writeText(texte),
            focalise: () => document.hasFocus(),
            cible: window,
            // Le canal de CONTRÔLE, jamais celui des entrées : un collage
            // exige un ORDRE (le presse-papier Windows d'abord, `Ctrl+V`
            // ensuite), et le canal d'entrées est `ordered: false`. Même
            // canal, et même geste, qu'`encodeResize` plus haut.
            emettre: (message) => {
                if (session.controlChannel.readyState === 'open') {
                    session.controlChannel.send(message);
                }
            },
            // `persistant`, sur le patron EXACT du micro : refus de taille et
            // échec répété demandent tous deux un geste de l'utilisateur.
            surMessage: (texte) => statut.afficher(texte, { persistant: true }),
            // Ce qui est arrivé AVANT ce point, s'il y a lieu. `undefined`
            // laisse le comportement d'avant P3 mot pour mot.
            initial: dernierPressePapier,
        });

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

        // Le suivi de taille : `ResizeObserver`, lissage, émission, et le
        // rejeu à l'ouverture du canal. Extrait de ce fichier le 20 août 2026
        // — voir l'en-tête de `resize-dom.ts`, qui porte l'invariant de
        // synchronicité (leg n°12 de D9) et la raison de l'extraction.
        //
        // 🔴 **APPEL SYNCHRONE, et il doit le rester** : glisser un `await`
        // avant cette ligne romprait le rejeu EN SILENCE.
        attacherResizeAuDOM(video, session);
    })
    .catch((error: unknown) => {
        statut.afficher(`échec : ${error instanceof Error ? error.message : String(error)}`, {
            terminal: true,
            ton: 'danger',
        });
    });
