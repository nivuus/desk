// Le canal de données du pont fichiers : une `RTCPeerConnection` DÉDIÉE, SANS
// MÉDIA, portant un unique data channel `fichiers`.
//
// 🔴 POURQUOI UNE CONNEXION À PART (décision D4 du plan de F1). Le cadrage
// promet qu'« une panne du canal fichiers ne touche jamais le flux vidéo ».
// Partager la `PeerConnection` d'une fenêtre ferait qu'une reconnexion de l'une
// emporterait l'autre, et lierait le pont — qui est un service de la SHELL, pas
// d'une fenêtre — au cycle de vie d'une fenêtre d'application quelconque. Or
// aucune fenêtre n'est spéciale, et c'est l'invariant que la page-shell existe
// pour tenir.
//
// ⚠️ `connectSession` N'EST PAS RÉUTILISABLE, et ce n'est pas un oubli.
// `SessionOptions` exige un `video: HTMLVideoElement`, et `connectSession`
// ajoute INCONDITIONNELLEMENT trois transceivers (`video` recvonly, `audio`
// recvonly, `audio` sendonly pour le micro) puis les canaux `input` et
// `control`. Le plan interdit de la refactorer pour rendre la vidéo
// optionnelle : le coût serait une régression possible sur le chemin critique
// de TOUTES les fenêtres, contre une trentaine de lignes dupliquées ici. Ce qui
// est réemployé SANS ÊTRE COPIÉ : `waitForAnswer`, `waitForIceGathering` et
// `attendreConfigIce`, les trois exportées de `webrtc.ts` (les deux dernières
// l'ont été PAR ce sous-bloc, modification déclarée dans leur documentation).

import {
    attendreConfigIce,
    waitForAnswer,
    waitForIceGathering,
} from '../webrtc';
import { jetonAcces } from '../jeton';
import { composer, lirePrefixe } from '../prefixe';
import type { Racine } from './adaptateur';

/**
 * Nom réservé de la session de signaling du pont fichiers.
 *
 * ⚠️ MIROIR de `NOM_SESSION_DU_PONT` (`agent/src/superviseur/protocole.rs`) :
 * les deux bouts composent le MÊME identifiant, et une divergence ne se verrait
 * qu'en session réelle. C'est exactement le régime de `SEPARATEUR`
 * (`client/src/prefixe.ts`) et de `NOM_SESSION_DE_CONTROLE`
 * (`client/src/shell-page.ts`), et la même dette : ce dépôt n'a pas de source
 * unique pour les noms de session, seulement des miroirs commentés.
 *
 * ⚠️ CE N'EST PAS UN IDENTIFIANT À LUI SEUL : il se compose avec le préfixe de
 * la VM (sous-bloc P3), sans quoi deux VMs se disputeraient la même session sur
 * la plateforme et la seconde serait refusée.
 */
export const NOM_SESSION_DU_PONT = 'fichiers';

export interface OptionsCanal {
    signalingUrl: string;
    /// L'identifiant COMPLET, préfixe compris. `sessionDuPont()` le compose.
    sessionId: string;
    onStatus?: (message: string) => void;
    /// Appelé pour chaque trame reçue. Rend la trame à réémettre, ou `null`.
    traiter(octets: ArrayBuffer): Promise<ArrayBuffer | null>;
}

export interface CanalFichiers {
    pc: RTCPeerConnection;
    canal: RTCDataChannel;
    close(): void;
}

/** L'identifiant de session du pont pour la VM courante. */
export function sessionDuPont(): string {
    return composer(lirePrefixe(), NOM_SESSION_DU_PONT);
}

