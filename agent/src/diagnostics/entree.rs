//! Input probes of work stream B: aiming linearity
//! (`INPUT_LINEARITY_PROBE`) and ViGEmBus availability (`VIGEM_PROBE`).

use anyhow::Result;

#[cfg(windows)]
use crate::{input, pointer_settings};

/// Starting point and amplitude of the linearity probe (task 8, work stream
/// B), extracted into a pure function — without a single Windows call — to
/// be testable on Linux, like `geometry.rs` or `cursor::Hysteresis`.
///
/// `dx` and `dy` always share the sign of `pas` (the probe only moves
/// the cursor within a single quadrant), so a margin is only needed IN
/// THE DIRECTION of the movement, not on both sides: starting at `marge` px from the
/// starting edge (top-left if `pas` is positive or zero, bottom-right otherwise)
/// is enough, whatever the resolution, as long as its smallest dimension
/// exceeds `2 * marge + amplitude.abs()`. Below that, clamping to the edges
/// of the screen would skew the measurement: refused explicitly rather than
/// leaving a silently wrong gap (observed in practice: a run
/// started from the CENTRE of a 2400×1080 screen with an amplitude of 1000 px
/// gave `obtenu_y = 539`, the signature of a cursor stuck at the bottom of the screen).
///
/// Returns `(centre_x, centre_y, amplitude)`.
fn point_depart_lineaire(
    largeur: i32,
    hauteur: i32,
    pas: i16,
    repetitions: i32,
    marge: i32,
) -> Result<(i32, i32, i32)> {
    let amplitude = pas as i32 * repetitions;
    anyhow::ensure!(
        largeur.min(hauteur) >= 2 * marge + amplitude.abs(),
        "screen {largeur}x{hauteur} too small for an amplitude of {amplitude} px \
         (step={pas}, repetitions={repetitions}): the clamping would skew the measurement"
    );
    let coord = |dimension: i32| {
        if amplitude >= 0 {
            marge
        } else {
            (dimension - 1 - marge).max(0)
        }
    };
    Ok((coord(largeur), coord(hauteur), amplitude))
}

