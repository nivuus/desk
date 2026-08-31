// Établissement de la session WebRTC. Le navigateur est l'offrant : il déclare
// la piste vidéo en réception seule, la piste audio descendante en réception
// seule, la piste MONTANTE du micro en émission seule et SANS PISTE (chantier
// E), et les deux canaux de données ; puis il attend la réponse de l'agent
// relayée par le signaling.

import { parseAgentControl, type AgentControl } from '../../proto/ts/control';
import { jetonAcces } from './jeton';

export interface SessionOptions {
    signalingUrl: string;
    sessionId: string;
    video: HTMLVideoElement;
    onControl?: (message: AgentControl) => void;
    onStatus?: (message: string) => void;
    /// Le jeton d'accès porté dans la poignée de main (sous-bloc P2).
    ///
    /// ⚠️ FACULTATIF À DESSEIN : un champ requis obligerait à modifier
    /// `main.ts`, unique appelant, qu'un autre chantier tient. Absent, le
    /// jeton est lu dans le coffre du navigateur (`jeton.ts`) — ce qui laisse
    /// UN SEUL lecteur du stockage dans tout le client, ce qui est meilleur en
    /// soi. Le coût est nommé : ce module gagne une dépendance à un global de
    /// navigateur, alors qu'il manipule déjà `WebSocket` et
    /// `RTCPeerConnection` ; `jeton.ts`, lui, reste pur.
    jeton?: string;
}

export interface SessionHandle {
    pc: RTCPeerConnection;
    inputChannel: RTCDataChannel;
    controlChannel: RTCDataChannel;
    /// L'émetteur de la piste MONTANTE (chantier E), déclaré SANS PISTE.
    ///
    /// C'est `client/src/micro.ts` qui le remplit par `replaceTrack`, au clic,
    /// et le vide à l'extinction. Exposé ici parce que `connectSession` est le
    /// seul endroit qui construise la `RTCPeerConnection` : le sender n'existe
    /// pas avant elle, et rien d'autre ne peut le retrouver sans fouiller
    /// `pc.getTransceivers()` par position — ce qui serait un index positionnel,
    /// c'est-à-dire exactement ce que ce dépôt a déjà payé sur les sorties DXGI.
    micSender: RTCRtpSender;
    close(): void;
}

// Délai maximal d'attente de la réponse de l'agent, après l'envoi de
// l'offre. Si aucun agent n'est connecté à la session demandée, le serveur
// de signaling relaie l'offre vers un pair inexistant et ne renvoie jamais
// rien au client : sans ce délai, la promesse d'attente ne se résoudrait
// jamais et l'utilisateur resterait bloqué indéfiniment.
const ANSWER_TIMEOUT_MS = 15_000;

/// Sous-ensemble des messages de signaling attendus en réponse à l'offre,
/// discriminé par `type`.
type SignalingMessage =
    | { type: 'answer'; sdp: string }
    | { type: 'error'; reason?: string }
    | { type: 'peer-gone' }
    | { type: 'ice-config'; iceServers: RTCIceServer[] };

/// Parse et valide un message de signaling brut.
///
/// `JSON.parse` réussit sur des charges utiles qui ne sont pas des objets :
/// `"null"` donne `null`, mais aussi `"42"` donne un nombre, `'"x"'` une
/// chaîne, `"[1,2]"` un tableau. Accéder à `.type` sur l'une de ces valeurs
/// ne lève pas toujours (un tableau ou une chaîne ont bien un `.type`
/// `undefined`, pas d'exception), mais `null.type` lève une `TypeError` non
/// interceptée — c'est le défaut corrigé ici. On valide donc explicitement
/// que le résultat est un objet non nul et non tableau avant toute lecture
/// de propriété, quelle que soit la forme de la charge utile.
///
/// Retourne `undefined` si le message est illisible ou de forme inattendue :
/// l'appelant l'ignore alors silencieusement, sans faire planter l'attente
/// (le message recherché — la réponse SDP — peut encore arriver ensuite).
// Exportée uniquement pour être testée unitairement sans avoir à instancier
// un vrai WebSocket (voir webrtc.test.ts) : le reste du module ne l'utilise
// que via `waitForAnswer`, en interne.
export function parseSignalingMessage(raw: string): SignalingMessage | undefined {
    let parsed: unknown;
    try {
        parsed = JSON.parse(raw);
    } catch {
        return undefined;
    }
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
        return undefined;
    }
    const { type } = parsed as Record<string, unknown>;
    if (type === 'answer' || type === 'error' || type === 'peer-gone' || type === 'ice-config') {
        return parsed as SignalingMessage;
    }
    return undefined;
}

