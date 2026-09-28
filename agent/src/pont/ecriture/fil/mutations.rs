//! **The mutations pushed by the write thread**: renaming and deletion,
//! and their scheduling relative to due writes.
//!
//! # 🔴 WHY THIS EXTRACTION COMES ONE COMMIT TOO LATE, AND IT IS DECLARED
//!
//! `fil.rs` stood at **438** lines at the end of F2 — after F2 had itself
//! extracted `fil/disque.rs` to hold the gate. Wiring the mutations of
//! F3's task 13 took it to **612**: **the ceiling of 500 was CROSSED, AND
//! THE COMMIT WENT OUT WITH IT.**
//!
//! ⚠️ **It is WORSE than a crossing during work**, which this repository has
//! known several times: this one was committed, and it was **the report of a
//! NEIGHBOURING work item** that named it first — the fourth time in a row that
//! this happens in this repository (G2 three times, P2 three times, G3 once).
//!
//! **The lesson is not "extract", which was known: it is that the size sweep
//! must be done BY THE COMMAND, over the WHOLE tree, before each
//! commit — and not only where the plan budgets a margin.** §2.2 of
//! F3's plan did not even mention this file.
//!
//! **NEVER A COMPRESSION** — a gesture `CLAUDE.md` forbids by name, and
//! which D9 paid for twice before having to extract anyway.
//!
//! # The dividing line
//!
//! [`super`] carries **the content**: the journal of due writes, the queue, the
//! chunks, the resumption. This module carries **the mutations**, which carry
//! no byte, are registered in no journal, and **do not coalesce**.
//! The two are reviewed separately, and it is the only reason worth
//! splitting a file.

use std::time::Instant;

use proto::files::entetes;

use super::Fil;
use crate::pont::ecriture::Evenement;
use crate::pont::mutation::{ordonnancer, Mutation, Ordonnancement};
use crate::pont::table::Attendue;

impl Fil {
    /// What we do with a renaming or a deletion.
    ///
    /// ⚠️ **It goes NEITHER through the journal of due writes, NOR through the content
    /// queue**: it carries no byte, and registering it would make the shell page's
    /// counter rise for a gesture that has nothing to transfer.
    pub(super) fn mutation(&mut self, evenement: Evenement) {
        let quoi = match evenement {
            Evenement::Renomme {
                de,
                vers,
                repertoire,
            } => Mutation::Renommer {
                de,
                vers,
                repertoire,
            },
            Evenement::Deleted { chemin, repertoire } => Mutation::Delete { chemin, repertoire },
            // The calling arm guarantees `est_mutation()`; this case is
            // unreachable, and SAYING so beats assuming it.
            autre => {
                tracing::warn!(?autre, "non-mutation event routed to the mutation thread");
                return;
            }
        };
        if let Some(a_pousser) = self.mutations.signaler(quoi) {
            self.commencer_mutation(a_pousser);
        }
    }

    /// 🔴 **THE INTERLEAVING WITH DUE WRITES — the rule of §0.3, the one
    /// whose omission produces DATA LOSS.**
    ///
    /// The saving idiom of spec §3.5 — write a temporary file,
    /// rename, delete the old one — sends its three gestures IN A BURST, whereas
    /// F2 pushes writes **after the fact**. The decision lives in
    /// `pont::mutation`, which is PURE and tested on the host; this body only
    /// obeys.
    pub(super) fn commencer_mutation(&mut self, quoi: Mutation) {
        if !self.config.armee {
            // The DISARMED arm of `PONT_ECRITURE`: we log, we never
            // push. Nothing is due in the journal for a mutation, so nothing
            // remains — it is said rather than assumed.
            tracing::warn!(?quoi, "mutation NOT pushed: PONT_ECRITURE=0");
            if let Some(suivante) = self.mutations.terminee() {
                self.commencer_mutation(suivante);
            }
            return;
        }
        match ordonnancer(&self.file.chemins_dus(), &quoi) {
            Ordonnancement::Pousser => {}
            Ordonnancement::AttendreEcrituresDues { chemins } => {
                // 🔴 **THE MUTATION IS NOT PUSHED, and it goes back AHEAD** of
                // the ones that followed it: the order of the user's
                // gestures is the very meaning.
                tracing::warn!(
                    ?quoi, ?chemins,
                    "rename suspended: write due on the source. The mutation will resume                      when the bytes are pushed"
                );
                self.mutations.differer();
                return;
            }
            Ordonnancement::AbandonnerEcrituresDues { chemins } => {
                // 🔴 **PUSHING WOULD RECREATE WHAT THE USER ERASES.**
                for chemin in &chemins {
                    tracing::warn!(chemin, "due write abandoned: the path was deleted");
                    self.file.oublier(chemin);
                    let ligne = self.journal.retirer(chemin);
                    self.write_journal(&ligne);
                }
                self.annoncer_les_dues();
            }
        }
        let (type_message, entete, chemin, renommage, destination) = match &quoi {
            Mutation::Renommer {
                de,
                vers,
                repertoire,
            } => (
                proto::files::TYPE_RENOMMER,
                serde_json::to_string(&entetes::Renommer {
                    de: de.clone(),
                    vers: vers.clone(),
                    repertoire: *repertoire,
                })
                .expect("a Renommer header always serializes"),
                de.clone(),
                true,
                Some(vers.clone()),
            ),
            Mutation::Delete { chemin, repertoire } => (
                proto::files::TYPE_DELETE,
                serde_json::to_string(&entetes::Delete {
                    chemin: chemin.clone(),
                    repertoire: *repertoire,
                })
                .expect("a Remove header always serializes"),
                chemin.clone(),
                false,
                None,
            ),
        };
        let correlation = self.inscrire_mutation(Attendue::Muter {
            chemin,
            renommage,
            destination,
        });
        self.mutation_en_vol = Some(correlation);
        self.emettre(type_message, correlation, &entete, &[]);
        tracing::debug!(?quoi, correlation, "mutation poussee");
    }

    /// The mutation in flight is over. `acquittee` decides nothing in the journal —
    /// **a mutation is never registered there** — but the trace depends on it.
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
            Err(empoisonne) => empoisonne
                .into_inner()
                .inscrire_sans_commande(quoi, echeance),
        }
    }
}
