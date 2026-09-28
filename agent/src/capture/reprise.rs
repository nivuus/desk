//! Classifying a DXGI acquisition failure, and bounding the resumptions.
//!
//! **Pure on purpose.** `capture.rs` is `#![cfg(windows)]` as a whole:
//! a CHILD module there would not be compilable on the Linux host, hence not
//! testable. This file is therefore declared as a SIBLING module in `main.rs`
//! (`#[path = "capture/reprise.rs"] mod capture_reprise;`), outside any
//! `cfg` — the same set-up as `windows_source/sortie.rs`, and for the same
//! reason.
//!
//! It only knows `i32`s: the bare DXGI codes. No `windows-rs` type
//! crosses this boundary, otherwise it would not hold.

/// `DXGI_ERROR_ACCESS_LOST`. DXGI revokes access to a duplication when the
/// display topology changes — and **creating a virtual output is
/// one such case**, recorded by sub-block D1 over three runs out of three.
/// The Desktop Duplication documentation describes this state as recoverable:
/// release the `IDXGIOutputDuplication` and create a new one.
pub const ACCES_PERDU: i32 = 0x887A0026u32 as i32;

/// `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`. Returned by `DuplicateOutput` when the
/// output cannot be duplicated **at this instant**. Two very
/// different causes show up under this same code, and the code does not distinguish
/// them: a topology reconfiguration in progress — transient —, and a cap
/// on concurrent duplications — lasting. That is why the retry window
/// is short and its abandonment loud.
pub const NON_DISPONIBLE: i32 = 0x887A0022u32 as i32;

/// `DXGI_ERROR_DEVICE_REMOVED`. The device itself is lost: reopening
/// the duplication alone would be useless. Remains definitive.
#[cfg(test)]
pub const DEVICE_REMOVED: i32 = 0x887A0005u32 as i32;

/// `DXGI_ERROR_WAIT_TIMEOUT`. Not a failure: the desktop simply has not
/// changed. Handled upstream of the classification, but named here so that the
/// test can check it is NOT taken for an access loss.
#[cfg(test)]
pub const ATTENTE_EXPIREE: i32 = 0x887A0027u32 as i32;

/// Duration during which an access loss is retried before being declared
/// definitive.
///
/// **An upper bound, not calibrated, and it must be said.** The measurement of
/// 1 August 2026 (`plans/journaux-multifenetres-d2/`) establishes two points and
/// two only: three attempts chained without delay, that is 14 to 21 ms,
/// **are not enough**; and a reopening attempted 3 s after the reshuffle
/// **succeeds**, on 7 probes out of 7. The real threshold is somewhere between the
/// two and was not searched for. Eight seconds cover it amply.
///
/// What bounds the cost of a value too large: the window blocks nothing
/// (see `Tentative::Patienter`), it only delays the admission of failure
/// of a source that would not return a frame anyway.
pub const DUREE_FENETRE_REPRISE: std::time::Duration = std::time::Duration::from_secs(8);

/// Minimum interval between two reopening attempts.
///
/// Small compared to the window, so as not to delay the real resumption; large
/// enough for the `info!` trace of each attempt to stay rare — at most
/// ~7 attempts per second and per source (1000 ms / 150 ms), that is up to
/// ~13 lines per second when every reopening fails (one attempt
/// line, one failure line), against one per `next_frame` call
/// (~90/s) if the step did not exist. The repository has already paid twice for a
/// trace emitted at the cadence of the capture loop.
pub const PAS_REPRISE: std::time::Duration = std::time::Duration::from_millis(150);

/// Duration during which opening a duplication is retried.
///
/// **Shorter than `DUREE_FENETRE_REPRISE`**, and for a diagnostic
/// reason: when the cause is a concurrency cap, waiting
/// longer does not change the result and delays the reading. Three seconds
/// cover the topology reconfiguration the repository admits elsewhere
/// (`DELAI_TOPOLOGIE`).
///
/// **An upper bound, not calibrated**, like `DUREE_FENETRE_REPRISE`.
pub const DUREE_FENETRE_OUVERTURE: std::time::Duration = std::time::Duration::from_secs(3);

pub fn est_acces_perdu(code: i32) -> bool {
    code == ACCES_PERDU
}

