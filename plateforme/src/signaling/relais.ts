// Serveur de signaling : met en relation un agent et un client par session et
// relaie l'offre et la réponse SDP.
//
// ⚠️ « Aucun état persistant » N'EST PLUS VRAI depuis le sous-bloc P1 : une
// session appariée laisse une ligne en base (`ObservateurDeSession` ci-dessous,
// implémenté par `trace.ts`). Le relais lui-même reste sans état persistant —
// il ne connaît ni la base ni le SQL —, mais le SERVICE en a un.
//
// 🔴 « Aucune authentification » N'EST PLUS VRAI depuis le sous-bloc P2, et
// n'est PAS DEVENU FAUX POUR AUTANT — voici la moitié exacte qui reste vraie.
//
// Un pair de rôle `client` doit désormais présenter un jeton d'accès valide
// (`identite/garde.ts`), sans quoi il est refusé, journalisé et son socket
// fermé — avant toute entrée dans la table d'appariement et avant tout envoi
// d'`ice-config`.
//
// ❌ CE QUI SUIVAIT ICI EST DEVENU FAUX AU SOUS-BLOC P3, et l'énoncé est
// corrigé plutôt que retiré. Il disait qu'un pair se déclarant
// `{"role":"agent"}` était « TOUJOURS ACCEPTÉ SANS AUCUNE IDENTITÉ » et
// recevait des identifiants TURN de 86 400 s — la « fenêtre anonyme »,
// tolérée parce que l'agent Rust n'avait pas d'identité et qu'en exiger une
// aurait cassé le chantier D en cours.
//
// ✅ ELLE EST FERMÉE. Le rôle `agent` exige désormais son jeton, de TYPE
// `agent`, et dont le SUJET doit préfixer le nom de session demandé
// (`identite/garde.ts`). L'identité vient du canal `/agent`
// (`agents/canal.ts`), qui la délivre contre le secret d'enrôlement de la VM.
// DEUX tests distincts la tiennent (`garde-fil.test.ts`) : le refus, et
// l'absence d'`ice-config` — un service qui refuserait APRÈS avoir envoyé la
// configuration ICE passerait le premier et laisserait fuir le second.
//
// ⚠️ Ce qui RESTE vrai de l'argument d'origine : l'écoute bornée sur
// `PLATEFORME_HOTE` (`config.ts`) demeure la défense de premier rang du
// service, et ce n'est pas parce que la fenêtre `agent` s'est refermée
// qu'elle cesse de compter.

import type { IncomingMessage } from 'node:http';
import { WebSocket, WebSocketServer } from 'ws';
import { Appariement, isRole, type Role } from './appariement';
import { messagePairPresent, prevenirLArrivant, prevenirLePairEnPlace } from './pair-present';
import type { Garde } from '../identite/garde';
import { configurationIce } from './ice';
import { adresseSource } from '../http/adresse-source';
import { ligne } from '../obs/journal';
import { BUDGET_REQUETES, cleRequetes, type Budget, type Frein } from '../securite/frein';

// Types que le serveur relaie au pair. Tout le reste est refusé — un relais
// qui accepterait n'importe quoi deviendrait un canal de diffusion arbitraire.
//
// ⚠️ Cette phrase disait « sur un serveur sans authentification » jusqu'au
// sous-bloc P2, et c'est devenu faux DE MOITIÉ dans la branche même : un pair
// `client` est désormais gardé, et n'atteint donc cette table qu'authentifié.
// Le bornage des TYPES garde pourtant tout son sens, et pour deux raisons —
// il borne ce qu'un pair `agent` peut faire transiter — même authentifié
// depuis P3, il n'est autorisé QUE sur les sessions que son préfixe porte, ce
// qui ne dit rien de ce qu'il a le droit d'y relayer ; et il borne ce qu'un
// client authentifié peut diffuser à un autre. Une identité n'est pas une autorisation de relayer n'importe quoi.

//
// `fenetre-ouverte`, `fenetre-fermee`, `refus` et `viewport` portent la
// session de contrôle du sous-bloc D1, entre le superviseur (rôle `agent`) et
// la page-shell (rôle `client`).
const TYPES_RELAYES = new Set([
    'offer',
    'answer',
    'fenetre-ouverte',
    'fenetre-fermee',
    'refus',
    'viewport',
]);

// Garde de type : un message JSON valide peut être `null`, un nombre, une chaîne
// ou un tableau (tous acceptés par JSON.parse), pas seulement un objet
// `{role, session}` ou `{type, sdp}`. `null` est le cas dangereux : contrairement
// aux nombres/chaînes/tableaux (dont l'accès de propriété retourne simplement
// `undefined` par auto-boxing), `null.role` lève une TypeError. Comme ce code
// tourne dans un handler d'événement `message` d'un WebSocket exposé sans
// authentification, une TypeError non interceptée y est fatale : elle abat tout

