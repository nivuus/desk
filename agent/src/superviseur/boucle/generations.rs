//! Le compteur de génération des enfants (D9, F5 de D7).
//!
//! **Extrait pour la seule raison de la place qu'il rend à `boucle.rs`**,
//! qui valait exactement 500 lignes — le plafond du projet — avec ce
//! compteur posé en ligne, marge nulle. Aucune logique n'est isolée ici :
//! c'est un simple compteur monotone, testé par construction (un `u64` qui
//! ne fait qu'augmenter n'a rien à couvrir de plus qu'une lecture).
//!
//! Un rattachement relance un enfant sous le MÊME nom de session : c'est
//! cette génération, frappée une fois par lancement et relayée telle quelle
//! jusqu'au registre du capteur (`capteur::sommeil`), qui lui permet de
//! reconnaître le `retirer` d'une instance déjà remplacée comme périmé
//! plutôt que de le laisser emporter l'inscription neuve.

/// Compteur monotone, un par exécution du superviseur.
#[derive(Default)]
pub(super) struct Generations(u64);

impl Generations {
    /// Frappe et rend la génération suivante. Commence à 1 : `0` reste la
    /// valeur par défaut d'un enfant lancé hors superviseur (`Config::
    /// generation` en mode mono-fenêtre), et ne doit donc jamais être émise
    /// ici.
    pub(super) fn suivante(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}
