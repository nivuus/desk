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
//
// ── L'ÉCRAN TERMINAL (sous-bloc S4, tâche 9) ────────────────────────────
// Une SECONDE cible, OPTIONNELLE, reçoit les messages TERMINAUX — et eux
// seuls. Elle est branchée ICI plutôt que dans `main.ts` pour la raison même
// qui a fait naître ce module : c'est le point d'écriture unique, et un
// appelant qui écrirait `ecran.montrer(...)` à côté de `statut.afficher(...)`
// serait deux écritures que rien n'oblige à rester d'accord. Sans cible, le
// comportement est celui d'avant S4, à la ligne près : les onze tests de
// `status.test.ts` passent INCHANGÉS, et s'ils devaient changer, c'est que la
// garde terminale aurait été affaiblie.

import type { EcranTerminal, TonTerminal } from './ecran-terminal';

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
    /// Le TON d'un message terminal, lu par l'écran seul : une fin normale
    /// n'est pas une erreur. Sans effet sur le bandeau, et sans effet du tout
    /// hors d'un message terminal. Défaut : `neutre`.
    ton?: TonTerminal;
}

export interface Statut {
    afficher(message: string, options?: OptionsAffichage): void;
    /// Masque le bandeau — sauf si un message terminal ou persistant est
    /// affiché : il doit rester visible jusqu'à ce qu'un autre message le
    /// remplace.
    masquer(): void;
    /// Lève la persistance du message courant puis masque, comme si ce
    /// message n'avait jamais porté `persistant: true` — sans toucher à la
    /// protection `terminal`, qui reste intouchable : c'est elle qui empêche
    /// une fin de session d'être écrasée par un bandeau de routine, et rien
    /// ne doit l'affaiblir.
    ///
    /// `masquer()` protège délibérément un message persistant : c'est ce qui
    /// lui permet de survivre à la minuterie d'un bandeau voisin qui ne sait
    /// pas qu'il existe (voir `OptionsAffichage.persistant`). Mais quand
    /// l'appelant qui a affiché ce message SAIT que la condition qui le
    /// justifiait a cessé (ex. une fenêtre endormie vient de se réveiller),
    /// il lui faut un moyen explicite de le dire — sans réafficher un
    /// message vide en guise de contournement, ce qui ferait clignoter le
    /// bandeau et recopierait le problème au prochain message persistant.
    expirer(): void;
}

export function creerStatut(element: CibleStatut, ecran?: EcranTerminal): Statut {
    let terminal = false;
    let persistant = false;

    const masquer = () => {
        if (terminal || persistant) return;
        element.dataset.hidden = 'true';
    };

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
            // APRÈS la garde ci-dessus, donc jamais pour un message ordinaire
            // ni persistant, et de nouveau pour un SECOND terminal — qui
            // remplace le premier, à l'écran comme au bandeau.
            if (estTerminal) ecran?.montrer(message, options?.ton ?? 'neutre');
        },
        masquer,
        expirer() {
            persistant = false;
            masquer();
        },
    };
}