// le process Node (aucun `uncaughtException` n'est installé dans le point
// d'entrée — REVÉRIFIÉ au sous-bloc P1, qui l'a DÉPLACÉ : ce n'est plus
// `signaling/src/index.ts` mais `plateforme/src/index.ts`, et il n'y installe
// toujours qu'un `SIGINT`), donc
// toutes les sessions actives avec elle. On rejette explicitement tout ce qui
// n'est pas un objet simple avant d'accéder à la moindre propriété.
//
// ⚠️ « exposé sans authentification » RESTE VRAI après le sous-bloc P2, et il
// faut dire POURQUOI, sans quoi un successeur croira la phrase périmée et
// desserrera la garde de type. Ce contrôle court sur le PREMIER message, qui
// arrive AVANT que la garde n'ait pu voir le moindre jeton : dans ce fichier,
// `isJsonObject` est appelé une trentaine de lignes avant `garde.verifier`.
// La poignée de main est donc, à cet instant précis, ouverte à quiconque
// atteint le port — exactement comme avant P2.

function isJsonObject(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/// Enregistre la connexion sur le budget « toute requête », et journalise SI
/// ET SEULEMENT SI le frein vient de mordre — même règle et même raison que
/// `http/routes-auth.ts::compterLEchec` : la connexion suivante sera refusée
/// tout en haut du gestionnaire `connection`, avant de jamais rappeler cette
/// fonction.
///
/// ⚠️ **CETTE LIGNE JOURNALISE À LA TRANSITION, ET NON À CHAQUE CONNEXION
/// ADMISE — c'est ce qui la distingue d'une trace par connexion.** Une ligne
/// à CHAQUE connexion, même après que le frein a commencé à refuser, ferait
/// écrire le service à un rythme que l'attaquant contrôle sans plus rien lui
/// coûter — la règle du chantier TURN (`CLAUDE.md`) : « compter ou
/// échantillonner, jamais tracer par paquet ». Journaliser à la transition
/// ferme cela : une adresse martelée écrit UNE ligne, jamais une par
/// connexion.
function compterLaConnexion(
    frein: Frein,
    cles: readonly (readonly [string, Budget])[],
    adresse: string,
): void {
    const instant = Date.now();
    frein.echec(cles, instant);
    const apres = frein.consulter(cles, instant);
    if (!apres.freine) return;
    console.warn(
        ligne('frein-requetes', {
            route: '/signal',
            adresse,
            retry_apres_s: apres.retryApresS,
            entrees: frein.taille(),
            evictions: frein.evictions(),
        }),
    );
}

export interface SignalingServer {
    port: number;
    close(): Promise<void>;
}

/// Ce que le relais SIGNALE d'une session, sans rien savoir de ce qu'on en
/// fait. C'est un port, pas une dépendance : l'implémentation de production
/// est `trace.ts`, qui écrit en base, et le relais reste ignorant de la base
/// comme il l'était.
///
/// 🔴 Les deux méthodes sont SYNCHRONES et ne rendent rien, à dessein. Le
/// gestionnaire `message` d'un socket `ws` est synchrone, et une promesse
/// rejetée y abat tout le process Node (voir `isJsonObject` ci-dessus). Une
/// signature qui rendrait une promesse inviterait un appelant à l'attendre —
/// donc à faire dépendre le signaling de sa propre trace. La trace est une
/// OBSERVATION du signaling, jamais une condition de son fonctionnement.
export interface ObservateurDeSession {
    /// Les DEUX rôles sont désormais présents sur cette session.
    ///
    /// `utilisateurId` est celui du CLIENT quand la garde en a établi un ;
    /// il est absent quand le second pair à arriver est l'agent. ⚠️ LA RAISON
    /// A CHANGÉ AU SOUS-BLOC P3 sans que la conséquence bouge : ce n'est plus
    /// que l'agent n'a « aucune identité » — il en a une depuis le canal
    /// `/agent` —, c'est qu'il ne REVENDIQUE toujours rien, sa session devant
    /// rester revendicable par le client humain qui la rejoindra
    /// (`identite/garde.ts`). C'est ce qui rend le mot « enregistrée » du
    /// critère ③ littéralement vrai en base.
    apparie(nomSession: string, utilisateurId?: string): void;
    /// La session s'est vidée : plus aucun rôle ne l'occupe.
    separe(nomSession: string): void;
}

// Deux formes, à dessein. La forme `port` est celle qu'éprouve
// `server.test.ts` depuis le jalon 1 : la garder intacte est ce qui permet de
// dire que le déménagement du sous-bloc P1 n'a rien changé au relais. La forme
// `wss` est celle qu'emploie le service, où le serveur HTTP possède le port.
//
// 🔴 `garde` est un paramètre REQUIS, jamais optionnel, et jamais permissif
// par défaut. Trois fichiers de test livrés par P1 ont dû changer pour cela
// (leur HARNAIS, aucune de leurs assertions). L'alternative — une garde
// optionnelle valant « accepter » — les aurait laissés verts sans une ligne de
// changement, ET aurait laissé un service mal câblé n'authentifier PLUS
// PERSONNE sans qu'aucun test ne rougisse. C'est le même argument que
// `http/serveur.ts` porte déjà pour `base` : « REQUISE, jamais optionnelle ».
//
// Il n'existe par ailleurs aucun chemin qui produise une garde ouverte hors
// d'un test : la seule fabrique de garde exige un secret, et
// `PLATEFORME_SECRET_JETON` n'a AUCUN défaut (`config.ts`).
//
// 🔴 `frein` ET `proxyDeConfiance` SONT REQUIS, JAMAIS OPTIONNELS — même
// argument que `garde` juste au-dessus : un défaut permissif (aucun frein,
// ou un ensemble de confiance ouvert) laisserait un service mal câblé ne
// borner AUCUNE connexion sans qu'aucun test ne rougisse. Voir
// `securite/frein.ts::BUDGET_REQUETES` : ce module partage le MÊME frein que
// `http/routes-vm.ts` et `http/routes-session.ts`, jamais un second.
export function createSignalingServer(
    port: number,
    garde: Garde,
    frein: Frein,
    proxyDeConfiance: ReadonlySet<string>,
    trace?: ObservateurDeSession,
): SignalingServer;
export function createSignalingServer(
    wss: WebSocketServer,
    garde: Garde,
    frein: Frein,
    proxyDeConfiance: ReadonlySet<string>,
    trace?: ObservateurDeSession,
): SignalingServer;
export function createSignalingServer(
    portOuWss: number | WebSocketServer,
    garde: Garde,
    frein: Frein,
    proxyDeConfiance: ReadonlySet<string>,
    trace?: ObservateurDeSession,
): SignalingServer {
    const port = typeof portOuWss === 'number' ? portOuWss : 0;
    const wss = typeof portOuWss === 'number' ? new WebSocketServer({ port }) : portOuWss;
    const sessions = new Appariement<WebSocket>();

    function send(socket: WebSocket | undefined, payload: unknown): void {
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify(payload));
        }
    }

    wss.on('connection', (socket: WebSocket, requete?: IncomingMessage) => {
        // 🔴 LE FREIN « TOUTE REQUÊTE » EST CONSULTÉ ICI, À LA CONNEXION —
        // AVANT LE PREMIER MESSAGE, donc avant `isJsonObject` et avant
        // `garde.verifier`. Une connexion WebSocket est ici l'équivalent
        // d'une requête : c'est elle qui coûte l'appariement et, si elle
        // aboutit, une ligne en base (`ObservateurDeSession`).
        // `TRAME_MAX_OCTETS` (`http/serveur.ts`) borne la taille d'un
        // message ; RIEN, avant ce lot, ne bornait le NOMBRE de connexions
        // qu'une même adresse pouvait ouvrir sur CE chemin-ci.
        //
        // 🔴 **C'EST CE LOT QUI FERME LA MOITIÉ `/signal` DU LEGS QUE
        // `http/serveur.ts` nommait — les SOCKETS, pas leur MUTISME.** Une
        // connexion est désormais comptée qu'elle envoie un message ou non :
        // c'est l'évènement `connection` lui-même qui coûte, pas le premier
        // message. Voir la note corrigée de `TRAME_MAX_OCTETS` dans
        // `http/serveur.ts` : elle distingue désormais `/signal` (borné ICI)
        // et `/agent` (`agents/canal.ts`, où le frein n'est TOUJOURS consulté
        // qu'au message — un pair muet y reste incompté).
        //
        // ⚠️ `requete?.socket.remoteAddress` PEUT ÊTRE ABSENT : la forme
        // `port` de cette fonction (`server.test.ts` depuis le jalon 1)
        // n'émet aucune requête de montée. `adresseSource` rend alors
        // `ADRESSE_INCONNUE`, budget PARTAGÉ par tous les pairs sans adresse
        // — même comportement que `agents/canal.ts`.
        const adresse = adresseSource(
            requete?.socket.remoteAddress,
            Array.isArray(requete?.headers['x-forwarded-for'])
                ? requete.headers['x-forwarded-for'].join(',')
                : requete?.headers['x-forwarded-for'],
            proxyDeConfiance,
        );
        // 🔴 UNE `PLATEFORME_PROXY_DE_CONFIANCE` MAL POSÉE FAIT DÉGÉNÉRER CE
        // FREIN EN FREIN GLOBAL, ET SA GRAVITÉ A CHANGÉ AVEC CE LOT — voir le
        // paragraphe complet chez `http/routes-vm.ts` (même clé
        // `BUDGET_REQUETES`, même témoin : la ligne `frein-requetes` qui
        // nomme l'adresse retenue), jamais recopié pour ne pas diverger.
        const clesRequetes: readonly (readonly [string, Budget])[] = [
            [cleRequetes(adresse), BUDGET_REQUETES],
        ];
        // `Date.now()` lu ici, comme pour `configurationIce` plus bas dans ce
        // même fichier : ce module ne reçoit pas d'horloge injectée.
        const verdictRequetes = frein.consulter(clesRequetes, Date.now());
        if (verdictRequetes.freine) {
            // ⚠️ ENVOYER PUIS FERMER, jamais l'inverse — même règle que sur
            // un refus de poignée de main plus bas : un `terminate()`
            // immédiat tronquerait le message.
            //
            // 🔴 `retryApresS` EST DÉSORMAIS PORTÉ SUR LE FIL (round de
            // correction 1, critique ②) — LES DEUX ROUTES HTTP FREINÉES
            // (`routes-vm.ts`, `routes-session.ts`) LE POSENT DÉJÀ, EN
            // `Retry-After`, DEPUIS CE MÊME LOT ; SEUL CE REFUS WebSocket EN
            // ÉTAIT PRIVÉ. Sans lui, l'agent ne peut deviner combien de temps
            // attendre — c'est la moitié la moins chère du remède au
            // verrouillage documenté par `surveillance_pont.rs`, l'autre
            // moitié étant le repli exponentiel qu'il applique déjà.
            send(socket, {
                type: 'error',
                reason: 'trop de requêtes',
                motif: 'trop-de-requetes',
                retryApresS: verdictRequetes.retryApresS,
            });
            socket.close(1008, 'trop-de-requetes');
            return;
        }
        compterLaConnexion(frein, clesRequetes, adresse);

        let role: Role | undefined;
        let sessionId: string | undefined;

        socket.on('message', (raw) => {
            let message: unknown;
            try {
                message = JSON.parse(raw.toString());
            } catch {
                send(socket, { type: 'error', reason: 'JSON invalide' });
                return;
            }

            // Rejet avant toute lecture de propriété : voir `isJsonObject` ci-dessus.
            // Le pair fautif reçoit une erreur mais sa connexion reste ouverte, pour
            // qu'il puisse retenter avec un message valide.
            if (!isJsonObject(message)) {
                send(socket, {
                    type: 'error',
                    reason: role
                        ? 'message invalide : objet JSON attendu'
                        : 'premier message invalide : {role, session} attendu',
                });
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

                // 🔴 THE GUARD RUNS BEFORE `declarer`, AND THE ORDER IS NOT
                // IRRELEVANT. A refused peer that had entered the pairing
                // table would occupy the role there and keep the LEGITIMATE
                // peer from arriving: a denial of service open to anyone,
                // obtained precisely by refusing to authenticate.
                const verdict = garde.verifier({
                    role: declaredRole,
                    session: declaredSession,
                    jeton: message.jeton,
                });
                if (!verdict.ok) {
                    // The log carries the session name and the requester's
                    // identifier; the message sent on the wire carries neither
                    // one nor the other (`identite/garde.ts`).
                    console.warn(`poignée de main refusée : ${verdict.journal}`);
                    // ⚠️ SEND THEN CLOSE, never the reverse: an
                    // immediate `terminate()` would truncate the message, and the
                    // peer would see a close with no reason.
                    send(socket, { type: 'error', reason: verdict.message, motif: verdict.motif });
                    // 🔴 The socket is CLOSED, whereas it stays OPEN after a
                    // malformed message (see above, deliberate since
                    // milestone 1). Spec §6: "typed refusal on the handshake,
                    // connection closed — unlike the malformed message,
                    // which the relay lets the peer retry on purpose".
                    socket.close(1008, verdict.motif);
                    return;
                }

                const refus = sessions.declarer(declaredSession, declaredRole, socket);
                if (refus) {
                    // 🔴 `motif` IS TYPED, `reason` IS A SENTENCE. Added on
                    // 31 August 2026: the hub elects a carrier tab through Web
                    // Locks, and its FALLBACK (browser without that API) must
                    // tell "the seat is taken" — to swallow silently,
                    // a second tab not being the user's fault —
                    // from a refusal with another cause, which must be shown.
                    // Deciding on `reason` would force the client to compare a
                    // FRENCH sentence, which gets reworded: that is the F1 trap,
                    // paid for with nine minutes on two messages that shared a
                    // substring.
                    //
                    // ⚠️ STRICTLY ADDITIVE: the field is added, none is
                    // removed (rule §10.2). A client from yesterday does not read `motif`
                    // and keeps reading `reason`.
                    send(socket, { type: 'error', reason: refus, motif: 'role-occupe' });
                    return;
                }

                // ONLY NOW: `declarer` has accepted. Claiming
                // earlier would leave a phantom membership behind a
                // peer refused because the role was already taken.
                garde.revendiquer(declaredSession, verdict.utilisateurId);

                role = declaredRole;
                sessionId = declaredSession;

                // THE PAIRING, and not the declaration: the opposite peer
                // exists, so both roles are there. `pair` returns the socket
                // OPPOSITE — if it is defined, this peer is the second one.
                const pairEnFace = sessions.pair(declaredSession, declaredRole);
                if (pairEnFace) {
                    trace?.apparie(declaredSession, verdict.utilisateurId);
                    // The symmetric twin of the `peer-gone` sent at the bottom of the
                    // file: the relay could say "your peer has left"
                    // and could not say "your peer has arrived". The whole
                    // reason for being — and the production measurement that
                    // forced it — lives in `pair-present.ts`, never copied
                    // here so as not to diverge.
                    if (prevenirLePairEnPlace(declaredRole)) send(pairEnFace, messagePairPresent());
                    // And the SYMMETRIC half: the agent that ARRIVES on a
                    // session where a client is already waiting. It did not exist
                    // as long as the agent opened its control session
                    // only once; it becomes the ordinary case since
                    // it REOPENS it. Full reasoning in
                    // `pair-present.ts::prevenirLArrivant`, never copied
                    // here so as not to diverge.
                    if (prevenirLArrivant(declaredRole)) send(socket, messagePairPresent());
                }

                // ICE configuration: sent to EACH peer as soon as it
                // declares itself, agent and client alike. Both need it — the
                // TURN relay is only useful if both ends can
                // use it.
                //
                // `Date.now()` is read here and not in `configurationIce`:
                // the latter thus stays a pure function, testable
                // with a fixed instant.
                const ice = configurationIce(process.env, declaredSession, Date.now());
                if (ice) {
                    send(socket, { type: 'ice-config', ...ice });
                } else {
                    // Explicit trace: a session without a relay that fails to
                    // connect from outside must be
                    // diagnosable without rereading the code.
                    console.warn(
                        'aucun serveur TURN configuré (TURN_URL/TURN_SECRET) : session sans relais',
                    );
                }

                // An offer that arrived before this agent is waiting for it: hand it over
                // now, otherwise it will never leave.
                if (declaredRole === 'agent') {
                    const offre = sessions.prendreOffre(declaredSession);
                    if (offre) send(socket, { type: 'offer', sdp: offre });
                }
                return;
            }

            // Following messages: relayed to the peer.
            const peer = sessions.pair(sessionId!, role);

            if (TYPES_RELAYES.has(message.type as string)) {
                if (message.type === 'offer' && !peer) {
                    // No agent opposite: we hold on to it, rather than lose it.
                    sessions.retenirOffre(sessionId!, message.sdp as string);
                    return;
                }
                send(peer, message);
            } else {
                send(socket, { type: 'error', reason: `type inconnu : ${message.type}` });
            }
        });

        socket.on('close', () => {
            if (!role || !sessionId) return;
            const peer = sessions.pair(sessionId, role);
            const { vide } = sessions.retirer(sessionId, role);
            send(peer, { type: 'peer-gone' });
            // The exact instant the session is forgotten from the table: that is
            // the one that closes the row, and not the departure of the first peer.
            //
            // ⚠️ `garde.liberer` is called HERE and not in `http/serveur.ts`
            // by the observer: the relay is ALSO used in its
            // `port` form, without an observer (`server.test.ts` since milestone 1).
            // Hooking the release onto the trace would mean a session name
            // would stay taken for life in that setup, with nothing
            // saying so.
            if (vide) {
                garde.liberer(sessionId);
                trace?.separe(sessionId);
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
