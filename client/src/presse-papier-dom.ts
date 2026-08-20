// Le branchement du presse-papier sur le navigateur : la seule ligne de DOM du
// mécanisme, et rien d'autre.
//
// **Extrait AVANT d'écrire quoi que ce soit dans `main.ts`** (tâche 15, 20 août
// 2026) : `main.ts` était à 460 lignes pour un plafond de projet à 500, et le
// plan nommait déjà ce fichier comme point de chute si l'addition franchissait
// 480. La règle du dépôt est d'extraire AVANT d'ajouter — et l'extraction
// préalable a un second bénéfice, que le plan déclarait hors d'atteinte : le
// câblage devient ÉPROUVABLE. `main.ts` n'a aucune couverture ; ce fichier en a
// une, parce qu'il ne touche ni `document` ni `navigator` directement mais
// reçoit `ecrire`, `focalise` et `cible` par injection — le patron de
// `attachFullscreenAuDOM` et de `armerLeSon`.
//
// 🔴 **`navigator.clipboard.readText` n'est appelée NULLE PART, ni ici ni
// ailleurs, ni au focus ni au clic ni jamais.** C'était le geste de l'ancien
// produit (`web/index.js`), il exige une permission du navigateur, et il lit
// une ressource privée EN DEHORS de toute intention de collage. Ce module ne
// reçoit aucune capacité de lecture : son interface n'en porte pas, donc il ne
// peut pas en acquérir une par accident. **Le nouveau produit ne demande aucune
// permission de presse-papier**, et c'est le meilleur résultat de ce chantier.

import { PressePapierLocal, type Recu } from './presse-papier';

/// Ce dont ce module a besoin de la fenêtre : le retour du focus, et rien
/// d'autre. `window` s'y conforme.
export interface CibleFocus {
    addEventListener(nom: 'focus', rappel: () => void): void;
    removeEventListener(nom: 'focus', rappel: () => void): void;
}

export interface OptionsPressePapier {
    /// `navigator.clipboard.writeText`, injectée. **Écriture seule.**
    ecrire: (texte: string) => Promise<void>;
    /// `document.hasFocus()`, injectée : `writeText` échoue sur un document
    /// qui n'a pas le focus, et le tenter coûterait un échec pour rien.
    focalise: () => boolean;
    /// La source du `focus` — `window`, en production.
    cible: CibleFocus;
    /// Le bandeau. Appelé pour un refus de taille, et pour un échec répété.
    surMessage: (texte: string) => void;
}

export interface PressePapierAttache {
    /// Un `AgentControl::Clipboard` vient d'arriver.
    ///
    /// ⚠️ **Un message arrivé avant l'attache est PERDU, et c'est déclaré.** Le
    /// dépôt différé vit dans `PressePapierLocal`, donc dans l'attache
    /// elle-même : il n'y a rien dans `main.ts` pour le mémoriser,
    /// contrairement à `micAnnonce`. Sans conséquence en pratique — l'agent ne
    /// pousse qu'au CHANGEMENT, et son premier sondage prend l'état courant
    /// pour référence sans rien annoncer (D-P1-4), donc la première copie
    /// annoncée suit forcément l'établissement de la session.
    recevoir(recu: Recu): void;
    /// Retire l'écouteur de focus. **Indispensable** : sans lui il survivrait
    /// à la fin de session et écrirait le presse-papier local pour une session
    /// morte — le défaut que les détachements voisins de `main.ts` existent
    /// déjà pour éviter.
    ///
    /// ⚠️ Cette phrase disait « les QUATRE détachements voisins » : ils sont
    /// **six** (pointeur, manette, plein écran, armement, visibilité, micro),
    /// et ils l'étaient déjà quand elle a été écrite. Un compte cité doit être
    /// relu, ou ne pas être cité — corrigé par la revue transverse du 20 août
    /// 2026, qui a trouvé le même « quatre » **aux deux endroits**.
    detacher(): void;
}

export function attacherPressePapierAuDOM(options: OptionsPressePapier): PressePapierAttache {
    const { ecrire, focalise, cible, surMessage } = options;
    const etat = new PressePapierLocal();

    const ecrireSiPossible = (): void => {
        // Le refus se dit AVANT l'écriture, et il se consomme : un refus reçu
        // n'empêche pas un texte valide mémorisé plus tôt de sortir au même
        // tour, et il ne se réaffiche pas au tour suivant.
        const refus = etat.refusADire();
        if (refus !== undefined) surMessage(refus);

        const texte = etat.aEcrire(focalise());
        if (texte === undefined) return;
        void ecrire(texte).then(
            () => etat.confirmer(texte),
            () => {
                const message = etat.echouer();
                if (message !== undefined) surMessage(message);
            },
        );
    };

    const surFocus = (): void => ecrireSiPossible();
    cible.addEventListener('focus', surFocus);

    return {
        recevoir(recu: Recu): void {
            etat.recevoir(recu);
            ecrireSiPossible();
        },
        detacher(): void {
            cible.removeEventListener('focus', surFocus);
        },
    };
}