export async function connecterCanalFichiers(options: OptionsCanal): Promise<CanalFichiers> {
    const statut = options.onStatus ?? (() => {});

    const socket = new WebSocket(options.signalingUrl);
    await new Promise<void>((resolve, reject) => {
        socket.addEventListener('open', () => resolve(), { once: true });
        socket.addEventListener('error', () => reject(new Error('signaling injoignable')), {
            once: true,
        });
    });
    socket.send(
        JSON.stringify({ role: 'client', session: options.sessionId, jeton: jetonAcces() }),
    );

    const iceServers = await attendreConfigIce(socket, 2000);
    const pc = new RTCPeerConnection({ iceServers });

    // 🔴 AUCUN `addTransceiver` : c'est tout l'objet de D4. Une offre sans
    // m-line média est licite ; l'agent y répond par `pont/transport.rs`, dont
    // le `Rtc` est construit sans codec.
    //
    // 🔴 FIABLE ET ORDONNÉ, c'est-à-dire les valeurs PAR DÉFAUT. La fiabilité
    // n'est pas demandée explicitement parce qu'elle n'a pas de drapeau : on
    // l'obtient en ne posant NI `maxRetransmits` NI `maxPacketLifeTime`. C'est
    // l'exact opposé du canal `input` (`webrtc.ts`, `maxRetransmits: 0`), et la
    // raison est inverse : une position de souris périmée n'a aucune valeur,
    // une plage d'octets perdue est un fichier corrompu.
    const canal = pc.createDataChannel('fichiers', { ordered: true });
    // ⚠️ SANS CECI, `event.data` PEUT ÊTRE UN `Blob`. Le défaut par défaut de
    // `RTCDataChannel.binaryType` est `'blob'` dans la spécification ; les
    // navigateurs qui ne gèrent que `'arraybuffer'` s'en tirent, les autres
    // livreraient un `Blob` que `decoder()` refuserait — un mode de défaillance
    // qui dépend du navigateur, donc invisible en recette sur un seul.
    canal.binaryType = 'arraybuffer';

    canal.addEventListener('open', () => statut('canal fichiers ouvert'));
    canal.addEventListener('close', () => statut('canal fichiers fermé'));
    canal.addEventListener('message', (evenement) => {
        const donnees: unknown = evenement.data;
        if (!(donnees instanceof ArrayBuffer)) {
            // Le pont n'émet que du binaire. Une chaîne ici n'est pas une trame.
            console.warn('trame fichiers non binaire, ignorée');
            return;
        }
        void options
            .traiter(donnees)
            .then((reponse) => {
                // ⚠️ ON NE REPOND QUE SI LE CANAL EST ENCORE OUVERT. `traiter`
                // est asynchrone — une lecture de fichier peut survivre à la
                // fermeture —, et `send` sur un canal fermé LÈVE.
                if (reponse !== null && canal.readyState === 'open') canal.send(reponse);
            })
            .catch((e: unknown) => {
                // `traiter` répond lui-même aux échecs qu'il sait nommer ; s'il
                // lève, c'est que le protocole lui-même a cassé. On le dit, et
                // on ne tue pas le canal pour autant.
                console.warn('trame fichiers non traitée', e);
            });
    });

    pc.addEventListener('connectionstatechange', () => {
        statut(`pont fichiers : ${pc.connectionState}`);
    });

    const offre = await pc.createOffer();
    await pc.setLocalDescription(offre);
    await waitForIceGathering(pc);

    statut('offre du pont envoyée, attente de l’agent…');
    socket.send(JSON.stringify({ type: 'offer', sdp: pc.localDescription!.sdp }));

    const reponse = await waitForAnswer(socket);
    await pc.setRemoteDescription({ type: 'answer', sdp: reponse });
    statut('pont fichiers : réponse reçue');

    return {
        pc,
        canal,
        close() {
            canal.close();
            pc.close();
            socket.close();
        },
    };
}

/**
 * Ouvre le sélecteur de dossier et rend la racine.
 *
 * ⚠️ `showDirectoryPicker()` EXIGE UNE ACTIVATION UTILISATEUR TRANSITOIRE :
 * cette fonction doit être appelée DANS un gestionnaire de clic, jamais depuis
 * un message de canal. C'est la même contrainte que `window.open()`, déjà
 * connue de la page-shell.
 *
 * ⚠️ LA POIGNÉE N'EST PAS PERSISTÉE. Elle est sérialisable en IndexedDB, mais
 * au rechargement la permission doit être re-accordée par `requestPermission()`,
 * qui exige à son tour une activation utilisateur : persister n'économiserait
 * que la traversée de l'arborescence dans le sélecteur, jamais le geste. Un
 * clic par chargement de la page-shell, et c'est tout.
 *
 * `mode: 'read'` — F1 vit tout entier en lecture seule (spec §8).
 *
 * ⚠️ REND `null` QUAND L'UTILISATEUR ANNULE. Le navigateur signale l'annulation
 * par une `AbortError`, c'est-à-dire par le même canal qu'une vraie panne :
 * remonter l'exception telle quelle ferait afficher « le lecteur n'a pas pu
 * être monté » à quelqu'un qui vient simplement de cliquer « Annuler ». Un
 * message d'échec sur un geste délibéré apprend à l'utilisateur à ignorer les
 * messages d'échec.
 *
 * ⚠️ CE MODULE N'A PAS DE TEST : il touche `globalThis`, `WebSocket` et
 * `RTCPeerConnection`, qui n'existent pas sous le Node de Vitest. C'est
 * exactement pour cela que `protocole.ts` et `adaptateur.ts` n'en dépendent
 * pas — toute la logique vit là-bas, testée ; ici il n'y a que du câblage.
 */
export async function choisirDossier(): Promise<{ racine: Racine; nom: string } | null> {
    const global = globalThis as {
        showDirectoryPicker?: (o?: { mode?: 'read' | 'readwrite' }) => Promise<
            FileSystemDirectoryHandle
        >;
    };
    if (typeof global.showDirectoryPicker !== 'function') {
        throw new Error(
            'ce navigateur n’expose pas la File System Access API ' +
                '(Chromium 86+ requis, hors navigation privée)',
        );
    }
    let poignee: FileSystemDirectoryHandle;
    try {
        poignee = await global.showDirectoryPicker({ mode: 'read' });
    } catch (e) {
        if (e instanceof DOMException && e.name === 'AbortError') return null;
        throw e;
    }
    // 🔴 L'AFFECTATION CI-DESSOUS EST LE CONTRÔLE DE COMPATIBILITÉ STRUCTURELLE
    // entre `FileSystemDirectoryHandle` et les interfaces de `adaptateur.ts`.
    // Elles décrivent un SOUS-ENSEMBLE de la vraie poignée, précisément pour
    // qu'un faux en mémoire puisse les satisfaire sous Node ; si la vraie ne les
    // satisfaisait plus, `tsc --noEmit` le dirait ICI, à la compilation, et non
    // en session réelle.
    //
    // ⚠️ ELLE DÉPEND DE `"DOM.AsyncIterable"` DANS `client/tsconfig.json` :
    // sans cette bibliothèque, `FileSystemDirectoryHandle` n'a pas de `values()`
    // du tout et l'affectation échoue. Elle y a été ajoutée par ce sous-bloc.
    const racine: Racine = poignee;
    return { racine, nom: poignee.name };
}
