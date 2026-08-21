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

use crate::pont::bonjour::{a_memoriser, decider, Decision};
use crate::pont::ecriture::Evenement;

use super::{disque, Fil};

impl Fil {
    /// **F5** — le navigateur s'est annoncé : on pousse, ou on RETIENT.
    ///
    /// ⚠️ **Le journal n'est JAMAIS vidé sur une retenue.** Rejouer aveuglément
    /// écrirait les fichiers d'une session dans le dossier d'une autre (spec
    /// §6.4 cas 2) ; jeter perdrait la donnée. **On ne fait ni l'un ni l'autre :
    /// on NOMME**, par `Dues.retenues`, et le navigateur propose « Reprendre
    /// l'enregistrement ».
    pub(super) fn bonjour(&mut self, racine: &str, forcer: bool) {
        let memorise = self.lire_le_nom_memorise();
        let decision = decider(memorise.as_deref(), racine, forcer);
        tracing::info!(
            racine,
            forcer,
            memorise = memorise.as_deref().unwrap_or("<aucun>"),
            decision = if decision == Decision::Pousser { "pousser" } else { "retenir" },
            dues = self.journal.dues().len(),
            "bonjour du navigateur : decision de reprise"
        );
        if let Some(nom) = a_memoriser(decision, racine) {
            self.ecrire_le_nom_memorise(nom);
        }
        match decision {
            Decision::Pousser => {
                self.retenues = false;
                self.reprendre();
            }
            Decision::Retenir => {
                // 🔴 **ON ANNONCE, ET ON NE POUSSE PAS.** Sans cette annonce, le
                // navigateur verrait un compteur de dues figé sans savoir
                // pourquoi — c'est-à-dire le silence que `retenues` existe pour
                // rompre.
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

    /// Le nom de racine mémorisé, **à côté du journal et jamais dedans**.
    ///
    /// 🔴 **HORS DE LA RACINE, et ce fichier-ci l'écrit déjà du journal** : un
    /// état qui vivrait DANS la racine serait lui-même un objet projeté — donc
    /// dépendant du pont pour être lu, donc circulaire — et il disparaîtrait
    /// avec elle exactement le jour où il sert.
    fn chemin_du_nom(&self) -> std::path::PathBuf {
        self.config.chemin_journal.with_file_name("racine.nom")
    }

    fn lire_le_nom_memorise(&self) -> Option<String> {
        let brut = std::fs::read_to_string(self.chemin_du_nom()).ok()?;
        // ⚠️ **Rogné, parce qu'un éditeur ajoute une fin de ligne.** Un nom qui
        // ne différerait que par un `\n` ferait RETENIR à chaque démarrage, sur
        // le bon répertoire, sans que rien n'en dise la cause.
        let nom = brut.trim_end_matches(['\r', '\n']).to_string();
        // Un fichier vide n'est pas un nom vide : il n'y a rien de mémorisé.
        // Les confondre ferait retenir sur toute racine au premier montage.
        if nom.is_empty() && brut.is_empty() {
            return None;
        }
        Some(nom)
    }

    fn ecrire_le_nom_memorise(&self, nom: &str) {
        if let Err(erreur) = std::fs::write(self.chemin_du_nom(), nom) {
            // Un `warn!` et rien d'autre : ne pas mémoriser fait RETENIR au
            // démarrage suivant, ce qui est le côté sûr.
            tracing::warn!(%erreur, nom, "nom de racine non memorise");
        }
    }

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
