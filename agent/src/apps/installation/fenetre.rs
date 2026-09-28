//! The counting window: what the catalogue gained **during** an
//! installation, and not "at the reconciliation that follows it".
//!
//! 🔴 **THIS MODULE EXISTS BECAUSE THE SPECIFICATION'S SENTENCE, TAKEN
//! LITERALLY, PRODUCES A FALSE `sans_effet`.** It says: *an installation is
//! successful when the reconciliation that FOLLOWS it reports at least one
//! application that appeared.* But reconciliation runs every
//! [`crate::apps::PERIODE_RECONCILIATION`] — thirty seconds — **while** the
//! installer works. An installer that places its shortcut at the thirtieth
//! second then continues for three minutes will see its applications appear in
//! an **intermediate** reconciliation; the one following its exit will then
//! have nothing new to report, and the verdict would be `sans_effet` for a
//! perfectly successful installation.
//!
//! **Hence a window**: opened at launch, fed by each
//! reconciliation, closed after the process exits.
//!
//! ⚠️ **[`Fenetres::add`] ADDS TO ALL OPEN WINDOWS, and it is
//! deliberate.** Two concurrent installations are possible, and each must
//! count what appeared during ITS window — even if both
//! count the same application. **Attributing an appearance to a single
//! installation would be GUESSING**: nothing in a shortcut that has just
//! appeared says which installer placed it. Double counting yields at
//! worst two `reussie` where one of them did nothing; guessed
//! attribution would yield a **false** `sans_effet`, that is exactly the
//! defect this module exists to prevent.
//!
//! **Pure, no `cfg`, no clock, no input-output**: this module knows
//! neither what an application is nor when a reconciliation happens — it
//! receives a count and stores it.

use std::collections::BTreeMap;

/// The open counting windows, one per installation in flight.
///
/// The `BTreeMap` gives a stable iteration order for the same reason
/// `apps::reconciliation` chose it: two identical runs must
/// log the same thing, otherwise any acceptance run that compares two ticks
/// becomes undecidable.
#[derive(Debug, Default)]
pub struct Fenetres {
    ouvertes: BTreeMap<String, usize>,
}

impl Fenetres {
    pub fn nouvelles() -> Self {
        Self::default()
    }

    /// Opens the window of an installation, at zero.
    ///
    /// ⚠️ **REOPENING A LIVE IDENTIFIER RESETS THE COUNTER TO ZERO**, and it is
    /// not a gratuitous edge case: an identifier is unique per
    /// installation, so a second opening means either a replayed
    /// order or a reused identifier. In both cases, keeping the
    /// previous count would attribute to the installation that is starting
    /// appearances **earlier than its own launch** — that is, the
    /// guess the header of this module refuses. Starting from zero can
    /// only produce a `sans_effet`, which is an admission; the other choice would produce
    /// a `reussie`, which is an assertion.
    pub fn ouvrir(&mut self, id: &str) {
        self.ouvertes.insert(id.to_string(), 0);
    }

    /// Adds a reconciliation's count to **all** open windows.
    ///
    /// 🔴 IT IS AN ACCUMULATION, NEVER AN ASSIGNMENT. A
    /// reconciliation that finds nothing — the nominal case, tick after tick, on
    /// an idle disk — must **erase nothing** of what the previous ones
    /// saw, otherwise the window would only measure its last instant and
    /// would yield the false `sans_effet` it exists to prevent. It is the
    /// red of this module.
    pub fn add(&mut self, apparues: usize) {
        for total in self.ouvertes.values_mut() {
            *total = total.saturating_add(apparues);
        }
    }

