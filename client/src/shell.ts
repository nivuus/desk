// La page-shell : le bureau. C'est elle qui ouvre une fenêtre navigateur par
// fenêtre Windows, et elle seule — aucune page d'application n'a ce pouvoir.
//
// Pourquoi une page dédiée plutôt que la première page d'application : sans
// elle, fermer cette première page couperait la capacité d'ouvrir toutes les
// suivantes. Ici, aucune fenêtre d'application n'est spéciale.
//
// Toute la logique est ici, séparée du DOM et du WebSocket, pour être
// testable : `creerBureau` reçoit ses effets par injection.

/**
 * Le TON d'un bandeau — l'un des quatre de la famille `message` des primitives
 * (sous-bloc S2). C'est une RÈGLE, et elle vit ici plutôt que dans le câblage :
 * `shell-page.ts` ne fait que poser la classe correspondante, et une condition
 * qui apparaîtrait là-bas serait au mauvais endroit.
 *
 * ⚠️ `alerte` N'A AUCUN APPELANT DANS CE FICHIER, et c'est délibéré : la page-
 * shell n'a aujourd'hui aucun état qui soit un avertissement sans être un
 * refus. Le ton existe dans la famille de primitives, et le type le nomme pour
 * que le jour où un tel état apparaît, il ne soit pas dit en `danger` faute
 * d'avoir le mot sous la main.
 */
export type Ton = 'neutre' | 'succes' | 'alerte' | 'danger';

export interface FenetreConnue {
    session: string;
    titre: string;
    ouverte: boolean;
}

/** Une écriture DUE : des octets qui vivent sur la VM et pas encore ici. */
export interface EcritureDue {
    chemin: string;
    octets: number;
}

export interface OptionsBureau {
    /// Rend `null` si le navigateur a bloqué l'ouverture.
    ouvrirFenetre(session: string, titre: string): Window | null;
    envoyer(message: unknown): void;
    afficher(message: string, ton: Ton): void;
    /// L'état du lecteur de fichiers, séparé du bandeau général : les deux
    /// messages ne se chassent pas l'un l'autre.
    afficherEtatFichiers(texte: string, ton: Ton): void;
    /// Le compteur d'écritures dues.
    ///
    /// 🔴 **`dues` ET `vues` SONT DEUX NOMBRES, ET LE SECOND EST CUMULATIF.**
    /// `dues` redescend, `vues` jamais. Un `dues = 0` **seul** ne dit RIEN :
    /// c'est aussi ce que rend une machine où rien n'a encore eu lieu. *Un
    /// verdict négatif exige que la chose mesurée soit ABSENTE, pas seulement
    /// nulle* — la sonde P0 du presse-papier a rendu un faux verdict
    /// éliminatoire pour avoir lu trois zéros sur une VM saine.
    afficherEcrituresDues(dues: number, vues: number, texte: string, ton: Ton): void;
    /**
     * **F5** — le pont RETIENT ses écritures dues : le répertoire annoncé n'est
     * pas celui qui a été enregistré (spec §6.4 cas 2).
     *
     * 🔴 **Le bouton « Reprendre l'enregistrement » n'apparaît QUE si c'est
     * vrai**, et disparaît sinon. *Un bouton toujours présent qui ne fait rien
     * la plupart du temps est un piège à clic* : l'utilisateur qui l'a vu inerte
     * dix fois ne le verra plus le jour où il compte.
     */
    afficherRetenues(retenues: boolean): void;
}

