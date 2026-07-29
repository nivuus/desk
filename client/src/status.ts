// Bandeau de statut : point d'écriture unique, qui protège un message
// TERMINAL contre l'écrasement par un message ORDINAIRE arrivant après lui.
//
// Cinq écrivains accèdent au bandeau : quatre dans `main.ts` (prêt, session
// terminée, armement du son, échec de connexion) et `webrtc.ts`, qui écrit
// via le callback `onStatus` (flux reçu, changement d'état de connexion,
// offre envoyée, réponse reçue). `connectionstatechange` se déclenche PAR
// CONSTRUCTION juste après `session-end` — `RTCPeerConnection` se ferme en
// réaction à la fin de session — donc sans garde, « connexion : disconnected »
// écrase systématiquement « session terminée : … », le message le plus
// informatif des deux. Router tous les écrivains par ce module plutôt que de
// garder l'ancien indicateur `sessionTerminee` ad hoc dans `main.ts` ferme ce
// trou à la racine : plus aucun appelant ne peut oublier la garde, parce
// qu'il n'a plus la main sur `element.textContent` directement.
//
// Les dépendances sont injectées, comme dans `audio.ts`, pour rester
// testable sans DOM.

/// Ce dont ce module a besoin d'un élément d'affichage.
export interface CibleStatut {
    textContent: string;
    dataset: { hidden?: string };
}

export interface OptionsAffichage {
    /// Un message TERMINAL (fin de session, échec) n'est plus jamais écrasé
    /// par un message ordinaire. Un second message terminal remplace bien le
    /// premier : c'est la dernière information définitive qui gagne.
    terminal?: boolean;
    /// Un message PERSISTANT résiste à `masquer()`, mais se laisse remplacer
    /// par un message suivant. Il sert aux conditions qui durent — un réseau
    /// dégradé, par exemple : leur affichage ne doit pas être effacé par la
    /// minuterie d'un bandeau voisin arrivé avant lui, minuterie que
    /// l'appelant n'a aucun moyen de connaître.
    persistant?: boolean;
}

export interface Statut {
    afficher(message: string, options?: OptionsAffichage): void;
    /// Masque le bandeau — sauf si un message terminal ou persistant est
    /// affiché : il doit rester visible jusqu'à ce qu'un autre message le
    /// remplace.
    masquer(): void;
}

export function creerStatut(element: CibleStatut): Statut {
    let terminal = false;
    let persistant = false;

    return {
        afficher(message, options) {
            const estTerminal = options?.terminal ?? false;
            if (terminal && !estTerminal) return;
            terminal = terminal || estTerminal;
            // Réaffecté à chaque appel, contrairement à `terminal` : un
            // message ordinaire qui suit une alerte lève la persistance —
            // un retour à un état normal doit pouvoir se masquer à nouveau.
            persistant = options?.persistant ?? false;
            element.textContent = message;
            element.dataset.hidden = 'false';
        },
        masquer() {
            if (terminal || persistant) return;
            element.dataset.hidden = 'true';
        },
    };
}