    /// Closes the window and returns its total, or `None` if it was not
    /// open.
    ///
    /// 🔴 `None` IS NOT `Some(0)`, and confusing them would be the same fault
    /// as a `code_sortie` of `-1` used as a sentinel: "closed at zero" and
    /// "never opened" are **two different facts**. The first says that
    /// the installer ran without placing anything — that is [`super::verdict::Issue::SansEffet`];
    /// the second says that we lost track of the installation, and that is
    /// [`super::verdict::Issue::IssueInconnue`]. Returning `0` in both cases
    /// would declare "cancelled" an installation we know nothing about.
    pub fn fermer(&mut self, id: &str) -> Option<usize> {
        self.ouvertes.remove(id)
    }

    /// How many installations are in flight. The caller uses it to avoid
    /// forcing a reconciliation when there is nobody to serve.
    #[cfg(test)]
    pub fn en_vol(&self) -> usize {
        self.ouvertes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 THE SCENARIO DECISION D9 DESCRIBES, and the red of this module.
    /// An implementation that kept only the LAST addition would return
    /// `0` here, and would produce the false `sans_effet` for an installation that
    /// did indeed place three applications.
    #[test]
    fn une_reconciliation_vide_n_efface_pas_ce_que_la_precedente_a_compte() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ouvrir("inst-1");
        fenetres.add(3);
        fenetres.add(0);
        assert_eq!(fenetres.fermer("inst-1"), Some(3));
    }

    #[test]
    fn les_versements_s_accumulent_sur_toute_la_duree_de_la_fenetre() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ouvrir("inst-1");
        for apparues in [0, 2, 0, 0, 1, 0] {
            fenetres.add(apparues);
        }
        assert_eq!(fenetres.fermer("inst-1"), Some(3));
    }

    /// Two concurrent installations count the SAME appearance, and that is
    /// the intended behaviour: nothing says which one placed the shortcut.
    #[test]
    fn deux_fenetres_concurrentes_comptent_chacune_ce_qui_passe_pendant_la_sienne() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ouvrir("inst-1");
        fenetres.add(1);
        // The second one opens along the way: it must inherit NOTHING from the
        // addition made before its launch.
        fenetres.ouvrir("inst-2");
        fenetres.add(2);
        assert_eq!(fenetres.en_vol(), 2);
        assert_eq!(fenetres.fermer("inst-1"), Some(3));
        assert_eq!(fenetres.fermer("inst-2"), Some(2));
        assert_eq!(fenetres.en_vol(), 0);
    }

    /// A closed window stops counting — otherwise a finished installation
    /// would inflate the total of the next one.
    #[test]
    fn une_fenetre_fermee_ne_compte_plus() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ouvrir("inst-1");
        fenetres.ouvrir("inst-2");
        assert_eq!(fenetres.fermer("inst-1"), Some(0));
        fenetres.add(5);
        assert_eq!(fenetres.fermer("inst-2"), Some(5));
    }

    /// 🔴 "Never opened" and "closed at zero" are not confused.
    #[test]
    fn fermer_une_fenetre_inconnue_rend_none_et_non_zero() {
        let mut fenetres = Fenetres::nouvelles();
        assert_eq!(fenetres.fermer("jamais-ouverte"), None);
        fenetres.ouvrir("inst-1");
        assert_eq!(fenetres.fermer("inst-1"), Some(0));
        // And a second closing of the same identifier is also an
        // unknown window: the total is not read twice.
        assert_eq!(fenetres.fermer("inst-1"), None);
    }

    #[test]
    fn rouvrir_un_identifiant_vivant_repart_de_zero() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ouvrir("inst-1");
        fenetres.add(4);
        fenetres.ouvrir("inst-1");
        assert_eq!(fenetres.en_vol(), 1, "l'identifiant reste unique");
        assert_eq!(fenetres.fermer("inst-1"), Some(0));
    }

    /// Without an open window, `add` is inert: the reconciliation
    /// loop calls it on every tick, installation or not.
    #[test]
    fn adding_without_an_open_window_does_nothing() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.add(9);
        assert_eq!(fenetres.en_vol(), 0);
        fenetres.ouvrir("inst-1");
        assert_eq!(fenetres.fermer("inst-1"), Some(0));
    }
}