export interface Bureau {
    fenetreOuverte(session: string, titre: string): void;
    fenetreFermee(session: string): void;
    refus(titre: string, motif: string): void;
    viewportRecu(session: string, largeur: number, hauteur: number): void;
    liste(): FenetreConnue[];
    rouvrir(session: string): void;
    /// Le lecteur `Mes Fichiers` est monté sur le dossier `nom`.
    lecteurMonte(nom: string): void;
    /// Le lecteur n'est plus monté : l'état est EFFACÉ, pas laissé en place.
    lecteurDemonte(): void;
    /// Le montage a échoué. DISTINCT de `lecteurDemonte` : « rien n'est
    /// partagé » et « le partage a raté, voici pourquoi » n'appellent pas le
    /// même geste de l'utilisateur.
    lecteurEchoue(motif: string): void;
    /// Le pont annonce ce qui n'est PAS encore arrivé sur le poste local.
    ecrituresDues(dues: EcritureDue[], retenues: boolean): void;
    /// Une écriture a échoué. Elle reste due, et elle est NOMMÉE.
    ecritureEchouee(chemin: string, motif: string): void;
    /// **F3** — une MUTATION a échoué : renommage ou suppression.
    ///
    /// 🔴 **DISTINCTE d'`ecritureEchouee`, et ce n'est pas une subtilité.** Une
    /// écriture en échec reste DUE : le pont la repoussera, et le compteur
    /// redescendra. Une mutation en échec, elle, **ne sera jamais rejouée** —
    /// ProjFS ne renvoie pas de notification pour un geste déjà accompli dans
    /// la VM. Les deux côtés ont donc DIVERGÉ, définitivement, et le seul
    /// remède est humain.
    ///
    /// ⚠️ **Elles ne se cumulent pas non plus** : les mutations en échec
    /// s'accumulent jusqu'au remontage du lecteur, alors que les écritures en
    /// échec disparaissent dès que leur chemin cesse d'être dû.
    mutationEchouee(quoi: string, motif: string): void;
    /// Faut-il prévenir l'utilisateur avant qu'il ne referme l'onglet ?
    ///
    /// ⚠️ **PRÉDICAT PUR, testé ici** ; le câblage de `beforeunload` vit dans
    /// `shell-page.ts`, qui n'est pas testé. Prévenir TOUJOURS apprendrait à
    /// l'utilisateur à ignorer l'avertissement, ce qui le rendrait inutile
    /// exactement le jour où il compte.
    doitPrevenir(): boolean;
    /// 🔴 **NEUF — CORRECTIF DU LEGS DES FREINS MANQUANTS (round de
    /// correction 1, critique ④), 25 août 2026.** Le socket de la session de
    /// contrôle (`shell-page.ts`) peut désormais recevoir un message
    /// `{type:'error'}` qu'AUCUNE branche de son aiguillage ne reconnaissait —
    /// notamment le refus de volume `trop-de-requetes` que ce même lot vient
    /// d'ouvrir sur `/signal` (`signaling/relais.ts`). Sans cette méthode, la
    /// page restait affichée « bureau connecté » et mourait en silence : la
    /// panne muette exacte que ce dépôt combat, ouverte par ce lot lui-même.
    ///
    /// `motif` prime sur `reason` quand les deux sont absents de sens pour
    /// l'utilisateur — VOIR L'IMPLÉMENTATION, qui documente l'arbitrage.
    canalDeControleRefuse(reason: string | undefined, motif: string | undefined, retryApresS: number | undefined): void;
    /// Le socket de la session de contrôle s'est fermé alors qu'il était
    /// ouvert — perte réseau, redémarrage du service, ou fin d'un refus. Le
    /// même défaut de silence que ci-dessus, sur l'événement `close` plutôt
    /// que sur un message `error` : `shell-page.ts` n'installait AUCUN
    /// écouteur `close` ni `error` sur ce socket avant ce correctif.
    canalDeControlePerdu(): void;
}

interface Entree {
    titre: string;
    fenetre: Window | null;
}

