//! Observing the Windows cursor: mode decision (absolute / relative) and
//! shape to display.
//!
//! The mode decision is stabilised here, outside any system call, to
//! be testable under Linux — same reason as `geometry.rs` for the
//! coordinate mapping.

/// Number of consistent consecutive observations before a mode change
/// is kept.
///
/// Screen transitions make the cursor blink: without this filter, the
/// pointer would lock and unlock during loading screens.
/// At 50 ms per poll, three observations cost ~150 ms of switching
/// latency — imperceptible, since it accompanies a scene change.
pub const SEUIL: u8 = 3;

/// Stability filter on a periodically observed boolean state.
///
/// Returns `Some(nouvel_état)` at the precise moment a change is kept, and
/// `None` otherwise — including for all observations following the
/// change. The caller therefore has nothing to memorise: it emits a message
/// every time it is given `Some`.
pub struct Hysteresis {
    current: bool,
    compte_contraire: u8,
}

impl Hysteresis {
    pub fn new(initial: bool) -> Self {
        Self {
            current: initial,
            compte_contraire: 0,
        }
    }

    pub fn observer(&mut self, observe: bool) -> Option<bool> {
        if observe == self.current {
            self.compte_contraire = 0;
            return None;
        }
        self.compte_contraire += 1;
        if self.compte_contraire < SEUIL {
            return None;
        }
        self.current = observe;
        self.compte_contraire = 0;
        Some(observe)
    }

    /// Currently kept state. Useful to the polling thread to fill in the
    /// flag shared with the input injector.
    pub fn current(&self) -> bool {
        self.current
    }
}

