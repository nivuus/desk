// Plein écran et Keyboard Lock.
//
// Le §4.1 du cadrage jeux fait de Windows le maître du plein écran, dans un
// sens unique. Ce module n'implémente QUE le plein écran demandé par
// l'utilisateur, qui ne touche pas à l'état de la fenêtre Windows : le
// chantier D ajoutera le sens Windows → navigateur par-dessus, sans rien
// défaire ici.
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
