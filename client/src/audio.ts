// Déblocage du son au premier geste utilisateur.
//
// Chrome bloque la lecture audio sans activation utilisateur, et l'activation
// obtenue sur la page d'accueil NE FRANCHIT PAS l'ouverture d'une nouvelle
// fenêtre — mesuré par le spike multi-fenêtres du 28/07/2026. La session
// démarre donc muette et se démute au premier geste, quel qu'il soit.
//
// C'est le troisième usage du même ressort d'armement, après l'ouverture de
// fenêtre (variante 3 du spike) et la bascule plein écran (cadrage jeu §4.1).
// Aucun clic n'est imposé : celui qui sert à jouer suffit.
//
// Les dépendances sont INJECTÉES plutôt que lues dans les objets globaux, ce
// qui rend le module testable sans DOM.

/// Ce dont ce module a besoin d'un élément média : rien d'autre que `muted`.
export interface CibleMedia {
    muted: boolean;
}

/// Ce dont il a besoin d'une cible d'écoute.
export interface CibleGeste {
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

export interface OptionsSon {
    media: CibleMedia;
    cible: CibleGeste;
    /// Appelé avec `false` à l'armement, puis `true` au démutage. De quoi
    /// afficher — et retirer — un bandeau « cliquez pour activer le son ».
    surEtat?: (actif: boolean) => void;
}

/// Gestes qui valent activation utilisateur pour Chrome.
const GESTES = ['pointerdown', 'keydown'] as const;

/// Arme le démutage. Renvoie une fonction d'annulation qui retire les
/// écouteurs sans démuter.
export function armerLeSon(options: OptionsSon): () => void {
    const { media, cible, surEtat } = options;
    let fait = false;

    const retirer = () => {
        for (const geste of GESTES) {
            cible.removeEventListener(geste, activer);
        }
    };

    // Nommée pour pouvoir être retirée. Les écouteurs sont retirés dès le
    // premier geste : sans cela, chaque geste ultérieur reforcerait
    // `muted = false` et écraserait le choix d'un utilisateur qui aurait
    // coupé le son lui-même.
    function activer(): void {
        if (fait) return;
        fait = true;
        media.muted = false;
        retirer();
        surEtat?.(true);
    }

    for (const geste of GESTES) {
        cible.addEventListener(geste, activer);
    }
    surEtat?.(false);

    return () => {
        if (fait) return;
        fait = true;
        retirer();
    };
}
