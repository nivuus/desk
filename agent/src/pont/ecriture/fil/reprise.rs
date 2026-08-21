//! **La reprise au démarrage** : ce qu'on fait des écritures que le journal
//! déclarait dues quand le pont s'est arrêté.
//!
//! # Pourquoi cette extraction
//!
//! `fil.rs` était à **505** lignes après l'extraction de [`super::mutations`],
//! soit **cinq de trop**. Ce dépôt interdit nommément de compresser pour
//! repasser sous la ligne — D9 l'a fait deux fois avant de devoir extraire
//! quand même —, et il restait une responsabilité entière à sortir.
//!
//! # La ligne de partage
//!
//! [`super`] porte le **régime établi** : une notification arrive, on
//! journalise, on annonce, on pousse. Ce module porte le **démarrage**, qui
//! obéit à d'autres règles — l'ordre d'inscription du journal fait foi, et un
//! fichier local disparu doit être RETIRÉ en étant NOMMÉ plutôt que repoussé
//! indéfiniment.

use crate::pont::ecriture::Evenement;

use super::{disque, Fil};

impl Fil {
    /// Au démarrage : annoncer les dues **avant** toute poussée, puis les
    /// repousser **dans l'ordre d'inscription**.
    pub(super) fn reprendre(&mut self) {
        if self.journal.est_vide() {
            return;
        }
        tracing::warn!(
            dues = self.journal.compte(),
            "des ecritures etaient dues au demarrage du pont : elles sont repoussees"
        );
        self.annoncer_les_dues();
        let a_reprendre: Vec<String> =
            self.journal.dues().iter().map(|(c, _)| c.clone()).collect();
        for chemin in a_reprendre {
            let local = disque::local(&self.config.racine, &chemin);
            let evenement = match std::fs::metadata(&local) {
                Ok(m) if m.is_dir() => Evenement::Cree { chemin, repertoire: true },
                Ok(_) => Evenement::Modifie { chemin },
                Err(erreur) => {
                    // 🔴 **La racine a été recréée, et le fichier est parti avec
                    // elle** (spec §6.4 cas 3). Boucler sur le réessai ferait
                    // repousser indéfiniment un fichier qui n'existe plus ; le
                    // retirer EN LE NOMMANT est tout ce qui reste — *savoir ce
                    // qu'on a perdu n'est pas l'avoir*.
                    tracing::warn!(
                        chemin, %erreur,
                        "ecriture due abandonnee : le fichier local n'existe plus"
                    );
                    let ligne = self.journal.retirer(&chemin);
                    self.ecrire_journal(&ligne);
                    continue;
                }
            };
            if let Some(a_pousser) = self.file.signaler(evenement) {
                self.commencer(a_pousser);
            }
        }
        self.annoncer_les_dues();
    }}