/// True if a duplication opening failure deserves to be retried.
pub fn est_ouverture_retentable(code: i32) -> bool {
    code == NON_DISPONIBLE || code == ACCES_PERDU
}

/// What the window asks the caller to do, now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tentative {
    /// Retry the reopening right away.
    Rouvrir,
    /// Do nothing this round. The caller returns "nothing new" — **without
    /// sleeping**: that is what distinguishes this form from a blocking resumption
    /// loop, and what lets the session loop keep running.
    Patienter,
    /// The window is closed: the access loss is definitive.
    Expiree,
}

/// Resumption window, opened at the first access loss and closed by the
/// first success.
///
/// **It reads no clock**: the instant is passed to it. That is what
/// makes it testable on the Linux host, where all the rest of this path is invisible.
///
/// **It replaced a budget in number of attempts**, which measurement
/// refuted: creating a virtual output returns `ACCESS_LOST`, the reopening
/// succeeds, and the reopened duplication **immediately** returns `ACCESS_LOST` in
/// turn as long as Windows has not finished reconfiguring its topology. Three
/// attempts without delay were therefore burnt before the phenomenon
/// ended. The attempt count did not measure the right quantity.
pub struct FenetreDeReprise {
    ouverte_a: Option<std::time::Instant>,
    derniere_tentative: Option<std::time::Instant>,
    tentatives: u32,
}

impl FenetreDeReprise {
    pub fn new() -> Self {
        Self {
            ouverte_a: None,
            derniere_tentative: None,
            tentatives: 0,
        }
    }

    pub fn tenter(&mut self, maintenant: std::time::Instant) -> Tentative {
        let ouverte_a = *self.ouverte_a.get_or_insert(maintenant);
        if maintenant.duration_since(ouverte_a) > DUREE_FENETRE_REPRISE {
            return Tentative::Expiree;
        }
        if let Some(derniere) = self.derniere_tentative {
            if maintenant.duration_since(derniere) < PAS_REPRISE {
                return Tentative::Patienter;
            }
        }
        self.derniere_tentative = Some(maintenant);
        self.tentatives += 1;
        Tentative::Rouvrir
    }

    /// Closes the window. Called on any acquisition success, `Ok(None)`
    /// included: as soon as DXGI stops refusing, the reshuffle is over.
    pub fn succes(&mut self) {
        self.ouverte_a = None;
        self.derniere_tentative = None;
        self.tentatives = 0;
    }

    pub fn tentatives(&self) -> u32 {
        self.tentatives
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seule_la_perte_d_acces_est_recuperable() {
        assert!(est_acces_perdu(ACCES_PERDU));
        assert!(
            !est_acces_perdu(DEVICE_REMOVED),
            "périphérique perdu : rouvrir ne sert à rien"
        );
        assert!(
            !est_acces_perdu(ATTENTE_EXPIREE),
            "attente expirée n'est même pas un échec"
        );
        assert!(!est_acces_perdu(0), "S_OK");
        assert!(!est_acces_perdu(0x80070057u32 as i32), "E_INVALIDARG");
    }

    /// `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE` says in its own label that the
    /// resource "may be later". That is what a
    /// `DuplicateOutput` attempted while Windows reconfigures its topology returns —
    /// the nominal case when another window opens at the same instant.
    #[test]
    fn une_ouverture_est_retentable_sur_indisponibilite_ou_perte_d_acces() {
        assert!(est_ouverture_retentable(NON_DISPONIBLE));
        assert!(est_ouverture_retentable(ACCES_PERDU));
    }

    /// A lost device will not come back, and an invalid argument is
    /// not a matter of patience: retrying them would only delay the
    /// diagnosis by three seconds.
    #[test]
    fn une_ouverture_n_est_pas_retentable_sur_une_panne_franche() {
        assert!(!est_ouverture_retentable(DEVICE_REMOVED));
        assert!(
            !est_ouverture_retentable(0x80070057u32 as i32),
            "E_INVALIDARG"
        );
        assert!(!est_ouverture_retentable(0), "S_OK");
    }

    /// The opening window is SHORTER than the capture one, and it is
    /// deliberate: a lasting failure at opening must show quickly, the real
    /// cause possibly being a concurrency cap no patience
    /// gets past.
    #[test]
    fn la_fenetre_d_ouverture_est_plus_courte_que_celle_de_la_capture() {
        assert!(DUREE_FENETRE_OUVERTURE < DUREE_FENETRE_REPRISE);
        assert!(DUREE_FENETRE_OUVERTURE >= std::time::Duration::from_secs(2));
    }

    /// A base of instants that does not read the system clock: the type under
    /// test reads none, that is the whole point.
    fn t(base: std::time::Instant, ms: u64) -> std::time::Instant {
        base + std::time::Duration::from_millis(ms)
    }

    #[test]
    fn la_premiere_perte_fait_rouvrir_tout_de_suite() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::new();
        assert_eq!(fenetre.tenter(t(base, 0)), Tentative::Rouvrir);
        assert_eq!(fenetre.tentatives(), 1);
    }