export function creerBureau(options: OptionsBureau): Bureau {
    const connues = new Map<string, Entree>();
    /** Les écritures dues à l'instant. Redescend à zéro. */
    let dues: EcritureDue[] = [];
    /**
     * Le nombre CUMULÉ de dues jamais vues. **Monotone, jamais remis à zéro.**
     *
     * 🔴 C'est ce qui distingue « rien n'est dû » de « rien n'a eu lieu ». Sans
     * lui, `data-dues="0"` sur une machine saine serait indiscernable d'une
     * MESURE NON PRISE, et un verdict négatif se lirait comme un succès.
     */
    let vues = 0;
    /** Les échecs, par chemin. Ils survivent au compteur : l'entrée reste due. */
    const echecs = new Map<string, string>();
    /**
     * Les MUTATIONS en échec, dans leur ordre d'arrivée.
     *
     * 🔴 **ELLES NE DISPARAISSENT JAMAIS TOUTES SEULES**, à l'inverse des
     * écritures en échec : rien ne les rejouera. Elles sont effacées au
     * remontage du lecteur, et là seulement — c'est-à-dire par un geste de
     * l'utilisateur, qui est le seul remède.
     */
    let mutations: string[] = [];
    /**
     * **F5** — le pont retient ses dues faute de reconnaître le répertoire.
     *
     * ⚠️ **C'est un état du PONT, pas de l'interface** : il n'est pas remis à
     * zéro par un geste local, mais par la prochaine annonce.
     */
    let retenu = false;

    function redessinerLesDues(): void {
        const texte = [phraseDesDues(dues, echecs), phraseDesMutations(mutations)]
            .filter((p) => p.length > 0)
            .join(' ');
        // DANGER dès qu'un échec est nommé — l'utilisateur doit AGIR. Sinon
        // ALERTE tant qu'il reste des dues : ce n'est pas un refus, c'est une
        // attente, mais une attente qu'il ne faut pas refermer par accident.
        //
        // ⚠️ **Une MUTATION en échec est un DANGER même sans aucune due**, et
        // c'est ce qui la distingue : les deux côtés ont divergé, et rien ne
        // les réconciliera tout seul.
        const ton: Ton =
            echecs.size > 0 || mutations.length > 0
                ? 'danger'
                : dues.length > 0
                  ? 'alerte'
                  : 'neutre';
        // ⚠️ **RETENIR EST UNE ALERTE, jamais un `neutre`** : rien ne repartira
        // sans un geste, et un ton neutre laisserait croire que le pont
        // travaille encore.
        const tonFinal: Ton = retenu ? 'alerte' : ton;
        options.afficherEcrituresDues(dues.length, vues, texte, tonFinal);
    }

    function ouvrir(session: string, titre: string): void {
        const fenetre = options.ouvrirFenetre(session, titre);
        if (!fenetre) {
            // DANGER : l'utilisateur doit AGIR — autoriser les pop-ups. Un
            // ton neutre laisserait croire que la fenêtre est en route.
            options.afficher(
                `« ${titre} » n'a pas pu s'ouvrir : le navigateur a bloqué la pop-up. ` +
                `Autorisez les pop-ups pour ce site, puis rouvrez la fenêtre.`,
                'danger',
            );
        }
        connues.set(session, { titre, fenetre });
    }

    return {
        fenetreOuverte(session, titre) {
            ouvrir(session, titre);
        },

        fenetreFermee(session) {
            const entree = connues.get(session);
            if (!entree) return;
            // La fenêtre Windows a disparu : sa page n'a plus rien à montrer.
            entree.fenetre?.close();
            connues.delete(session);
        },

        refus(titre, motif) {
            // DANGER : la fenêtre n'existera pas.
            options.afficher(`« ${titre} » n'a pas pu s'ouvrir : ${motif}.`, 'danger');
        },

        viewportRecu(session, largeur, hauteur) {
            // Le message vient de `postMessage` : n'importe quelle page de
            // même origine peut en émettre un. On ne relaie que ce qu'on a
            // soi-même ouvert.
            if (!connues.has(session)) return;
            options.envoyer({ type: 'viewport', session, largeur, hauteur });
        },

        liste() {
            return [...connues.entries()].map(([session, e]) => ({
                session,
                titre: e.titre,
                // `closed` est la seule source de vérité : l'utilisateur peut
                // avoir fermé la page sans que personne ne nous prévienne.
                ouverte: e.fenetre !== null && !e.fenetre.closed,
            }));
        },

        rouvrir(session) {
            const entree = connues.get(session);
            if (!entree) return;
            ouvrir(session, entree.titre);
        },

        lecteurMonte(nom) {
            // SUCCÈS — et c'est le seul état positif du produit.
            options.afficherEtatFichiers(`Lecteur « Mes Fichiers » monté sur « ${nom} ».`, 'succes');
        },

        lecteurDemonte() {
            // Le remontage est le SEUL remède à une mutation en échec : rien ne
            // la rejouera. Les effacer ici, et là seulement.
            mutations = [];
            redessinerLesDues();
            // 🔴 LA CHAÎNE VIDE, ET NON UN MESSAGE « démonté ». C'est le défaut
            // relevé en D5 : le bandeau `#status` gardait son `textContent`
            // après `expirer()`, si bien que lire le texte prouvait qu'un
            // message était ARRIVÉ, jamais qu'il était AFFICHÉ — une recette
            // entière a lu un bandeau périmé en croyant lire l'état courant.
            // Un état de lecteur qui ne s'efface pas ferait croire à un dossier
            // toujours partagé alors qu'il ne l'est plus, ce qui est pire qu'un
            // texte périmé : c'est une affirmation fausse sur une permission.
            //
            // 🔴 ET LE TON RESTE `neutre` : un bandeau VIDE ne doit pas porter
            // de couleur. Une pastille colorée sans texte serait une alarme
            // sans énoncé — le pire des deux mondes, et l'exact symétrique du
            // défaut ci-dessus. Le texte vide et le ton neutre sont DEUX
            // propriétés, et `shell.test.ts` les éprouve séparément.
            options.afficherEtatFichiers('', 'neutre');
        },

        ecrituresDues(neuves, retenues) {
            // ⚠️ **L'ANNONCE ÉCRASE, elle ne s'ajoute pas.** Le pont envoie
            // l'ÉTAT complet de son journal à chaque changement : cumuler ferait
            // qu'un chemin acquitté resterait affiché pour toujours.
            dues = neuves;
            vues += neuves.length;
            // Un chemin qui n'est plus dû n'a plus d'échec à montrer : il est
            // arrivé.
            for (const chemin of [...echecs.keys()]) {
                if (!neuves.some((d) => d.chemin === chemin)) echecs.delete(chemin);
            }
            // ⚠️ **RETENUES SANS AUCUNE DUE N'A PAS DE SENS**, et l'afficher
            // proposerait de reprendre ce qu'il n'y a pas à reprendre. Le pont
            // ne l'émet pas, mais s'en remettre à lui ferait dépendre l'interface
            // d'une propriété qu'aucun type ne garantit.
            retenu = retenues && neuves.length > 0;
            options.afficherRetenues(retenu);
            redessinerLesDues();
        },

        ecritureEchouee(chemin, motif) {
            // 🔴 **LE FICHIER EST NOMMÉ, ET LA CAUSE AUSSI.** « Une écriture a
            // échoué » ne dit pas à l'utilisateur quel document rouvrir.
            echecs.set(chemin, motif);
            redessinerLesDues();
        },

        mutationEchouee(quoi, motif) {
            // 🔴 **`quoi` PORTE LES DEUX CHEMINS D'UN RENOMMAGE** (`de → vers`),
            // parce que « impossible de renommer X » ne dit pas vers quoi — et
            // c'est précisément ce que l'utilisateur doit vérifier : la
            // destination existe peut-être déjà.
            mutations.push(`« ${quoi} » (${motif})`);
            redessinerLesDues();
        },

        doitPrevenir() {
            return dues.length > 0;
        },

        lecteurEchoue(motif) {
            // DANGER : le partage a raté, et « rien n'est partagé » n'appelle
            // pas le même geste que « le partage a raté, voici pourquoi ».
            options.afficherEtatFichiers(
                `Le lecteur « Mes Fichiers » n’a pas pu être monté : ${motif}.`,
                'danger',
            );
        },

        canalDeControleRefuse(reason, motif, retryApresS) {
            // 🔴 `reason` D'ABORD : c'est la phrase destinée à un humain
            // (`identite/garde.ts::Verdict.message`, ou le texte fixe de
            // `relais.ts` pour `trop-de-requetes`) ; `motif` est un MOT-CLÉ
            // stable pour le code, pas une phrase — voir `bureau.refus`
            // ci-dessus, qui suit la même hiérarchie pour la même raison.
            const cause = reason ?? motif ?? 'raison inconnue';
            // ⚠️ `retryApresS` n'accompagne QUE le refus de volume
            // (`signaling/relais.ts`) : un refus de poignée de main (jeton
            // absent ou invalide) n'a rien à retenter, se reconnecter ne
            // changera rien. Absent, la phrase ne promet donc rien qui ne
            // tiendrait pas.
            const attente =
                typeof retryApresS === 'number' ? ` Nouvelle tentative possible dans ${retryApresS} s.` : '';
            options.afficher(`Bureau refusé : ${cause}.${attente}`, 'danger');
        },

        canalDeControlePerdu() {
            // DANGER, jamais NEUTRE : plus aucune fenêtre ne peut s'ouvrir ni
            // se refermer tant que la page n'est pas rechargée, et le dire en
            // neutre laisserait croire à un bureau qui fonctionne encore.
            options.afficher(
                'Connexion au bureau perdue. Rechargez la page pour vous reconnecter.',
                'danger',
            );
        },
    };
}