#[cfg(windows)]
pub(super) fn executer_vigem() -> Result<()> {
    let secondes: u64 = std::env::var("VIGEM_PROBE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    match crate::gamepad::probe(secondes) {
        Ok(rapport) => tracing::info!(rapport, "ViGEmBus probe"),
        Err(e) => tracing::warn!(error = %e, "ViGEmBus probe failed"),
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn executer_linearite() -> Result<()> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetSystemMetrics, SetCursorPos, SM_CXSCREEN, SM_CYSCREEN,
    };

    let pas: i16 = std::env::var("INPUT_LINEARITY_PAS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let repetitions: i32 = std::env::var("INPUT_LINEARITY_REPETITIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    if std::env::var("INPUT_LINEARITY_NEUTRALISER").as_deref() != Ok("0") {
        match pointer_settings::neutraliser() {
            Ok(rapport) => tracing::info!(rapport, "neutralisation applied"),
            Err(e) => tracing::warn!(error = %e, "neutralisation failed"),
        }
    } else {
        tracing::warn!("neutralisation SKIPPED (reference measurement)");
    }

    // Gap from the brief: the fixed point (960, 540) it proposes assumes a
    // 1920×1080 screen, never checked, and its CENTRE proved
    // insufficient in the run (see `point_depart_lineaire`, tested without
    // Windows). Replaced by a real reading of the resolution and a
    // starting point biased in the direction of the movement.
    const MARGE: i32 = 20;
    let largeur = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let hauteur = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let (centre_x, centre_y, amplitude) =
        point_depart_lineaire(largeur, hauteur, pas, repetitions, MARGE)?;
    tracing::info!(
        largeur,
        hauteur,
        centre_x,
        centre_y,
        amplitude,
        "starting point of the probe"
    );
    unsafe { SetCursorPos(centre_x, centre_y) }?;
    std::thread::sleep(std::time::Duration::from_millis(200));

    let mut before = POINT::default();
    unsafe { GetCursorPos(&mut before) }?;

    let injecteur_hwnd = windows::Win32::Foundation::HWND(std::ptr::null_mut());
    let mode = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    // `None`: this bench probe targets a WINDOW, never a whole
    // output — the input reference is therefore the original one.
    let mut injecteur = input::InputInjector::new(
        injecteur_hwnd,
        // Single-window probe: the capture crops the window, the client
        // area IS the frame.
        crate::entrees::Reference::ZoneClientDeLaFenetre,
        mode,
    );
    for _ in 0..repetitions {
        injecteur.inject(proto::input::InputMessage::MouseMoveRelative { dx: pas, dy: pas })?;
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    std::thread::sleep(std::time::Duration::from_millis(200));

    let mut apres = POINT::default();
    unsafe { GetCursorPos(&mut apres) }?;

    // Same formula as `point_depart_lineaire`: reused as
    // is rather than recomputed, so as not to risk making it
    // diverge from the one that sized the starting point (review 1).
    let attendu = amplitude;
    let obtenu_x = apres.x - before.x;
    let obtenu_y = apres.y - before.y;
    tracing::info!(
        attendu,
        obtenu_x,
        obtenu_y,
        ecart_x = obtenu_x - attendu,
        ecart_y = obtenu_y - attendu,
        "linearity probe finished"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::point_depart_lineaire;

    /// Resolution of the VM at the time of the measurements (2400×1080): the centre of the
    /// desktop (1200, 540) only leaves 540 px of vertical margin, below
    /// the 1000 px amplitude the task asks to measure — it is
    /// exactly the clamping observed in practice before the fix.
    const LARGEUR_VM: i32 = 2400;
    const HAUTEUR_VM: i32 = 1080;

    #[test]
    fn pas_positif_part_pres_du_bord_haut_gauche() {
        // step=10, repetitions=100: amplitude 1000, like measurement 2 of the
        // brief.
        let (x, y, amplitude) = point_depart_lineaire(LARGEUR_VM, HAUTEUR_VM, 10, 100, 20).unwrap();
        assert_eq!(amplitude, 1000);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }

    #[test]
    fn pas_negatif_part_pres_du_bord_bas_droit() {
        // Never exercised by the probe as called today (the brief
        // only tests positive `pas`), but the formula must stay
        // correct for this case: that is precisely what this test checks.
        let (x, y, amplitude) =
            point_depart_lineaire(LARGEUR_VM, HAUTEUR_VM, -10, 100, 20).unwrap();
        assert_eq!(amplitude, -1000);
        assert_eq!(x, LARGEUR_VM - 1 - 20);
        assert_eq!(y, HAUTEUR_VM - 1 - 20);
    }

    #[test]
    fn grand_pas_faible_repetition_donne_la_meme_amplitude() {
        // step=200, repetitions=5: amplitude 1000, like measurement 3 of the
        // brief — that is where acceleration would show if the
        // neutralisation had not taken.
        let (x, y, amplitude) = point_depart_lineaire(LARGEUR_VM, HAUTEUR_VM, 200, 5, 20).unwrap();
        assert_eq!(amplitude, 1000);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }

    #[test]
    fn refuse_un_ecran_trop_petit_pour_l_amplitude_demandee() {
        // 100x100 leaves no room for a 1000 px amplitude:
        // the guard must refuse rather than let clamping silently
        // skew the measurement.
        let error = point_depart_lineaire(100, 100, 200, 5, 20).unwrap_err();
        assert!(error.to_string().contains("too small"));
    }

    #[test]
    fn accepte_pile_a_la_limite_de_la_marge() {
        // largeur.min(hauteur) == 2*marge + amplitude exactly: the guard
        // compares with `>=`, so this edge case must pass.
        let (x, y, amplitude) = point_depart_lineaire(1040, 2000, 10, 100, 20).unwrap();
        assert_eq!(amplitude, 1000);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }

    #[test]
    fn refuses_just_below_the_margin_limit() {
        let error = point_depart_lineaire(1039, 2000, 10, 100, 20).unwrap_err();
        assert!(error.to_string().contains("too small"));
    }

    #[test]
    fn zero_amplitude_needs_no_particular_margin() {
        let (x, y, amplitude) = point_depart_lineaire(50, 50, 0, 0, 20).unwrap();
        assert_eq!(amplitude, 0);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }
}
