//! **Resumption at startup**: what we do with the writes the journal
//! declared due when the bridge stopped.
//!
//! # Why this extraction
//!
//! `fil.rs` was at **505** lines after the extraction of [`super::mutations`],
//! that is **five too many**. This repository forbids by name compressing to
//! get back under the line — D9 did it twice before having to extract
//! anyway —, and there remained a whole responsibility to move out.
//!
//! # The dividing line
//!
//! [`super`] carries the **steady state**: a notification arrives, we
//! journal, we announce, we push. This module carries **startup**, which
//! obeys other rules — the journal's registration order is authoritative, and a
//! vanished local file must be REMOVED while being NAMED rather than pushed again
//! indefinitely.

use crate::pont::bonjour::{a_memoriser, decider, Decision};
use crate::pont::ecriture::Evenement;

use super::{disque, Fil};

impl Fil {
    /// **F5** — the browser announced itself: we push, or we HOLD BACK.
    ///
    /// ⚠️ **The journal is NEVER emptied on a hold-back.** Replaying blindly
    /// would write the files of one session into the folder of another (spec
    /// §6.4 case 2); throwing away would lose the data. **We do neither one nor the other:
    /// we NAME it**, through `Dues.retenues`, and the browser offers "Resume
    /// saving".
    pub(super) fn bonjour(&mut self, racine: &str, forcer: bool) {
        let memorise = self.lire_le_nom_memorise();
        let decision = decider(memorise.as_deref(), racine, forcer);
        tracing::info!(
            racine,
            forcer,
            memorise = memorise.as_deref().unwrap_or("<aucun>"),
            decision = if decision == Decision::Pousser {
                "pousser"
            } else {
                "retenir"
            },
            dues = self.journal.dues().len(),
            "bonjour du navigateur : decision de reprise"
        );
        if let Some(nom) = a_memoriser(decision, racine) {
            self.write_remembered_name(nom);
        }
        match decision {
            Decision::Pousser => {
                self.retenues = false;
                self.reprendre();
            }
            Decision::Retenir => {
                // 🔴 **WE ANNOUNCE, AND WE DO NOT PUSH.** Without this announcement, the
                // browser would see a frozen dues counter without knowing
                // why — that is, the silence `retenues` exists to
                // break.
                self.retenues = true;
                tracing::warn!(
                    racine,
                    memorise = memorise.as_deref().unwrap_or("<aucun>"),
                    dues = self.journal.dues().len(),
                    "ecritures dues RETENUES : le repertoire annonce n'est pas celui qui a ete \
                     enregistre. Rien n'est pousse, rien n'est jete."
                );
                self.annoncer_les_dues();
            }
        }
    }

    /// The memorised root name, **next to the journal and never inside it**.
    ///
    /// 🔴 **OUTSIDE THE ROOT, and this file already says it of the journal**: a
    /// state that lived IN the root would itself be a projected object — hence
    /// dependent on the bridge to be read, hence circular — and it would disappear
    /// with it exactly the day it matters.
    fn chemin_du_nom(&self) -> std::path::PathBuf {
        self.config.chemin_journal.with_file_name("racine.nom")
    }

    fn lire_le_nom_memorise(&self) -> Option<String> {
        let brut = std::fs::read_to_string(self.chemin_du_nom()).ok()?;
        // ⚠️ **Trimmed, because an editor adds a line ending.** A name that
        // differed only by a `\n` would cause a HOLD-BACK at each startup, on
        // the right directory, with nothing saying why.
        let nom = brut.trim_end_matches(['\r', '\n']).to_string();
        // An empty file is not an empty name: nothing is memorised.
        // Confusing them would hold back on any root at the first mount.
        if nom.is_empty() && brut.is_empty() {
            return None;
        }
        Some(nom)
    }

    fn write_remembered_name(&self, nom: &str) {
        if let Err(error) = std::fs::write(self.chemin_du_nom(), nom) {
            // A `warn!` and nothing else: not memorising causes a HOLD-BACK at the
            // next startup, which is the safe side.
            tracing::warn!(%error, nom, "nom de racine non memorise");
        }
    }

    /// At startup: announce the dues **before** any push, then
    /// push them again **in registration order**.
    pub(super) fn reprendre(&mut self) {
        if self.journal.est_vide() {
            return;
        }
        tracing::warn!(
            dues = self.journal.compte(),
            "des ecritures etaient dues au demarrage du pont : elles sont repoussees"
        );
        self.annoncer_les_dues();
        let a_reprendre: Vec<String> = self.journal.dues().iter().map(|(c, _)| c.clone()).collect();
        for chemin in a_reprendre {
            let local = disque::local(&self.config.racine, &chemin);
            let evenement = match std::fs::metadata(&local) {
                Ok(m) if m.is_dir() => Evenement::Cree {
                    chemin,
                    repertoire: true,
                },
                Ok(_) => Evenement::Modified { chemin },
                Err(error) => {
                    // 🔴 **The root was recreated, and the file went away with
                    // it** (spec §6.4 case 3). Looping on retry would
                    // push indefinitely a file that no longer exists; removing
                    // it WHILE NAMING IT is all that remains — *knowing what
                    // we lost is not having it*.
                    tracing::warn!(
                        chemin, %error,
                        "ecriture due abandonnee : le fichier local n'existe plus"
                    );
                    let ligne = self.journal.retirer(&chemin);
                    self.write_journal(&ligne);
                    continue;
                }
            };
            if let Some(a_pousser) = self.file.signaler(evenement) {
                self.commencer(a_pousser);
            }
        }
        self.annoncer_les_dues();
    }
}