#[cfg(windows)]
mod win {
    use super::Hysteresis;
    use anyhow::{Context, Result};
    use proto::control::{AgentControl, CursorShape};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::Arc;
    use std::thread::JoinHandle;
    use std::time::Duration;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorInfo, LoadCursorW, CURSORINFO, CURSOR_SHOWING, HCURSOR, IDC_APPSTARTING,
        IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW,
        IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT,
    };

    /// Polling period. 20 Hz: enough for the switch to accompany a
    /// scene change, little enough to be invisible to the profiler.
    const PERIODE: Duration = Duration::from_millis(50);

    /// Table of system cursors, loaded once. `LoadCursorW` on an
    /// `IDC_*` returns a SHARED HANDLE, stable for the duration of the process:
    /// comparing the current handle with this table identifies the shape without
    /// inspecting the bitmap.
    fn table_des_formes() -> Vec<(isize, CursorShape)> {
        let paires = [
            (IDC_ARROW, CursorShape::Default),
            (IDC_IBEAM, CursorShape::Text),
            (IDC_WAIT, CursorShape::Wait),
            (IDC_APPSTARTING, CursorShape::Progress),
            (IDC_CROSS, CursorShape::Crosshair),
            (IDC_HAND, CursorShape::Pointer),
            (IDC_SIZEALL, CursorShape::Move),
            (IDC_NO, CursorShape::NotAllowed),
            (IDC_HELP, CursorShape::Help),
            (IDC_SIZENS, CursorShape::NsResize),
            (IDC_SIZEWE, CursorShape::EwResize),
            (IDC_SIZENWSE, CursorShape::NwseResize),
            (IDC_SIZENESW, CursorShape::NeswResize),
        ];
        paires
            .into_iter()
            .filter_map(|(id, forme)| {
                unsafe { LoadCursorW(None, id) }
                    .ok()
                    .map(|h| (h.0 as isize, forme))
            })
            .collect()
    }

    fn forme_de(handle: HCURSOR, table: &[(isize, CursorShape)]) -> CursorShape {
        // A custom application cursor matches no system cursor
        // and falls back on `default`: that is the accepted price of the "shape
        // only" choice, which avoids any overlay rendering on the client side.
        table
            .iter()
            .find(|(h, _)| *h == handle.0 as isize)
            .map(|(_, forme)| *forme)
            .unwrap_or(CursorShape::Default)
    }

    fn lire() -> Result<(bool, HCURSOR)> {
        let mut info = CURSORINFO {
            cbSize: std::mem::size_of::<CURSORINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetCursorInfo(&mut info) }.context("GetCursorInfo")?;
        Ok((info.flags.0 & CURSOR_SHOWING.0 != 0, info.hCursor))
    }

    /// Launches the polling thread. It emits an `AgentControl::Pointer` at each
    /// kept change — of visibility as of shape — and keeps up to date the
    /// `mode_relatif` flag the input injector reads.
    pub fn spawn_probe(
        tx: Sender<AgentControl>,
        mode_relatif: Arc<AtomicBool>,
        arret: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let table = table_des_formes();
            let mut hysteresis = Hysteresis::new(true);
            let mut derniere_forme: Option<CursorShape> = None;
            let mut echec_signale = false;

            while !arret.load(Ordering::Relaxed) {
                match lire() {
                    Ok((visible, handle)) => {
                        let forme = forme_de(handle, &table);
                        let bascule = hysteresis.observer(visible);
                        let visible_retenu = hysteresis.current();
                        let forme_changee = derniere_forme != Some(forme);

                        // Always memorise the observed shape, even if it
                        // causes no emission below: otherwise, a
                        // change happening while the cursor is
                        // hidden would never be announced on return to
                        // visible mode (the next `forme_changee` would compare it
                        // to an already obsolete shape).
                        derniere_forme = Some(forme);

                        // A shape change alone only justifies an emission
                        // if the cursor is kept visible: hidden, the
                        // client sets `cursor: 'none'` and never uses
                        // `shape` (see pointer.ts) — a shape message
                        // while a game is running would therefore be pure noise on a
                        // reliable channel, up to 20 messages per second.
                        if bascule.is_some() || (forme_changee && visible_retenu) {
                            mode_relatif.store(!visible_retenu, Ordering::Relaxed);
                            // The receiver has dropped: the session is over,
                            // this thread has no reason to exist any more.
                            if tx
                                .send(AgentControl::pointer(visible_retenu, forme))
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        // We stay absolute: never a blind switch.
                        if !echec_signale {
                            echec_signale = true;
                            tracing::warn!(error = %e, "sondage du curseur indisponible (avertissement unique)");
                        }
                    }
                }
                std::thread::sleep(PERIODE);
            }
        })
    }
}

#[cfg(windows)]
pub use win::spawn_probe;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_etat_stable_ne_produit_aucun_changement() {
        let mut h = Hysteresis::new(true);
        for _ in 0..10 {
            assert_eq!(h.observer(true), None);
        }
    }

    #[test]
    fn trois_observations_contraires_retiennent_le_changement() {
        let mut h = Hysteresis::new(true);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), Some(false));
    }

    #[test]
    fn le_changement_n_est_annonce_qu_une_seule_fois() {
        let mut h = Hysteresis::new(true);
        for _ in 0..SEUIL - 1 {
            assert_eq!(h.observer(false), None);
        }
        assert_eq!(h.observer(false), Some(false));
        assert_eq!(h.observer(false), None);
    }

    #[test]
    fn une_oscillation_ne_bascule_jamais() {
        let mut h = Hysteresis::new(true);
        for _ in 0..20 {
            assert_eq!(h.observer(false), None);
            assert_eq!(h.observer(true), None);
        }
    }

    #[test]
    fn a_return_to_the_current_state_resets_the_counter() {
        let mut h = Hysteresis::new(true);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(true), None); // the counter restarts from zero
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), Some(false));
    }

    #[test]
    fn toggles_both_ways() {
        let mut h = Hysteresis::new(true);
        for _ in 0..SEUIL - 1 {
            h.observer(false);
        }
        assert_eq!(h.observer(false), Some(false));
        for _ in 0..SEUIL - 1 {
            assert_eq!(h.observer(true), None);
        }
        assert_eq!(h.observer(true), Some(true));
    }
}
