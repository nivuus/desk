// Le presse-papier reçu de la VM, côté navigateur : quoi écrire, quand, et
// quoi dire quand ça ne marche pas.
//
// **PUR — aucun `document`, aucun `navigator`, aucune promesse.** Ce module
// décide ; c'est `main.ts` qui appelle `navigator.clipboard.writeText` et lui
// rapporte le résultat. C'est le patron de `status.ts` et de `resize.ts`, et
// c'est ce qui le rend éprouvable sans DOM.
//
// Il n'importe rien de `proto/ts/control.ts` : il prend une `Recu` locale.
// C'est délibéré — le découpler du protocole est ce qui le garde pur, et un
// changement de forme du message ne doit pas traverser jusqu'ici.

/// Ce que l'agent a annoncé.
export interface Recu {
    /// Le texte à écrire, ou `null` quand l'agent a REFUSÉ le contenu parce
    /// qu'il dépassait sa borne. `null` n'est pas « rien » : c'est un refus,
    /// et il se dit.
    texte: string | null;
    /// La taille en octets — celle du texte émis, ou celle du contenu refusé.
    octets: number;
}

/// Ce que le message d'échec doit porter : COMMENT rétablir, pas seulement
/// qu'il manque quelque chose. Même règle que `DETAIL_REFUS` du micro
/// (`micro.ts`), et pour la même raison — un message qui ne dit que le
/// symptôme laisse l'utilisateur sans geste à faire.
export const MESSAGE_ECHEC =
    "copie de la VM non recopiée ici — cliquez dans la fenêtre pour lui rendre le focus, puis recopiez";

/// Nombre d'échecs CONSÉCUTIFS avant de crier.
///
/// **Deux, pas un** : un premier échec est le cas ordinaire d'une fenêtre qui
/// n'a pas le focus au moment où l'agent pousse, et crier là-dessus ferait un
/// bandeau permanent sur un produit qui marche.
export const ECHECS_AVANT_MESSAGE = 2;

/// Le message de refus, qui NOMME la taille — « trop grand » seul ne dit pas
/// à l'utilisateur ce qu'il doit réduire.
export function messageDeRefus(octets: number): string {
    const kio = Math.round(octets / 1024);
    return `copie trop volumineuse (${kio} Kio) — elle n'a pas été recopiée ici, réduisez la sélection`;
}

export class PressePapierLocal {
    /// Le dernier texte reçu et pas encore écrit. **Un seul**, jamais une
    /// file : une écriture obsolète est impossible parce qu'on ne garde que
    /// le dernier.
    private enAttente: string | undefined;
    /// Le dernier texte réellement écrit — on ne le réécrit pas.
    private ecrit: string | undefined;
    /// Échecs consécutifs d'écriture.
    private echecs = 0;
    /// Le refus à dire, **consommable** : sinon le bandeau se réafficherait à
    /// chaque tour.
    private refus: string | undefined;

    /// Un message est arrivé de l'agent. **Toujours mémorisé**, même sans
    /// focus : c'est le dépôt différé.
    recevoir(recu: Recu): void {
        if (recu.texte === null) {
            // Un refus n'écrase PAS le dernier texte mémorisé : sinon il
            // effacerait un contenu valide encore non écrit.
            this.refus = messageDeRefus(recu.octets);
            return;
        }
        this.enAttente = recu.texte;
    }

    /// Ce qu'il faut écrire MAINTENANT, ou `undefined`.
    ///
    /// Sans focus on ne rend rien : `navigator.clipboard.writeText` échoue
    /// sur un document qui n'a pas le focus, et l'échec coûterait un compteur
    /// pour rien. Le texte reste en attente et sortira au retour du focus.
    aEcrire(focalise: boolean): string | undefined {
        if (!focalise) return undefined;
        if (this.enAttente === undefined) return undefined;
        if (this.enAttente === this.ecrit) return undefined;
        return this.enAttente;
    }

    /// L'écriture a réussi.
    confirmer(texte: string): void {
        this.ecrit = texte;
        // Un succès remet le compteur à zéro : sans cela un échec au démarrage
        // et un échec une heure plus tard crieraient ensemble.
        this.echecs = 0;
    }

    /// L'écriture a échoué. Rend le message à afficher au DEUXIÈME échec
    /// consécutif, `undefined` avant.
    echouer(): string | undefined {
        this.echecs += 1;
        return this.echecs >= ECHECS_AVANT_MESSAGE ? MESSAGE_ECHEC : undefined;
    }

    /// Le refus à dire, ou `undefined`. **Se consomme.**
    refusADire(): string | undefined {
        const refus = this.refus;
        this.refus = undefined;
        return refus;
    }
}