/**
 * La phrase du compteur. **Elle NOMME les fichiers**, parce que le dialogue de
 * `beforeunload` ne le peut pas.
 *
 * ⛔ **Le message personnalisé de `beforeunload` est IGNORÉ par tous les
 * navigateurs modernes** : ils n'affichent qu'un libellé générique de leur
 * choix. La spec §6.2 demande « un `beforeunload` avec un texte qui nomme les
 * fichiers » — **ce texte n'existe pas**. Les nommer DANS LA PAGE, à côté du
 * compteur, est ce qui reste. *(Fait de plateforme, non mesuré ici, déclaré
 * comme tel.)*
 */
function phraseDesMutations(mutations: string[]): string {
    if (mutations.length === 0) return '';
    const pluriel = mutations.length > 1 ? 's' : '';
    return (
        `${mutations.length} renommage${pluriel} ou suppression${pluriel} n'${
            mutations.length > 1 ? 'ont' : 'a'
        } PAS été répercuté${pluriel} sur ce poste : ${mutations.join(', ')}. ` +
        `Les deux côtés ont divergé, et rien ne le rejouera.`
    );
}

function phraseDesDues(dues: EcritureDue[], echecs: Map<string, string>): string {
    if (dues.length === 0) return '';
    const noms = dues
        .map((d) => {
            const motif = echecs.get(d.chemin);
            return motif === undefined ? `« ${d.chemin} »` : `« ${d.chemin} » (${motif})`;
        })
        .join(', ');
    const pluriel = dues.length > 1 ? 's' : '';
    return (
        `${dues.length} fichier${pluriel} enregistré${pluriel} dans la VM n'${dues.length > 1 ? 'ont' : 'a'} ` +
        `pas encore été recopié${pluriel} sur ce poste : ${noms}. Ne fermez pas cet onglet.`
    );
}
