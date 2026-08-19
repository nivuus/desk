// La page-shell : le bureau. C'est elle qui ouvre une fenêtre navigateur par
// fenêtre Windows, et elle seule — aucune page d'application n'a ce pouvoir.
//
// Pourquoi une page dédiée plutôt que la première page d'application : sans
// elle, fermer cette première page couperait la capacité d'ouvrir toutes les
// suivantes. Ici, aucune fenêtre d'application n'est spéciale.
//
// Toute la logique est ici, séparée du DOM et du WebSocket, pour être
// testable : `creerBureau` reçoit ses effets par injection.

export interface FenetreConnue {
    session: string;
    titre: string;
    ouverte: boolean;
}

export interface OptionsBureau {
    /// Rend `null` si le navigateur a bloqué l'ouverture.
    ouvrirFenetre(session: string, titre: string): Window | null;
    envoyer(message: unknown): void;
    afficher(message: string): void;
    /// L'état du lecteur de fichiers, séparé du bandeau général : les deux
    /// messages ne se chassent pas l'un l'autre.
    afficherEtatFichiers(texte: string): void;
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
            options.afficher(
                `« ${titre} » n'a pas pu s'ouvrir : le navigateur a bloqué la pop-up. ` +
                `Autorisez les pop-ups pour ce site, puis rouvrez la fenêtre.`,
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
            options.afficher(`« ${titre} » n'a pas pu s'ouvrir : ${motif}.`);
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
            options.afficherEtatFichiers(`Lecteur « Mes Fichiers » monté sur « ${nom} ».`);
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
            options.afficherEtatFichiers('');
        },

        lecteurEchoue(motif) {
            options.afficherEtatFichiers(`Le lecteur « Mes Fichiers » n’a pas pu être monté : ${motif}.`);
        },
    };
}
