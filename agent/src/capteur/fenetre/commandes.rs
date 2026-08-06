//! Le service des commandes de l'enfant, et la contre-pression du média.
//!
//! **Extrait de `fenetre.rs`** pour la même raison que `sommeil.rs` : le
//! sous-bloc D5 aurait porté le fichier parent au-delà du plafond de 500 lignes
//! du projet.
//!
//! Toutes les fonctions d'ici prennent `source: Option<&mut WindowsSource>` :
//! `None` signifie « la fenêtre dort ». **Elles servent les commandes dans les
//! deux cas** — refuser tout pendant un sommeil ferait remonter des échecs
//! jusqu'à l'adaptation réseau de l'enfant, qui clorait la session par un
//! chemin étranger au sommeil.

use std::sync::mpsc::{SyncSender, TryRecvError, TrySendError};

use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::source::VideoSource;
use crate::windows_source::WindowsSource;

use super::{AEcrire, Contexte, Fin, PAS_A_VIDE};

/// Vide la file des commandes en attente et renvoie chaque réponse au fil de
/// commandes. **Ne bloque jamais** : `try_recv` d'un côté, `Sender` non borné
/// de l'autre.
pub(super) fn servir_les_commandes(source: Option<&mut WindowsSource>, ctx: &Contexte) -> Fin {
    let mut source = source;
    loop {
        match ctx.commandes.try_recv() {
            Ok(message) => {
                // `as_deref_mut` et non `source` : l'emprunt serait consommé
                // par l'appel, et le tour suivant en a besoin.
                let reponse = executer_commande(source.as_deref_mut(), ctx, message);
                // La réponse repart par le canal, jamais par une écriture
                // directe : ce fil ne touche aucun objet fichier.
                if ctx.reponses.send(reponse).is_err() {
                    return Fin::Terminer("le fil de commandes est parti");
                }
            }
            Err(TryRecvError::Empty) => return Fin::Continuer,
            // L'enfant a fermé sa connexion de commandes : la fenêtre est finie.
            Err(TryRecvError::Disconnected) => return Fin::Terminer("l'enfant a fermé le canal"),
        }
    }
}

/// Dépose une charge pour le fil écrivain de la connexion média.
///
/// ⚠️ **C'est le SEUL point où le fil de fenêtre peut attendre, et c'est ce qui
/// garantit qu'il ne peut jamais attendre sans servir les commandes.** La file
/// est bornée pour que la contre-pression remonte jusqu'à la capture ; quand
/// elle est pleine, on ne bloque pas dessus — on sert les commandes, on souffle
/// un pas, et on réessaie. Un `send` bloquant ici recréerait exactement
/// l'interblocage que la tâche 10 du sous-bloc D4 devait supprimer : enfant
/// figé dans `commander` → file d'images de l'enfant pleine → tampon du tube
/// plein → écriture du capteur bloquée → commande jamais servie → enfant figé.
///
/// **Y compris pour un état poussé par le sommeil** : une fenêtre qui s'endort
/// alors que la file est pleine attend ici, en servant ses commandes.
pub(super) fn deposer(
    charge: AEcrire,
    ecritures: &SyncSender<AEcrire>,
    source: Option<&mut WindowsSource>,
    ctx: &Contexte,
) -> Fin {
    let mut charge = charge;
    let mut source = source;
    loop {
        match ecritures.try_send(charge) {
            Ok(()) => return Fin::Continuer,
            Err(TrySendError::Full(rendue)) => {
                charge = rendue;
                if let Fin::Terminer(motif) = servir_les_commandes(source.as_deref_mut(), ctx) {
                    return Fin::Terminer(motif);
                }
                std::thread::sleep(PAS_A_VIDE);
            }
            // Le fil écrivain est mort : la connexion média est perdue.
            Err(TrySendError::Disconnected(_)) => {
                return Fin::Terminer("la connexion média est fermée")
            }
        }
    }
}

