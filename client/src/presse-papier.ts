// Le presse-papier reçu de la VM, côté navigateur : quoi écrire, quand, et
// quoi dire quand ça ne marche pas.
//
// **PUR — aucun `document`, aucun `navigator`, aucune promesse.** Ce module
// décide ; c'est `presse-papier-dom.ts` qui appelle `writeText` — reçue par
// injection, depuis `main.ts` — et qui lui rapporte le résultat (`confirmer`,
// `echouer`). C'est le patron de `status.ts` et de `resize.ts`, et c'est ce
// qui le rend éprouvable sans DOM.
//
// ⚠️ **Cette phrase disait « c'est `main.ts` qui […] lui rapporte le
// résultat », et c'était vrai quand elle a été écrite.** L'extraction de
// `presse-papier-dom.ts` — jouée AVANT l'addition, dans la même branche — a
// déplacé le câblage : `main.ts` ne construit plus `PressePapierLocal` et ne
// lui rapporte plus rien (`grep -c PressePapierLocal client/src/main.ts`
// rend **0**). Corrigée par la revue transverse du 20 août 2026. C'est le
// mode de défaillance dominant de ce dépôt : une affirmation devenue fausse
// **dans sa propre branche**.
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

/// Taille maximale, en **octets d'UTF-8**, d'un texte que la page accepte
/// d'émettre vers l'agent (sous-bloc P2).
///
/// 🔴 **C'EST UNE COPIE, ET RIEN DANS LE LANGAGE NE LA CONFRONTE À SA
/// SOURCE.** La valeur qui fait foi est `agent::presse_papier::PRESSE_PAPIER_MAX`
/// (`agent/src/presse_papier.rs`), et `client/` ne peut pas importer de Rust.
/// Le dépôt a déjà payé cette classe — deux constantes écrites dans deux
/// langages sans `import` possible divergent en SILENCE (sous-bloc P2 de la
/// plateforme). Le remède employé est le même qu'alors : **un test qui relit
/// le fichier Rust et refuse la divergence**, dans `presse-papier.test.ts`.
///
/// **Pourquoi la borne est ici et pas seulement chez l'agent** : sans elle,
/// l'agent la ferait bien respecter, mais le canal de contrôle aurait DÉJÀ
/// porté la charge, et le bandeau ne paraîtrait jamais — l'agent refuse en
/// journalisant, sans rien renvoyer. C'est ici, et ici seulement, que
/// l'utilisateur peut être averti.
export const PRESSE_PAPIER_MAX = 64 * 1024;

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
    /// Le dernier texte REÇU de l'agent et pas encore réémis — le **garde n°3
    /// de D5**, et il se **consomme**.
    ///
    /// Il est posé par `recevoir`, jamais par `confirmer` : un texte reçu sans
    /// focus reste en attente d'écriture, et l'utilisateur peut coller
    /// entre-temps. Les deux cas sont indiscernables de l'extérieur, et le
    /// choix est de se taire — un aller-retour évité de trop coûte un collage
    /// répété que l'utilisateur peut refaire, là où un aller-retour de trop
    /// est du trafic que rien ne borne (legs n°4 de P1).
    private recuNonReemis: string | undefined;

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
        // Arme le garde n°3 : ce texte-là ne repartira pas vers l'agent.
        this.recuNonReemis = recu.texte;
    }

    /// Ce qu'il faut écrire MAINTENANT, ou `undefined`.
    ///
    /// Sans focus on ne rend rien : `navigator.clipboard.writeText` échoue
    /// sur un document qui n'a pas le focus, et l'échec coûterait un compteur
    /// pour rien. Le texte reste en attente et sortira au retour du focus.
    ///
    /// ⚠️ **CETTE PHRASE AFFIRMAIT COMME UN FAIT CE QUE LA SPEC DÉCLARE
    /// SUPPOSÉ DEPUIS LE 28 JUILLET 2026** (§3.3). Le sous-bloc P3 l'a
    /// mesurée, et le verdict est plus fin que « vrai » ou « faux » :
    ///
    /// - la cellule qui TRANCHE — pas de focus, MAIS sous activation
    ///   utilisateur — est **INATTEIGNABLE** à ce montage : le geste de
    ///   confiance REND le focus à la fenêtre qui le reçoit, et
    ///   `Page.bringToFront` ne le lui reprend plus. Le §3.3 reste donc
    ///   **supposé au sens strict** ;
    /// - **mais il est CORROBORÉ par une pièce** : sans focus et sans geste,
    ///   `writeText` refuse en NOMMANT le focus —
    ///   `NotAllowedError: … Document is not focused.` — là où le refus
    ///   d'activation dit `… Write permission denied.` Les deux portent le
    ///   MÊME NOM et des MESSAGES DIFFÉRENTS : **un chemin de refus propre au
    ///   focus existe, et il se nomme lui-même**. Ce qui reste non mesuré est
    ///   s'il survit à une activation.
    ///
    /// 🔵 **ET À N FENÊTRES, CE TEST FAIT AUTRE CHOSE QUE SE PROTÉGER D'UN
    /// REFUS — il ÉLIT l'unique écrivain local.** Le capteur pousse le contenu
    /// à TOUTES les fenêtres (D3), chacune a son propre `PressePapierLocal`,
    /// et si toutes écrivaient, N appels concurrents à `writeText` partiraient
    /// pour une seule copie, le dernier gagnant arbitrairement. Cette
    /// justification-là vaut **indépendamment** du §3.3, et c'est pourquoi la
    /// règle reste même si le §3.3 devait être réfuté un jour. Le retrait de
    /// la règle serait alors une **décision du propriétaire du dépôt**, avec
    /// son coût nommé — un régime que rien ne mesure —, jamais une conséquence
    /// mécanique d'un verdict de sonde.
    ///
    /// ⚠️ **Sondes versées** : `journaux-presse-papier-p3/p3-writetext-{1,2}.json`
    /// (le 2×2) et `p3-focus-{1,2}.json` (la mesurabilité du focus à N
    /// fenêtres), deux exécutions chacune, relevés identiques.
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

    /// Rend le texte à émettre vers l'agent, ou `undefined` si c'est l'écho
    /// d'un contenu qu'on vient de recevoir de lui — **le garde n°3 de D5**.
    ///
    /// **Il ne vaut que pour le PREMIER renvoi**, et c'est délibéré : un
    /// utilisateur qui colle deux fois le même texte le veut deux fois. Le
    /// doublon n'en coûte rien côté VM — le garde n°2 de l'agent l'absorbe,
    /// le presse-papier Windows portant déjà ce contenu.
    ///
    /// ⚠️ **Un refus n'arme pas ce garde** : rien n'a été écrit localement,
    /// donc rien ne peut en être l'écho.
    aEmettre(texte: string): string | undefined {
        const recu = this.recuNonReemis;
        this.recuNonReemis = undefined;
        return recu === texte ? undefined : texte;
    }

    /// Le refus à dire, ou `undefined`. **Se consomme.**
    refusADire(): string | undefined {
        const refus = this.refus;
        this.refus = undefined;
        return refus;
    }
}