/// Attend le SDP de réponse de l'agent, relayé par le socket de signaling.
/// Rejette si : un message d'erreur ou « peer-gone » est reçu, le socket se
/// ferme avant la réponse, ou le délai maximal est dépassé. Un message
/// illisible ou de forme inattendue (JSON invalide, valeur non-objet comme
/// `null`/un nombre/un tableau, ou objet sans `type` reconnu) est journalisé
/// et ignoré plutôt que de faire planter l'attente : d'autres messages
/// valides peuvent encore arriver, notamment la réponse elle-même.
///
/// Exportée uniquement pour être testée sans passer par `connectSession`
/// (qui exige un DOM complet — `RTCPeerConnection`, `WebSocket` réel, etc.,
/// indisponibles sous le runtime Node du test) : `waitForAnswer` ne dépend
/// que de `addEventListener`/`removeEventListener`, qu'un faux socket minimal
/// suffit à fournir (voir webrtc.test.ts).
export function waitForAnswer(socket: WebSocket): Promise<string> {
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
            const message = parseSignalingMessage(String(event.data));
            if (!message) {
                console.warn('message de signaling illisible ou de forme inattendue, ignoré');
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
///
/// ⚠️ EXPORTÉE PAR LE SOUS-BLOC F1, ET C'EST UNE MODIFICATION DÉCLARÉE.
/// `client/src/fichiers/canal.ts` ouvre une `RTCPeerConnection` DÉDIÉE, sans
/// média (décision D4 du plan de F1) : `connectSession` ne lui convient pas —
/// elle exige un `HTMLVideoElement` et ajoute inconditionnellement trois
/// transceivers. Le plan interdit de refactorer `connectSession` pour rendre la
/// vidéo optionnelle : ce serait toucher le chemin critique de toutes les
/// fenêtres pour un besoin qui a sa propre fonction. Il prescrit en revanche de
/// RÉEMPLOYER les fonctions de signaling « sans les copier » — d'où cet export
/// et celui d'`attendreConfigIce`. Aucun comportement n'est modifié.
export function waitForIceGathering(pc: RTCPeerConnection): Promise<void> {
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

/// Attend la configuration ICE du signaling, au plus `delaiMs`.
///
/// Rend un tableau VIDE en cas d'absence : c'est le cas normal d'un
/// déploiement sans relais, pas une erreur. Le message est retiré du flux
/// pour ne pas être confondu plus tard avec une réponse SDP.
///
/// ⚠️ EXPORTÉE PAR LE SOUS-BLOC F1 — voir la note de `waitForIceGathering`.
export function attendreConfigIce(socket: WebSocket, delaiMs: number): Promise<RTCIceServer[]> {
    return new Promise((resolve) => {
        const finir = (serveurs: RTCIceServer[]) => {
            socket.removeEventListener('message', onMessage);
            clearTimeout(timer);
            resolve(serveurs);
        };
        const onMessage = (event: MessageEvent) => {
            const message = parseSignalingMessage(String(event.data));
            if (message?.type === 'ice-config') finir(message.iceServers);
        };
        const timer = setTimeout(() => finir([]), delaiMs);
        socket.addEventListener('message', onMessage);
    });
}

export async function connectSession(options: SessionOptions): Promise<SessionHandle> {
    const status = options.onStatus ?? (() => {});

    // Le socket s'ouvre AVANT la `RTCPeerConnection`, contrairement au jalon 1 :
    // les serveurs ICE ne sont connus qu'une fois la configuration reçue du
    // signaling, et `RTCPeerConnection` les veut à la construction.
    const socket = new WebSocket(options.signalingUrl);
    await new Promise<void>((resolve, reject) => {
        socket.addEventListener('open', () => resolve(), { once: true });
        socket.addEventListener('error', () => reject(new Error('signaling injoignable')), {
            once: true,
        });
    });
    // 🔴 LE CHAMP `jeton` EST AJOUTÉ, AUCUN N'EST RETIRÉ — spec §10.2. Un
    // service du sous-bloc P1 ne lit que `role` et `session` (son relais ignore
    // tout le reste) : ce client reste donc compatible avec un service
    // antérieur à la garde. LA COMPATIBILITÉ NE VA QUE DANS CE SENS — un
    // client d'avant P2, lui, sera refusé par un service de P2, et c'est
    // précisément l'objet du sous-bloc.
    //
    // ⚠️ AUCUNE REDIRECTION ICI, et ce n'est pas un oubli. Sans jeton, la
    // session est refusée et le refus s'affiche ; c'est `hub/page.ts`
    // (`shell-page.ts` avant que le hub ne devienne la seule surface,
    // 31 août 2026) qui renvoie vers l'écran de connexion, parce qu'il est
    // l'entrée réelle de l'utilisateur. Une page de session est TOUJOURS ouverte par la shell, sur
    // la même origine, donc le jeton y est déjà. Rediriger depuis une
    // bibliothèque lui donnerait un pouvoir sur la navigation de ses appelants.
    const jeton = options.jeton ?? jetonAcces();
    socket.send(JSON.stringify({ role: 'client', session: options.sessionId, jeton }));

    // La configuration ICE arrive juste après la déclaration de rôle, ou
    // jamais si aucun relais n'est déployé. On l'attend brièvement plutôt que
    // de bloquer : une session en réseau local doit continuer à s'établir
    // sans relais, exactement comme avant ce chantier.
    const iceServers = await attendreConfigIce(socket, 2000);
    const pc = new RTCPeerConnection({ iceServers });

    pc.addTransceiver('video', { direction: 'recvonly' });
    // Le navigateur est l'offrant : c'est lui qui doit déclarer la piste
    // audio. L'agent ne fait que répondre, à condition d'avoir activé Opus sur
    // son constructeur `Rtc` — sans quoi il répondrait sans piste audio.
    pc.addTransceiver('audio', { direction: 'recvonly' });

    // Le micro (chantier E). Déclaré SANS PISTE : rien n'est capté, aucune
    // permission n'est demandée, aucun octet n'est émis tant que
    // `client/src/micro.ts` n'a pas appelé `replaceTrack`. C'est ce qui rend
    // « à la demande » réalisable sans renégociation — `connectSession` fait
    // un aller-retour UNIQUE (offre, puis réponse) et n'a AUCUN chemin pour
    // une seconde offre. `replaceTrack` sur un sender existant ne change ni
    // le codec ni les m-lines, donc ne demande pas de renégociation.
    //
    // ⚠️ L'ORDRE DES TROIS `addTransceiver` DÉCIDE LES `mid`, et l'agent en
    // dépend. Ce transceiver-ci doit venir APRÈS l'audio descendant : il prend
    // alors `mid:2`, et l'agent le voit en `RecvOnly` (str0m inverse la
    // direction distante à l'acceptation de l'offre) — c'est ce qui range son
    // `mid` dans `mic_mid` et non dans `audio_mid`
    // (`agent/src/transport/evenements.rs`). Intervertir les deux lignes
    // ferait partir le son DESCENDANT sur une piste que l'agent ne peut pas
    // émettre, sans une seule erreur : c'est le défaut latent que la tâche 7
    // du chantier E a exhibé puis corrigé.
    const micTransceiver = pc.addTransceiver('audio', { direction: 'sendonly' });

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

    // Un seul MediaStream porte les deux pistes. Réassigner `srcObject` à
    // chaque piste reçue ferait chasser la première par la seconde : l'ordre
    // d'arrivée n'est pas garanti, et le résultat serait une session tantôt
    // muette, tantôt sans image.
    const flux = new MediaStream();
    pc.addEventListener('track', (event) => {
        flux.addTrack(event.track);
        // Latence de restitution : demander au navigateur de ne pas
        // constituer de tampon de gigue au-delà du strict nécessaire.
        //
        // Ce n'est pas gratuit — sur un lien qui gigue, ce tampon est ce qui
        // lisse la restitution, et le raboter échange de la latence contre du
        // saccadement. Mesuré sous chaque profil netem à la recette.
        //
        // Chromium seulement : ailleurs la propriété n'existe pas et
        // l'affectation est sans effet. D'où l'accès défensif plutôt qu'un
        // `receiver.playoutDelayHint = 0` direct, qui lèverait en mode strict
        // sur un objet scellé.
        if (event.track.kind === 'video' && 'playoutDelayHint' in event.receiver) {
            (event.receiver as RTCRtpReceiver & { playoutDelayHint: number }).playoutDelayHint = 0;
        }
        if (options.video.srcObject !== flux) {
            options.video.srcObject = flux;
        }
        status(`flux reçu (${flux.getTracks().length} piste(s))`);
    });

    pc.addEventListener('connectionstatechange', () => {
        status(`connexion : ${pc.connectionState}`);
    });

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
        micSender: micTransceiver.sender,
        close() {
            // ⚠️ L'EXTINCTION DOIT ÊTRE RÉELLE (spec §9). `pc.close()` NE STOPPE
            // PAS les pistes locales : l'indicateur de micro de Chrome resterait
            // allumé après la fin de session, et le périphérique resterait pris.
            // C'est précisément le « mensonge visuel » que la spec qualifie
            // d'inacceptable sur cette fonction. `stop()` est idempotent : que
            // `micro.ts` l'ait déjà appelé ne coûte rien.
            //
            // ⚠️ DIVERGENCE ASSUMÉE AVEC LE PLAN, qui écrit « `close()` appelle
            // `detacher()` du micro ». Cela ferait dépendre `webrtc.ts` de
            // `micro.ts`, lequel dépend déjà de `SessionHandle.micSender` : un
            // cycle, et un cycle que la tâche 10 ne pourrait de toute façon pas
            // écrire, `micro.ts` naissant à la tâche 11. On arrête donc la piste
            // du sender directement — ce qui suffit à l'exigence, `detacher()`
            // ne faisant rien de plus que `replaceTrack(null)` et ce `stop()`.
            // `main.ts` appelle par ailleurs son propre détachement en fin de
            // session, pour que l'ÉTAT du bouton suive lui aussi.
            micTransceiver.sender.track?.stop();
            socket.close();
            pc.close();
        },
    };
}