fn executer_commande(
    source: Option<&mut WindowsSource>,
    ctx: &Contexte,
    message: VersCapteur,
) -> DepuisCapteur {
    // Les quatre messages qui ne touchent pas la source sont traités AVANT
    // elle, pour que leur réponse soit la même endormie et éveillée :
    // `Visibilite` et `AudioMort` parce qu'ils COMMANDENT ou alimentent
    // l'arbitrage du sommeil — les ignorer pendant un sommeil interdirait
    // tout réveil ou toute promotion d'une voisine —, les deux autres parce
    // qu'une violation de protocole n'en cesse pas d'être une pendant un
    // sommeil.
    match message {
        VersCapteur::Visibilite { visible, focalisee } => {
            // L'effet ne revient PAS par cette réponse : l'arbitrage est global
            // et peut concerner une AUTRE fenêtre que celle-ci. Il revient par
            // `DepuisCapteur::Sommeil`, poussé sur la connexion média.
            crate::capteur::sommeil::signaler(ctx.session, visible, focalisee);
            return DepuisCapteur::Fait;
        }
        VersCapteur::AudioMort => {
            // L'effet ne revient PAS par cette réponse : l'arbitrage est global
            // et peut concerner une AUTRE fenêtre du même groupe de PID. Il
            // revient par `DepuisCapteur::Audio`, poussé sur la connexion
            // média. Même patron que `Visibilite` juste au-dessus.
            crate::capteur::sommeil::audio_mort(ctx.session);
            return DepuisCapteur::Fait;
        }
        VersCapteur::Attache { .. } => {
            return DepuisCapteur::Erreur {
                motif: "seconde attache sur un canal déjà attaché".into(),
            }
        }
        // `Identite` n'appartient qu'à la connexion média, où elle est la
        // première et unique trame : la voir ici signale un enfant qui confond
        // ses deux connexions.
        VersCapteur::Identite { session: autre } => {
            return DepuisCapteur::Erreur {
                motif: format!("identité de {autre} sur la connexion de commandes"),
            }
        }
        _ => {}
    }

    // Tout le reste exige la source. Endormie, il n'y a ni encodeur à régler ni
    // duplication à retailler : on répond comme si c'était fait, plutôt qu'une
    // erreur qui remonterait jusqu'à l'adaptation réseau de l'enfant et s'y
    // journaliserait comme un refus — un bruit sans objet.
    //
    // ⚠️ **Ce qu'on accepte ainsi n'est ni appliqué ni retenu.** Le réveil
    // reconstruit la source par `sur_sortie`, donc à la taille d'encodage
    // pleine et au débit d'attache. Le débit se rattrape seul —
    // `appliquer_decision` le repousse à chaque décision du contrôleur ; la
    // taille d'encodage, elle, ne se rattrape qu'au prochain changement de
    // barreau, la comparaison à `encode_size_appliquee` côté enfant croyant la
    // cible déjà appliquée. Conséquence : après un réveil, une image encodée en
    // pleine résolution au débit d'un barreau réduit, donc dégradée — jamais un
    // dépassement de débit.
    //
    // **Ce n'est PAS corrigé, et c'est une décision — mais sa raison a changé.**
    // L'obstacle technique est tombé : `set_encode_size` détruit l'encodeur
    // courant avant d'en construire un neuf (`windows_source/encodage.rs`,
    // tâche 10 de D5), il ne dépasse donc plus le plafond. Reste un arbitrage de
    // portée, plus faible : réappliquer coûterait une construction d'encodeur à
    // l'instant du réveil — celui où la session a le plus besoin de sa première
    // image — pour une dégradation qui se résorbe seule au prochain barreau.
    // **À rouvrir hors de ce sous-bloc**, son obstacle n'existant plus.
    let Some(source) = source else {
        return match message {
            // La taille retenue, telle quelle : une fenêtre endormie n'a plus
            // ni capture ni encodeur, il n'y a rien à retailler. Un `Fait`
            // ferait échouer `SourceDistante::resize`, qui attend une `Taille`.
            //
            // ⚠️ **Ce commentaire a dit successivement deux choses fausses, et
            // c'est la revue TRANSVERSE de fin de branche D9 qui l'a rattrapé —
            // aucune revue par tâche ne le pouvait.** Il a d'abord affirmé que
            // « `resize` est de toute façon sans effet sur une source en mode
            // `SortieEntiere` » ; D8 l'a réfuté en faisant suivre la sortie au
            // viewport ; le commentaire a donc été réécrit pour annoncer, comme
            // conséquence assumée, qu'un passage en plein écran demandé pendant
            // le sommeil serait **perdu**. ❌ **Cette seconde rédaction est
            // périmée depuis la tâche 3 du sous-bloc D9**, qui a retiré le
            // changement de mode de sortie sur mesure (voir le constat en tête
            // de `capteur/plein_ecran.rs`) : `resize` est redevenu, sans
            // réserve, sans effet en `SortieEntiere`
            // (`ModeCapture::redimensionne_la_fenetre` rend `false` et
            // `WindowsSource::resize` retourne avant tout), et il n'y a donc
            // plus AUCUN plein écran à perdre par ce chemin — la détection et
            // l'annonce, la seule moitié qui reste livrée, ne passent pas par
            // `Redimensionner`.
            //
            // Ce qui reste vrai, et pourquoi ce bras existe : rendre une
            // `Taille` plutôt qu'un `Fait`, parce que `SourceDistante::resize`
            // attend une `Taille`.
            VersCapteur::Redimensionner { .. } => {
                DepuisCapteur::Taille { largeur: ctx.taille.0, hauteur: ctx.taille.1 }
            }
            _ => DepuisCapteur::Fait,
        };
    };

    let resultat = match message {
        VersCapteur::Redimensionner { largeur, hauteur } => {
            return match source.resize(largeur, hauteur) {
                Ok(()) => {
                    let (largeur, hauteur) = source.dimensions();
                    DepuisCapteur::Taille { largeur, hauteur }
                }
                Err(erreur) => DepuisCapteur::Erreur { motif: format!("{erreur:#}") },
            }
        }
        VersCapteur::TailleEncodage { largeur, hauteur } => source.set_encode_size(largeur, hauteur),
        VersCapteur::Debit { bps } => source.set_bitrate(bps),
        VersCapteur::ImageCle => source.request_keyframe(),
        // Traités plus haut, avant la source, donc jamais atteints ici. Une
        // `Erreur` plutôt qu'un `unreachable!` : une panique sur ce fil
        // emporterait la fenêtre pour une faute de rédaction.
        VersCapteur::Attache { .. }
        | VersCapteur::Identite { .. }
        | VersCapteur::Visibilite { .. }
        | VersCapteur::AudioMort => {
            return DepuisCapteur::Erreur {
                motif: "commande déjà traitée hors de la source".into(),
            }
        }
    };
    match resultat {
        Ok(()) => DepuisCapteur::Fait,
        Err(erreur) => DepuisCapteur::Erreur { motif: format!("{erreur:#}") },
    }
}
