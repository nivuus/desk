// Plein écran et Keyboard Lock.
//
// Le §4.1 du cadrage jeux fait de Windows le maître du plein écran, dans un
// sens UNIQUE : c'est Windows qui décide, le navigateur qui suit, jamais
// l'inverse. Ce module porte les deux entrées vers le plein écran — le
// bouton de bascule (`attachFullscreen`, geste local de l'utilisateur) et
// l'armement déclenché par le message `fullscreen` que l'agent relaie pour
// suivre l'état de la fenêtre Windows (`armerPleinEcran` /
// `armerPleinEcranAuDOM`, câblés dans `main.ts` sur `AgentControl.fullscreen`)
// — mais ni l'une ni l'autre ne remonte quoi que ce soit vers Windows : le
// navigateur ne force jamais l'état de la fenêtre distante. C'est ce qui rend
// toute oscillation impossible.
//
// Keyboard Lock n'est pas un confort : Échap est à la fois la touche de
// sortie du plein écran navigateur et la touche de menu pause de presque tous
// les jeux. Sans elle, chaque pause quitte le plein écran.
//
// Comme pointer.ts, les dépendances sont INJECTÉES plutôt que lues dans les
// objets globaux (`document`, `navigator`), ce qui rend le module testable
// sans DOM.

/** Ce dont ce module a besoin de `navigator` : seulement l'API Keyboard Lock. */
export interface NavigateurClavier {
    keyboard?: {
        lock(codes?: string[]): Promise<void>;
        unlock(): void;
    };
}

/**
 * Verrouille toutes les touches vers la page. Sans argument : c'est le mode
 * jeu. L'utilisateur sort par appui long sur Échap, comportement prévu par
 * l'API.
 *
 * Ne lève jamais : sur Firefox et Safari `navigator.keyboard` est absent, et
 * un rejet ne doit pas remonter dans le gestionnaire d'événement.
 */
export async function verrouillerClavier(navigateur: NavigateurClavier): Promise<void> {
    if (!navigateur.keyboard) return;
    try {
        await navigateur.keyboard.lock();
    } catch (error) {
        console.warn('Keyboard Lock refusé', error);
    }
}

/** Ce dont ce module a besoin de l'élément dont le plein écran est demandé. */
export interface CibleEcran {
    requestFullscreen(): Promise<void>;
}

/** Ce dont ce module a besoin du bouton de bascule. */
export interface BoutonPleinEcran {
    dataset: { actif?: string };
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

/** Ce dont ce module a besoin du document : l'état plein écran courant. */
export interface DocumentPleinEcran {
    readonly fullscreenElement: CibleEcran | null;
    exitFullscreen(): Promise<void>;
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

export interface FullscreenOptions {
    bouton: BoutonPleinEcran;
    cible: CibleEcran;
    doc: DocumentPleinEcran;
    navigateur?: NavigateurClavier;
}

/**
 * Câble le bouton de bascule sur `cible`. Le clic demande ou quitte le plein
 * écran ; `fullscreenchange` est la seule source de vérité pour l'état affiché
 * et pour le (dé)verrouillage clavier — y compris quand la sortie vient
 * d'ailleurs que du bouton (Échap après appui long, F11, etc.).
 *
 * Renvoie une fonction de détachement qui retire les deux écouteurs posés.
 */
export function attachFullscreen({ bouton, cible, doc, navigateur = {} }: FullscreenOptions): () => void {
    const onClick = (): void => {
        if (doc.fullscreenElement) {
            void doc.exitFullscreen();
        } else {
            void cible.requestFullscreen();
        }
    };

    const onChange = (): void => {
        const actif = doc.fullscreenElement === cible;
        bouton.dataset.actif = String(actif);
        if (actif) {
            void verrouillerClavier(navigateur);
        } else {
            navigateur.keyboard?.unlock();
        }
    };

    bouton.addEventListener('click', onClick);
    doc.addEventListener('fullscreenchange', onChange);

    return () => {
        bouton.removeEventListener('click', onClick);
        doc.removeEventListener('fullscreenchange', onChange);
    };
}

// Valeurs par défaut pour utilisation dans le navigateur réel (voir tâche 15 : câblage).
export function attachFullscreenAuDOM(
    options: Omit<FullscreenOptions, 'doc' | 'navigateur'> & { navigateur?: NavigateurClavier },
): () => void {
    return attachFullscreen({
        doc: document as unknown as DocumentPleinEcran,
        navigateur: navigator as unknown as NavigateurClavier,
        ...options,
    });
}

/** Ce dont l'armement a besoin d'une cible d'événements (le document). */
export interface CibleEvenement {
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

export interface ArmementOptions {
    cible: CibleEcran;
    doc: DocumentPleinEcran;
    ecouteurs: CibleEvenement;
}

/** Les gestes qui portent une activation utilisateur transitoire. */
const GESTES = ['pointerdown', 'keydown'] as const;

/**
 * Arme l'entrée en plein écran sur le prochain geste utilisateur.
 *
 * **On arme, on n'agit pas.** `requestFullscreen()` exige une activation
 * utilisateur transitoire ; un message reçu sur canal de données n'en est pas
 * une, et l'appel serait rejeté. C'est le mécanisme retenu au §4.1 du cadrage
 * jeux, et le même que le spike multi-fenêtres a validé pour `window.open()` :
 * un seul mécanisme pour les deux besoins.
 *
 * Le clavier compte autant que le pointeur : un joueur à la manette ou au
 * clavier n'a aucune raison de cliquer.
 *
 * **Keyboard Lock n'est pas à demander ici** : `attachFullscreen` verrouille
 * déjà sur `fullscreenchange`, quelle que soit l'origine de l'entrée.
 *
 * Renvoie une fonction de détachement, à appeler en fin de session.
 */
export function armerPleinEcran({ cible, doc, ecouteurs }: ArmementOptions): () => void {
    if (doc.fullscreenElement) return () => {};

    const detacher = (): void => {
        for (const geste of GESTES) ecouteurs.removeEventListener(geste, surGeste);
    };
    const surGeste = (): void => {
        detacher();
        void cible.requestFullscreen();
    };
    for (const geste of GESTES) ecouteurs.addEventListener(geste, surGeste);
    return detacher;
}

/** Valeurs par défaut pour utilisation dans le navigateur réel. */
export function armerPleinEcranAuDOM(cible: CibleEcran): () => void {
    return armerPleinEcran({
        cible,
        doc: document as unknown as DocumentPleinEcran,
        ecouteurs: document as unknown as CibleEvenement,
    });
}
