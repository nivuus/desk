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

export interface OptionsBureau {
    /// Rend `null` si le navigateur a bloqué l'ouverture.
    ouvrirFenetre(session: string, titre: string): Window | null;
    envoyer(message: unknown): void;
    afficher(message: string, ton: Ton): void;
    /// L'état du lecteur de fichiers, séparé du bandeau général : les deux
    /// messages ne se chassent pas l'un l'autre.
    afficherEtatFichiers(texte: string, ton: Ton): void;
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
}

interface Entree {
    titre: string;
    fenetre: Window | null;
}

export function creerBureau(options: OptionsBureau): Bureau {
    const connues = new Map<string, Entree>();

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

        lecteurEchoue(motif) {
            // DANGER : le partage a raté, et « rien n'est partagé » n'appelle
            // pas le même geste que « le partage a raté, voici pourquoi ».
            options.afficherEtatFichiers(
                `Le lecteur « Mes Fichiers » n’a pas pu être monté : ${motif}.`,
                'danger',
            );
        },
    };
}
