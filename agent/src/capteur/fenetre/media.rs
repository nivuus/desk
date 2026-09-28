//! The media connection's writer thread: what the window thread hands it, the
//! bounded depth of their queue, and the writing itself.
//!
//! Split out of `fenetre.rs` when `cargo fmt` pushed that file past 500 lines.

use std::io::Write;
use std::sync::mpsc::Receiver;

use crate::capteur::protocole::{ecrire_image, ecrire_json, DepuisCapteur};
use crate::h264::AccessUnit;

/// Profondeur de la file entre le fil de fenêtre et le fil écrivain de la
/// connexion média.
///
/// **Bornée à dessein** : une file libre laisserait s'accumuler sans limite des
/// unités d'accès qu'un enfant qui ne lit plus ne prendra jamais. C'est le
/// pendant exact de `CAPACITE_FILE` côté enfant, et la contre-pression continue
/// donc de remonter jusqu'à la capture — mais elle remonte désormais dans
/// `deposer`, qui sert les commandes à chaque tour d'attente.
pub(super) const CAPACITE_ECRITURES: usize = 8;

/// Ce que le fil de fenêtre confie au fil écrivain de la connexion média.
pub(super) enum AEcrire {
    Image(AccessUnit),
    Etat(DepuisCapteur),
}

/// Le fil écrivain de la connexion média : il ne fait qu'écrire, et il est le
/// seul à toucher cet objet fichier. Personne ne le lit.
pub(super) fn ecrire_le_media<E: Write>(
    mut ecrivain: E,
    charges: Receiver<AEcrire>,
    session: &str,
) {
    for charge in charges {
        let ecrit = match charge {
            AEcrire::Image(unite) => ecrire_image(&mut ecrivain, &unite),
            AEcrire::Etat(message) => ecrire_json(&mut ecrivain, &message),
        };
        // `flush` à chaque charge : devant une fenêtre immobile, la charge
        // suivante peut ne jamais venir, et l'enfant attendrait celle-ci dans
        // un tampon. Même leçon que la réponse d'attache de la tâche 9.
        if let Err(erreur) = ecrit.and_then(|()| ecrivain.flush()) {
            tracing::warn!(%session, %erreur, "écriture de la connexion média interrompue");
            return;
        }
    }
}
