// Le branchement du presse-papier sur le navigateur, DANS LES DEUX SENS : la
// seule ligne de DOM du mécanisme, et rien d'autre.
//
// ⚠️ Ce titre disait « la seule ligne de DOM du mécanisme » d'un module qui
// n'écoutait qu'un `focus` ; le sous-bloc P2 y a ajouté le `paste`, donc le
// sens navigateur → VM. La clause reste vraie — c'est toujours le seul module
// du mécanisme à toucher un écouteur —, sa portée a doublé.
//
// **Extrait AVANT d'écrire quoi que ce soit dans `main.ts`** (tâche 15, 20 août
// 2026) : `main.ts` était à 460 lignes pour un plafond de projet à 500, et le
// plan nommait déjà ce fichier comme point de chute si l'addition franchissait
// 480. La règle du dépôt est d'extraire AVANT d'ajouter — et l'extraction
// préalable a un second bénéfice, que le plan déclarait hors d'atteinte : le
// câblage devient ÉPROUVABLE. `main.ts` n'a aucune couverture ; ce fichier en a
// une, parce qu'il ne touche ni `document` ni `navigator` directement mais
// reçoit `ecrire`, `focalise`, `cible` et — depuis P2 — `emettre` par
// injection : le patron de `attachFullscreenAuDOM` et de `armerLeSon`.
//
// 🔴 **`navigator.clipboard.readText` n'est appelée NULLE PART, ni ici ni
// ailleurs, ni au focus ni au clic ni jamais.** C'était le geste de l'ancien
// produit (`web/index.js`), il exige une permission du navigateur, et il lit
// une ressource privée EN DEHORS de toute intention de collage. Ce module ne
// reçoit aucune capacité de lecture : son interface n'en porte pas, donc il ne
// peut pas en acquérir une par accident. **Le nouveau produit ne demande aucune
// permission de presse-papier**, et c'est le meilleur résultat de ce chantier.

import { encodeClipboard } from '../../proto/ts/control';
import { PRESSE_PAPIER_MAX, PressePapierLocal, messageDeRefus, type Recu } from './presse-papier';

/// Ce dont ce module a besoin de la fenêtre : le retour du focus et le collage,
/// et rien d'autre. `window` s'y conforme.
///
/// ⚠️ **L'écouteur `paste` va sur la MÊME cible que le `focus`, c'est-à-dire
/// `window` en production — jamais sur le `<video>`.** La sonde du 20 août 2026
/// relève `e.target = VIDEO#remote` : un écouteur posé sur `window` le reçoit
/// par REMONTÉE, ce que la sonde vérifie. L'attacher au `<video>` le rendrait
/// muet le jour où un `video.focus()` se perd — et il se perd, la fenêtre de
/// session ayant deux boutons de coin qui prennent le focus au clic.
export interface CibleFocus {
    addEventListener(nom: 'focus' | 'paste', rappel: (event: EvenementCollage) => void): void;
    removeEventListener(nom: 'focus' | 'paste', rappel: (event: EvenementCollage) => void): void;
}

/// Ce qu'on lit d'un `ClipboardEvent`. **Structurel, jamais le type du DOM** :
/// `clipboardData` est `null`able, et le déclarer ici permet d'éprouver le cas
/// sans jsdom.
///
/// ⚠️ **`preventDefault` n'y figure pas, et ce n'est pas un oubli** : le module
/// ne l'appelle jamais. La cible du `paste` est le `<video>`, qui n'est pas
/// éditable — l'action par défaut du navigateur n'y colle rien. Empêcher une
/// action qui n'a pas lieu serait du bruit, et surprendrait le jour où le focus
/// se trouverait dans un champ de saisie.
export interface EvenementCollage {
    clipboardData: { getData(type: string): string } | null;
}

export interface OptionsPressePapier {
    /// `navigator.clipboard.writeText`, injectée. **Écriture seule.**
    ecrire: (texte: string) => Promise<void>;
    /// `document.hasFocus()`, injectée : `writeText` échoue sur un document
    /// qui n'a pas le focus, et le tenter coûterait un échec pour rien.
    focalise: () => boolean;
    /// La source du `focus` et du `paste` — `window`, en production.
    cible: CibleFocus;
    /// Émet un message sur le canal de CONTRÔLE.
    ///
    /// 🔴 **Le canal de contrôle, jamais celui des entrées**, et c'est portant :
    /// le canal d'entrées est `ordered: false, maxRetransmits: 0`, quand le
    /// contrôle est `ordered: true`. Un collage exige un ORDRE — le
    /// presse-papier Windows d'abord, `Ctrl+V` ensuite —, et un canal non
    /// ordonné ne le donnerait pas. Injecté plutôt que pris de la session :
    /// c'est ce qui rend ce fichier éprouvable.
    emettre: (message: string) => void;
    /// Le bandeau. Appelé pour un refus de taille, et pour un échec répété.
    surMessage: (texte: string) => void;
    /// Le dernier contenu reçu AVANT l'attache, à rejouer au montage.
    ///
    /// 🔴 **C'est la moitié CLIENT du legs n°3 de P1, et elle vit ICI parce que
    /// c'est ici qu'elle est TESTABLE.** `client/src/main.ts` fait
    /// `pressePapier?.recevoir(...)` alors que `pressePapier` n'est assigné que
    /// dans le `.then()` de `connectSession`, câblé APRÈS `onControl` — un
    /// message arrivé dans cet intervalle était PERDU EN SILENCE. Et depuis le
    /// sous-bloc P3, l'agent émet l'état courant À L'INSCRIPTION, c'est-à-dire
    /// très avant que le navigateur ne se connecte : le message attend dans
    /// `pending_control` et part dès l'ouverture du canal de contrôle,
    /// **c'est-à-dire possiblement AVANT que `main.ts` n'ait assigné
    /// `pressePapier`**. L'émission de l'agent tombe donc précisément dans
    /// l'intervalle du défaut.
    ///
    /// ⚠️ **`main.ts` N'A AUCUN TEST**, et il ne peut pas en avoir : module
    /// d'entrée, effets de bord au premier niveau, non importable — relevé par
    /// la commande, `ls client/src/*.test.ts` ne rend aucun `main.test.ts`, et
    /// aucun autre ne le couvre. Il ne garde donc que DEUX LIGNES DE CÂBLAGE,
    /// sur le patron exact de `micAnnonce` ; la RÈGLE — « rejouer le mémorisé
    /// au montage » — est ici, et elle y est éprouvée.
    ///
    /// ⚠️ **Le rejeu passe par le CHEMIN QUI EXISTE DÉJÀ** (`etat.recevoir`
    /// puis `ecrireSiPossible`), jamais par un second : le dépôt différé de D3
    /// doit rester le seul chemin d'écriture, y compris au montage. Une fenêtre
    /// qui n'a pas le focus mémorise et écrira à son retour.
    initial?: Recu;
}

