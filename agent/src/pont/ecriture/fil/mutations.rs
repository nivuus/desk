//! **Les mutations poussées par le fil d'écriture** : renommage et suppression,
//! et leur ordonnancement par rapport aux écritures dues.
//!
//! # 🔴 POURQUOI CETTE EXTRACTION ARRIVE UN COMMIT TROP TARD, ET C'EST DÉCLARÉ
//!
//! `fil.rs` valait **438** lignes à la fin de F2 — après que F2 en eut lui-même
//! extrait `fil/disque.rs` pour tenir la porte. Le câblage des mutations de la
//! tâche 13 de F3 l'a porté à **612** : **le plafond de 500 a été FRANCHI, ET
//! LE COMMIT EST PARTI AVEC.**
//!
//! ⚠️ **C'est PIRE qu'un franchissement en cours de travail**, que ce dépôt a
//! connu plusieurs fois : celui-ci a été committé, et c'est **le relevé d'un
//! chantier VOISIN** qui l'a nommé en premier — la quatrième fois de suite que
//! cela arrive dans ce dépôt (G2 trois fois, P2 trois fois, G3 une).
//!
//! **La leçon n'est pas « extraire », qui était su : c'est que le balayage des
//! tailles doit se faire PAR LA COMMANDE, sur TOUT l'arbre, avant chaque
//! commit — et non seulement là où le plan budgète une marge.** Le §2.2 du plan
//! de F3 ne mentionnait même pas ce fichier.
//!
//! **JAMAIS UNE COMPRESSION** — geste que `CLAUDE.md` interdit nommément, et
//! que D9 a payé deux fois avant de devoir extraire quand même.
//!
//! # La ligne de partage
//!
//! [`super`] porte **le contenu** : le journal des écritures dues, la file, les
//! morceaux, la reprise. Ce module porte **les mutations**, qui ne transportent
//! aucun octet, ne s'inscrivent à aucun journal, et **ne se coalescent pas**.
//! Les deux se relisent séparément, et c'est la seule raison qui vaille de
//! scinder un fichier.

use std::time::Instant;

use proto::fichiers::entetes;

use super::Fil;
use crate::pont::ecriture::Evenement;
use crate::pont::mutation::{ordonnancer, Mutation, Ordonnancement};
use crate::pont::table::Attendue;

impl Fil {
    /// Ce qu'on fait d'un renommage ou d'une suppression.
    ///
    /// ⚠️ **Elle ne passe NI par le journal des écritures dues, NI par la file
    /// de contenu** : elle ne porte aucun octet, et l'inscrire ferait monter le
    /// compteur de la page-shell pour un geste qui n'a rien à transférer.
    pub(super) fn mutation(&mut self, evenement: Evenement) {
        let quoi = match evenement {
            Evenement::Renomme { de, vers, repertoire } => {
                Mutation::Renommer { de, vers, repertoire }
            }
            Evenement::Supprime { chemin, repertoire } => {
                Mutation::Supprimer { chemin, repertoire }
            }
            // Le bras appelant garantit `est_mutation()` ; ce cas est
            // inatteignable, et le DIRE vaut mieux que de le supposer.
            autre => {
                tracing::warn!(?autre, "evenement non-mutation route vers le fil de mutation");
                return;
            }
        };
        if let Some(a_pousser) = self.mutations.signaler(quoi) {
            self.commencer_mutation(a_pousser);
        }
    }

    /// 🔴 **L'ENTRELACEMENT AVEC LES ÉCRITURES DUES — la règle du §0.3, celle
    /// dont l'oubli produit une PERTE DE DONNÉES.**
    ///
    /// L'idiome d'enregistrement de la spec §3.5 — écrire un temporaire,
    /// renommer, supprimer l'ancien — envoie ses trois gestes EN RAFALE, alors
    /// que F2 pousse les écritures **après coup**. La décision vit dans
    /// `pont::mutation`, qui est PUR et testé sur l'hôte ; ce corps ne fait
    /// qu'obéir.
    pub(super) fn commencer_mutation(&mut self, quoi: Mutation) {
        if !self.config.armee {
            // Le bras DÉSARMÉ de `PONT_ECRITURE` : on journalise, on ne pousse
            // jamais. Rien n'est dû au journal pour une mutation, donc rien ne
            // reste — c'est dit plutôt que supposé.
            tracing::warn!(?quoi, "mutation NON poussee : PONT_ECRITURE=0");
            if let Some(suivante) = self.mutations.terminee() {
                self.commencer_mutation(suivante);
            }
            return;
        }
        match ordonnancer(&self.file.chemins_dus(), &quoi) {
            Ordonnancement::Pousser => {}
            Ordonnancement::AttendreEcrituresDues { chemins } => {
                // 🔴 **LA MUTATION N'EST PAS POUSSÉE, et elle repasse DEVANT**
                // celles qui l'ont suivie : l'ordre des gestes de
                // l'utilisateur est le sens même.
                tracing::warn!(
                    ?quoi, ?chemins,
                    "renommage suspendu : ecriture due sur la source. La mutation repartira                      quand les octets seront pousses"
                );
                self.mutations.differer();
                return;
            }
            Ordonnancement::AbandonnerEcrituresDues { chemins } => {
                // 🔴 **POUSSER RECRÉERAIT CE QUE L'UTILISATEUR EFFACE.**
                for chemin in &chemins {
                    tracing::warn!(
                        chemin,
                        "ecriture due abandonnee : le chemin a ete supprime"
                    );
                    self.file.oublier(chemin);
                    let ligne = self.journal.retirer(chemin);
                    self.ecrire_journal(&ligne);
                }
                self.annoncer_les_dues();
            }
        }
        let (type_message, entete, chemin, renommage) = match &quoi {
            Mutation::Renommer { de, vers, repertoire } => (
                proto::fichiers::TYPE_RENOMMER,
                serde_json::to_string(&entetes::Renommer {
                    de: de.clone(),
                    vers: vers.clone(),
                    repertoire: *repertoire,
                })
                .expect("un en-tete Renommer se serialise toujours"),
                de.clone(),
                true,
            ),
            Mutation::Supprimer { chemin, repertoire } => (
                proto::fichiers::TYPE_SUPPRIMER,
                serde_json::to_string(&entetes::Supprimer {
                    chemin: chemin.clone(),
                    repertoire: *repertoire,
                })
                .expect("un en-tete Supprimer se serialise toujours"),
                chemin.clone(),
                false,
            ),
        };
        let correlation = self.inscrire_mutation(Attendue::Muter { chemin, renommage });
        self.mutation_en_vol = Some(correlation);
        self.emettre(type_message, correlation, &entete, &[]);
        tracing::debug!(?quoi, correlation, "mutation poussee");
    }

    /// La mutation en vol est finie. `acquittee` ne décide de rien au journal —
    /// **une mutation n'y est jamais inscrite** — mais la trace en dépend.
    pub(super) fn terminer_mutation(&mut self, acquittee: bool) {
        self.mutation_en_vol = None;
        if let Some(suivante) = self.mutations.terminee() {
            self.commencer_mutation(suivante);
        }
        let _ = acquittee;
    }

    pub(super) fn inscrire_mutation(&self, quoi: Attendue) -> u32 {
        let echeance = Instant::now() + crate::pont::table::DELAI_MUTATION;
        match self.config.table.lock() {
            Ok(mut table) => table.inscrire_sans_commande(quoi, echeance),
            Err(empoisonne) => empoisonne.into_inner().inscrire_sans_commande(quoi, echeance),
        }
    }}
