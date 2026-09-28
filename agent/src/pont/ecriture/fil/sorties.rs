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
        .expect("un en-tete Dues se serialise toujours");
        self.emettre(proto::files::TYPE_DUES, CORRELATION_ANNONCE, &entete, &[]);
    }

    pub(super) fn emettre(&self, type_message: u8, correlation: u32, entete: &str, charge: &[u8]) {
        let trame = proto::files::encoder(type_message, correlation, entete, charge);
        if self
            .config
            .vers_navigateur
            .send(VersNavigateur::Requete { correlation, trame })
            .is_err()
        {
            tracing::warn!(
                correlation,
                "transport du pont parti : trame d'ecriture non emise"
            );
        }
    }

    pub(super) fn inscrire(&self, quoi: Attendue) -> u32 {
        let echeance = Instant::now() + WRITE_TIMEOUT;
        match self.config.table.lock() {
            Ok(mut table) => table.inscrire_sans_commande(quoi, echeance),
            Err(empoisonne) => empoisonne
                .into_inner()
                .inscrire_sans_commande(quoi, echeance),
        }
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
                "journal des ecritures dues non ecrit : une reprise apres arret brutal perdrait \
                 cette entree"
            );
            return;
        }
        disque::compacter_si_possible(&self.config.chemin_journal, &self.journal);
    }
}
