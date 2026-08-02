//! Lancement et surveillance du capteur unique de capture mutualisée.
//!
//! **`surveillance_capteur` et non `capteur`** (I7 de la revue finale de
//! branche du sous-bloc D4). `crate::capteur` existe déjà et désigne AUTRE
//! CHOSE : le capteur lui-même, c'est-à-dire le processus qui tient les N
//! duplications DXGI et les N encodeurs. Ce module-ci n'en est que la
//! supervision, vue du superviseur — il ne capture rien. Deux `capteur` dans
//! le même graphe de modules, dont celui-ci fait `use super::*`, n'attendaient
//! qu'un lecteur pressé pour se confondre.
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
    /// Vrai dès qu'un cycle de relance en cours a été signalé.
    ///
    /// **Correctif I3 de la revue finale de branche.**
    /// `PERIODE_RELANCE_CAPTEUR_MIN` espace les `spawn`, pas les LIGNES : un
    /// capteur qui remeurt aussitôt après chaque relance produisait deux
    /// lignes par seconde, indéfiniment, sur le partage CIFS — soit environ
    /// 170 000 par jour. C'est exactement le risque que la documentation de
    /// cette constante invoque, et elle n'en couvrait que la moitié.
    ///
    /// Même motif qu'`Enfant::etat_illisible_signale` (`superviseur/lanceur.rs`,
    /// correctif I2) : signaler la première fois, se taire tant que la
    /// situation se répète à l'identique, redevenir bruyant dès qu'elle cesse.
    /// Ce qui est perdu est le COMPTE des relances ; ce qui est gardé est le
    /// fait qu'un cycle a commencé — et le premier retour à la normale est
    /// journalisé, ce qui borne le silence.
    cycle_signale: bool,
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
        Ok(Self {
            pid,
            derniere_tentative: std::time::Instant::now(),
            cycle_signale: false,
        })
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
    /// **Journalise le premier tour d'un cycle de relance, puis se tait**
    /// (correctif I3, voir `cycle_signale`) : les tentatives, elles, ne
    /// cessent jamais.
    pub(super) fn surveiller(&mut self, lanceur: &LanceurDeProcessus) {
        if lanceur.capteur_vivant() {
            // Le capteur a SURVÉCU à sa période de relance : le cycle est
            // rompu, une mort ultérieure sera une information neuve.
            //
            // La condition de durée n'est pas décorative. La boucle du
            // superviseur tourne à ~10 Hz alors que `PERIODE_RELANCE_CAPTEUR_MIN`
            // vaut 500 ms : un capteur qui vivrait deux ou trois tours de
            // boucle avant de mourir serait vu vivant au moins une fois entre
            // deux relances, ce qui réarmerait le signalement à chaque cycle
            // et rendrait la parade sans effet. Exiger qu'il tienne au moins
            // aussi longtemps que l'espacement des relances est ce qui
            // distingue « il repart » de « il agonise en boucle ».
            if self.cycle_signale
                && self.derniere_tentative.elapsed() >= PERIODE_RELANCE_CAPTEUR_MIN
            {
                self.cycle_signale = false;
                tracing::info!(pid = self.pid, "capteur de nouveau stable");
            }
            return;
        }
        if self.derniere_tentative.elapsed() < PERIODE_RELANCE_CAPTEUR_MIN {
            return;
        }
        self.derniere_tentative = std::time::Instant::now();
        // Lu AVANT la tentative, et armé quoi qu'il arrive : les deux issues
        // ci-dessous journalisent, et les deux doivent se taire au tour
        // suivant si le cycle se poursuit.
        let premier_du_cycle = !self.cycle_signale;
        self.cycle_signale = true;
        match lanceur.lancer_capteur() {
            Ok(nouveau) => {
                if premier_du_cycle {
                    tracing::warn!(
                        pid_mort = self.pid,
                        pid_neuf = nouveau,
                        "capteur mort, relancé (relances suivantes silencieuses \
                         tant que le cycle se répète)"
                    );
                }
                self.pid = nouveau;
            }
            Err(erreur) => {
                // `self.pid` n'est PAS mis à jour : il reste la dernière
                // valeur connue, pour que la prochaine relance réussie
                // journalise un « mort » exact — `lancer_capteur` ne réussit
                // `Ok` qu'en atomique, un échec n'a jamais fait vivre de
                // processus (voir sa doc).
                if premier_du_cycle {
                    tracing::error!(
                        %erreur,
                        "relance du capteur échouée — retentée indéfiniment passé le délai \
                         minimal, et silencieusement tant que l'échec se répète"
                    );
                }
            }
        }
    }
}