    /// The defect the measurement found: three attempts without delay were
    /// burnt in 14 to 21 ms, while the topology takes up to 3 s to
    /// stabilise. The wait step is what prevents that.
    #[test]
    fn une_seconde_tentative_trop_proche_fait_patienter() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::new();
        fenetre.tenter(t(base, 0));
        assert_eq!(fenetre.tenter(t(base, 5)), Tentative::Patienter);
        assert_eq!(fenetre.tenter(t(base, 20)), Tentative::Patienter);
        assert_eq!(fenetre.tentatives(), 1, "patienter n'est pas une tentative");
    }

    #[test]
    fn the_elapsed_step_reopens_again() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::new();
        fenetre.tenter(t(base, 0));
        let apres_le_pas = PAS_REPRISE.as_millis() as u64;
        assert_eq!(fenetre.tenter(t(base, apres_le_pas)), Tentative::Rouvrir);
        assert_eq!(fenetre.tentatives(), 2);
    }

    /// The window must amply cover the 3 s this repository already admits for
    /// a topology to stabilise (`DELAI_TOPOLOGIE`).
    #[test]
    fn la_fenetre_couvre_largement_la_stabilisation_de_la_topologie() {
        assert!(
            DUREE_FENETRE_REPRISE >= std::time::Duration::from_secs(6),
            "la sonde post-mortem a réussi à 3 s ; une fenêtre qui ne les \
             couvrirait pas au double reproduirait le défaut mesuré"
        );
        assert!(
            PAS_REPRISE < DUREE_FENETRE_REPRISE / 10,
            "un pas trop grand devant la fenêtre retarderait la reprise réelle"
        );
    }

    #[test]
    fn la_fenetre_expire_au_bout_de_sa_duree() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::new();
        fenetre.tenter(t(base, 0));
        let apres = DUREE_FENETRE_REPRISE.as_millis() as u64 + 1;
        assert_eq!(fenetre.tenter(t(base, apres)), Tentative::Expiree);
    }

    /// Expiry is counted from the window's OPENING, not from the
    /// last attempt: otherwise a resumption that fails indefinitely would
    /// never end.
    #[test]
    fn l_expiration_se_compte_depuis_l_ouverture_et_non_depuis_la_derniere_tentative() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::new();
        let pas = PAS_REPRISE.as_millis() as u64;
        let mut instant = 0;
        while instant < DUREE_FENETRE_REPRISE.as_millis() as u64 {
            fenetre.tenter(t(base, instant));
            instant += pas;
        }
        assert_eq!(fenetre.tenter(t(base, instant)), Tentative::Expiree);
    }

    /// The window closes on an acquisition success, `Ok(None)` included —
    /// that is as soon as DXGI stops refusing, even without a new frame. An
    /// idle desktop produces no frame for long periods, and
    /// a window that only closed on a delivered frame
    /// would turn rare and unrelated losses into wear.
    #[test]
    fn un_succes_referme_la_fenetre_qui_rouvre_alors_pleine() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::new();
        fenetre.tenter(t(base, 0));
        fenetre.succes();
        assert_eq!(fenetre.tentatives(), 0);

        let tard = DUREE_FENETRE_REPRISE.as_millis() as u64 * 3;
        assert_eq!(
            fenetre.tenter(t(base, tard)),
            Tentative::Rouvrir,
            "une perte d'accès bien plus tard ouvre une fenêtre NEUVE"
        );
        assert_eq!(
            fenetre.tenter(t(base, tard + DUREE_FENETRE_REPRISE.as_millis() as u64 + 1)),
            Tentative::Expiree,
            "et cette fenêtre neuve court depuis SA propre ouverture"
        );
    }
}
