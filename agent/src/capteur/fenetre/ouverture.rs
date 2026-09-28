//! `Fenetre::ouvrir`: interprets the attach message and prepares the
//! `Fenetre`, **without building its source**.
//!
//! Extracted from `fenetre.rs` (review of task 8 of sub-block D10): the
//! tasks 8 and 9 had taken this file to 505 lines, above the cap
//! of 500 (`CLAUDE.md`). `ouvrir` has a clear responsibility — interpreting the
//! attach frame and building a sleeping `Fenetre`, without opening a
//! DXGI duplication nor building an encoder — and joins the pattern already
//! set by the other three child modules of this same file
//! (`commandes.rs`, `trace.rs`, `transitions.rs`), all born of the same
//! cap reason.
//!
//! Moved character for character, comments included: this module
//! adds and removes no behaviour, it only changes
//! address.

use std::time::Instant;

use anyhow::{bail, Context, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

use crate::capteur::horloge::{frequence_qpc, lire_qpc, origine_depuis_qpc};
use crate::capteur::protocole::VersCapteur;

use super::{Fenetre, Parametres};

impl Fenetre {
    /// Prepares the window announced by an attach frame — **without building
    /// its source**.
    ///
    /// **A window nobody watches consumes no encoder.**
    /// Until sub-block D5 this function called `sur_sortie`, hence opened
    /// a DXGI duplication and built a hardware encoder, *before* any
    /// registration in the pool: the 9th window failed at the hardware cap as
    /// if the pool did not exist, and the pool's accounting was wrong
    /// from birth — it believed the place free when it was already
    /// taken. The window is now born **asleep**, and it is the first
    /// `Ordre::Reveiller` that builds the source, once the place is acquired.
    ///
    /// ⚠️ **This MOVES failure detection, without losing it.** An invalid
    /// `hwnd` or an inaccessible output used to make the attach fail,
    /// and the child received an immediate `Refus`. Now the attach can
    /// only fail on resolving the SIZE; a source impossible
    /// to build only shows at the first wake-up, through a `warn!` and an
    /// `echec_de_reveil` that makes it be proposed again indefinitely. A window whose
    /// output vanished between attach and wake-up therefore loops on
    /// refused wake-ups instead of dying — that is the price of the prior
    /// acquisition, and it is accepted.
    ///
    /// The attach reply is NOT written here: it goes out on the
    /// command connection, which this thread never touches. The caller writes
    /// `Attachee { largeur, hauteur }` on success, `Refus` otherwise.
    pub fn ouvrir(attache: VersCapteur) -> Result<Fenetre> {
        // Renamed on destructuring: `Contexte.taille` — in the
        // PARENT module, `capteur/fenetre.rs`, and not "further down in this file"
        // as this sentence said before the extraction of the same sub-block
        // (the verbatim move kept the text and broke the
        // deictic) — designates the current RESOLVED size, unrelated to the
        // REQUESTED size the attach brings here. Both coexist in
        // this module; do not confuse them at first glance.
        let VersCapteur::Attache {
            session,
            hwnd,
            sortie,
            fps,
            debit,
            taille: taille_demandee,
            origine_qpc,
        } = attache
        else {
            bail!("le premier message d'un enfant doit être une attache");
        };

        let clock_origin = origine_depuis_qpc(
            origine_qpc,
            lire_qpc().context("lecture de QPC à l'attache")?,
            frequence_qpc().context("fréquence de QPC")?,
            Instant::now(),
        );

        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        // The PID does not travel on the protocol: it is derived from the `hwnd` the
        // child has already sent. The child does the same on its side, from its
        // `FENETRE_HWND`. Two independent derivations of the same stable
        // identifier are better than a protocol field to keep consistent.
        // Pre-initialised to 0, and it is THIS 0 that the `ensure` below
        // catches on failure — not a guarantee of the Windows API, which
        // documents no write of `lpdwProcessId` on failure.
        let mut pid = 0u32;
        // SAFETY: `hwnd` comes from a live child, and `&mut pid` is a
        // valid pointer to an initialised variable for the whole duration
        // of the call.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        anyhow::ensure!(
            pid != 0,
            "impossible de dériver le PID de la fenêtre {hwnd:?} de la session {session}"
        );
        // The size is the only thing needed before having the
        // place: `taille_de_sortie` reads it without opening a duplication, hence
        // without taking the output's mutex nor disturbing any neighbour.
        let sortie_taille = crate::capture::ouverture::taille_de_sortie(&sortie)
            .with_context(|| format!("attache de la session {session}"))?;
        // The output may be larger than the window (polluted registry,
        // D9 §9). The child announced the size the supervisor gave
        // it; the sensor bounds it to what the output actually offers,
        // with the SAME pure function as the supervisor — two deterministic
        // computations on the same inputs, never two rules.
        let (largeur, hauteur) =
            crate::superviseur::placement::taille_retenue(taille_demandee, sortie_taille);
        let parametres = Parametres {
            hwnd,
            sortie,
            fps,
            debit,
            clock_origin,
        };
        Ok(Fenetre {
            source: None,
            parametres,
            session,
            largeur,
            hauteur,
            pid,
        })
    }
}
