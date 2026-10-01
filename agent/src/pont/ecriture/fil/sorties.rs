//! What the write thread emits: frames to the browser, lines of the journal
//! of pending writes, and registrations in the shared table.
//!
//! Split out of `fil.rs` when `cargo fmt` pushed that file past 500 lines.

use super::*;

impl Fil {
    pub(super) fn annoncer_les_dues(&self) {
        let dues: Vec<entetes::Due> = self
            .journal
            .dues()
            .iter()
            .map(|(chemin, octets)| entetes::Due {
                chemin: chemin.clone(),
                octets: *octets,
            })
            .collect();
        let entete = serde_json::to_string(&entetes::Dues {
            dues,
            retenues: self.retenues,
        })
        .expect("a Dues header always serializes");
        // An announcement answers no command: it has no deadline.
        self.emettre(
            proto::files::TYPE_DUES,
            CORRELATION_ANNONCE,
            None,
            &entete,
            &[],
        );
    }

    pub(super) fn emettre(
        &self,
        type_message: u8,
        correlation: u32,
        echeance: Option<Instant>,
        entete: &str,
        charge: &[u8],
    ) {
        let trame = proto::files::encoder(type_message, correlation, entete, charge);
        if self
            .config
            .vers_navigateur
            .send(VersNavigateur::Requete {
                correlation,
                trame,
                echeance,
            })
            .is_err()
        {
            tracing::warn!(
                correlation,
                "bridge transport gone: write frame not emitted"
            );
        }
    }

    /// Returns the correlation AND its deadline: the transport must not emit a
    /// request whose command the table has already expired.
    pub(super) fn inscrire(&self, quoi: Attendue) -> (u32, Instant) {
        let echeance = Instant::now() + WRITE_TIMEOUT;
        let correlation = match self.config.table.lock() {
            Ok(mut table) => table.inscrire_sans_commande(quoi, echeance),
            Err(empoisonne) => empoisonne
                .into_inner()
                .inscrire_sans_commande(quoi, echeance),
        };
        (correlation, echeance)
    }

    /// Appends a line to the journal, and compacts it if possible.
    ///
    /// ⚠️ **A JOURNAL WRITE FAILURE DOES NOT PREVENT THE PUSH.** Not
    /// pushing would lose the data just as much, and **without even naming it**. The
    /// `warn!` is all we can do — and it says exactly what is
    /// lost: the ability to RESUME this entry after an abrupt stop.
    pub(super) fn write_journal(&mut self, ligne: &str) {
        if let Err(error) = disque::add(&self.config.chemin_journal, ligne) {
            tracing::warn!(
                %error, chemin = %self.config.chemin_journal.display(),
                "due-write journal not written: a resume after a hard stop would lose \
                 this entry"
            );
            return;
        }
        disque::compacter_si_possible(&self.config.chemin_journal, &self.journal);
    }
}
