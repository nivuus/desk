//! Lancement et surveillance du capteur unique de capture mutualisée.
//!
//! Extrait de `boucle.rs` (tâche 7 du sous-bloc D4) pour rester sous le
//! plafond de 500 lignes du projet — pas pour une raison de conception : cette
//! logique fait partie de la boucle comme les autres, dans le même module
//! logique, juste dans un fichier voisin. Même schéma que
//! `placement_periodique.rs`.

use anyhow::Context;

use super::*;

/// Espacement minimal entre deux tentatives de relance du capteur.
///
/// Sans cette borne, un capteur qui meurt AUSSITÔT après avoir été relancé
/// ferait retenter `lancer_capteur` — donc `Command::spawn`, un vrai processus
/// — à la cadence de la boucle : jusqu'à plusieurs fois par seconde tant
/// qu'il reste des effets à traiter. Le dépôt a déjà payé deux fois le coût
/// d'une simple LIGNE de journal émise à ce rythme sur un partage CIFS
/// (correctif I2 de `lanceur.rs`, et le chantier TURN avant lui) ; relancer un
/// processus à cette cadence serait pire. Cette constante ne fait qu'espacer
/// les tentatives, elle ne les empêche jamais : le capteur reste retenté
/// indéfiniment tant qu'il ne revient pas.
const PERIODE_RELANCE_CAPTEUR_MIN: std::time::Duration = std::time::Duration::from_millis(500);

/// Ce que la boucle retient du capteur d'un tour à l'autre : le PID de la
/// dernière tentative réussie (pour journaliser une relance avec le PID mort
/// ET le PID neuf) et l'horodatage de la dernière tentative (pour l'espacer).
pub(super) struct EtatCapteur {
    pid: u32,
    derniere_tentative: std::time::Instant,
}

impl EtatCapteur {
    /// Lance le capteur, AVANT la moindre fenêtre : c'est lui qui sert le
    /// média à tout enfant qui se rattache, et un enfant lancé sans capteur en
    /// face capturerait dans le vide.
    ///
    /// Pas de contrat atomique à défaire ici, contrairement à `lancer_capteur`
    /// lui-même : un échec de CET appel est fatal au superviseur, au même
    /// titre qu'un pilote ou qu'un hook qui ne s'ouvre pas — il n'y a rien
    /// d'autre à nettoyer.
    pub(super) fn demarrer(lanceur: &LanceurDeProcessus) -> Result<Self> {
        let pid = lanceur.lancer_capteur().context("lancement initial du capteur")?;
        Ok(Self { pid, derniere_tentative: std::time::Instant::now() })
    }

    /// Relance le capteur s'il est mort, au plus une fois par
    /// `PERIODE_RELANCE_CAPTEUR_MIN`, et journalise chaque relance réussie
    /// avec le PID mort et le PID neuf.
    ///
    /// **Ne ferme jamais aucune fenêtre.** Les enfants tiennent sur leur
    /// fenêtre de reprise (15 s, voir `capteur::distante`) et se rattachent
    /// d'eux-mêmes au capteur relancé — c'est le critère de réception n°2 du
    /// sous-bloc D4, et le fermer côté superviseur le ferait échouer par
    /// construction. Cette méthode ne fait donc rien d'autre que relancer et
    /// journaliser : aucun appel à `enfants.tuer` ni à un effet de la table
    /// n'a sa place ici.
    pub(super) fn surveiller(&mut self, lanceur: &LanceurDeProcessus) {
        if lanceur.capteur_vivant() {
            return;
        }
        if self.derniere_tentative.elapsed() < PERIODE_RELANCE_CAPTEUR_MIN {
            return;
        }
        self.derniere_tentative = std::time::Instant::now();
        match lanceur.lancer_capteur() {
            Ok(nouveau) => {
                tracing::warn!(pid_mort = self.pid, pid_neuf = nouveau, "capteur mort, relancé");
                self.pid = nouveau;
            }
            Err(erreur) => {
                // `self.pid` n'est PAS mis à jour : il reste la dernière
                // valeur connue, pour que la prochaine relance réussie
                // journalise un « mort » exact — `lancer_capteur` ne réussit
                // `Ok` qu'en atomique, un échec n'a jamais fait vivre de
                // processus (voir sa doc).
                tracing::error!(
                    %erreur,
                    "relance du capteur échouée — retentée au prochain tour passé le délai minimal"
                );
            }
        }
    }
}
