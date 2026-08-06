/**
 * Retient la dernière taille observée et dit s'il faut l'émettre.
 *
 * **Pourquoi cet objet existe** : le `ResizeObserver` de `main.ts` abandonnait
 * en silence quand le canal de contrôle n'était pas ouvert au moment où sa
 * temporisation expirait, et ne réémettait JAMAIS — l'observateur ne se
 * redéclenche que si l'élément change encore de taille. Une taille perdue
 * l'était donc à jamais (leg 10 du sous-bloc D8 : deux `Resize` relevés pour
 * cinq sessions).
 *
 * **Pur, sans DOM** : c'est ce qui le rend éprouvable.
 */
export interface Taille {
    largeur: number;
    hauteur: number;
}

export class RejeuResize {
    private derniere: Taille | undefined;
    private emise: Taille | undefined;

    /** Le `ResizeObserver` a vu une taille. */
    observer(taille: Taille): void {
        this.derniere = taille;
    }

    /** La taille à émettre, ou `undefined` s'il n'y a rien de neuf. */
    aEmettre(): Taille | undefined {
        const derniere = this.derniere;
        if (!derniere) return undefined;
        if (
            this.emise &&
            this.emise.largeur === derniere.largeur &&
            this.emise.hauteur === derniere.hauteur
        ) {
            return undefined;
        }
        return derniere;
    }

    /** L'émission a réellement eu lieu. */
    confirmer(taille: Taille): void {
        this.emise = taille;
    }
}