export interface PressePapierAttache {
    /// Un `AgentControl::Clipboard` vient d'arriver.
    ///
    /// ❌ **CETTE DOC DISAIT « un message arrivé avant l'attache est PERDU, et
    /// c'est déclaré », ET LE SOUS-BLOC P3 L'A RÉFUTÉE — sur ses DEUX
    /// clauses.** Elle ajoutait « sans conséquence en pratique : l'agent ne
    /// pousse qu'au CHANGEMENT, et son premier sondage prend l'état courant
    /// pour référence sans rien annoncer (D-P1-4), donc la première copie
    /// annoncée suit forcément l'établissement de la session ».
    ///
    /// - **le message n'est plus perdu** : `main.ts` mémorise le dernier
    ///   `clipboard` reçu et le passe en `initial`, qui est rejoué au montage ;
    /// - **et « sans conséquence en pratique » est devenu FAUX** : P3 fait
    ///   émettre à l'agent l'état courant À L'INSCRIPTION de la fenêtre, très
    ///   avant que le navigateur ne se connecte. Ce message-là ne suit pas
    ///   l'établissement de la session, il le précède.
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
    const { ecrire, focalise, cible, surMessage, emettre, initial } = options;
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

    // Le rejeu de ce qui est arrivé AVANT l'attache (moitié client du legs n°3
    // de P1). Posé APRÈS `ecrireSiPossible`, dont il se sert, et AVANT les deux
    // écouteurs : rien n'en dépend, mais l'ordre de lecture suit celui du
    // raisonnement.
    if (initial !== undefined) {
        etat.recevoir(initial);
        ecrireSiPossible();
    }

    const surFocus = (): void => ecrireSiPossible();
    cible.addEventListener('focus', surFocus);

    /// L'utilisateur a collé dans la fenêtre de session (sous-bloc P2).
    ///
    /// 🔴 **C'est le seul endroit du produit où le presse-papier de
    /// l'UTILISATEUR est lu**, et il est lu par un événement `paste` DE
    /// CONFIANCE — jamais par `navigator.clipboard.readText()`, qui exigerait
    /// une permission et lirait une ressource privée EN DEHORS de toute
    /// intention de collage. Le nouveau produit ne demande aucune permission de
    /// presse-papier, et c'est le meilleur résultat de ce chantier.
    const surCollage = (event: EvenementCollage): void => {
        const texte = event.clipboardData?.getData('text/plain') ?? '';
        // Un collage VIDE n'émet rien : émettre une chaîne vide viderait le
        // presse-papier de la VM sans que l'utilisateur l'ait demandé.
        if (texte === '') return;

        // 🔴 **LA BORNE CÔTÉ CLIENT EST OBLIGATOIRE, pas une ceinture.** Sans
        // elle, l'agent la ferait bien respecter — mais le canal de contrôle
        // aurait DÉJÀ porté la charge, et le bandeau ne paraîtrait jamais :
        // l'agent refuse en journalisant, sans rien renvoyer (D-P2-10). C'est
        // ici, et ici seulement, que l'utilisateur peut être averti.
        //
        // La borne porte sur des OCTETS d'UTF-8, la même unité que celle de
        // l'agent — `TextEncoder` plutôt que `texte.length`, qui compte des
        // unités UTF-16 et laisserait passer un texte d'emojis de deux fois la
        // taille.
        const octets = new TextEncoder().encode(texte).length;
        if (octets > PRESSE_PAPIER_MAX) {
            surMessage(messageDeRefus(octets));
            return;
        }

        // Le garde n°3 de D5 : on ne renvoie jamais à l'agent ce qu'on vient de
        // recevoir de lui. `aEmettre` consomme son témoin — un utilisateur qui
        // colle DEUX fois le même texte le veut deux fois.
        const aEmettre = etat.aEmettre(texte);
        if (aEmettre === undefined) return;
        emettre(encodeClipboard(aEmettre));
    };
    cible.addEventListener('paste', surCollage);

    return {
        recevoir(recu: Recu): void {
            etat.recevoir(recu);
            ecrireSiPossible();
        },
        detacher(): void {
            cible.removeEventListener('focus', surFocus);
            cible.removeEventListener('paste', surCollage);
        },
    };
}
