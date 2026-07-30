//! Sondes d'entrée du chantier B : linéarité de la visée
//! (`INPUT_LINEARITY_PROBE`) et disponibilité de ViGEmBus (`VIGEM_PROBE`).

use anyhow::Result;

#[cfg(windows)]
use crate::{input, pointer_settings};

/// Point de départ et amplitude de la sonde de linéarité (tâche 8, chantier
/// B), extraits en fonction pure — sans le moindre appel Windows — pour
/// être testables sur Linux, comme `geometry.rs` ou `cursor::Hysteresis`.
///
/// `dx` et `dy` partagent toujours le signe de `pas` (la sonde ne déplace
/// le curseur que dans un seul quadrant), donc il ne faut de marge que DANS
/// LE SENS du déplacement, pas des deux côtés : partir à `marge` px du bord
/// de départ (haut-gauche si `pas` est positif ou nul, bas-droite sinon)
/// suffit, quelle que soit la résolution, tant que sa plus petite dimension
/// excède `2 * marge + amplitude.abs()`. En dessous, le clampage aux bords
/// de l'écran fausserait la mesure : refusé explicitement plutôt que de
/// laisser un écart silencieusement faux (constaté en pratique : un essai
/// parti du CENTRE d'un écran 2400×1080 avec une amplitude de 1000 px a
/// donné `obtenu_y = 539`, signature d'un curseur buté en bas d'écran).
///
/// Renvoie `(centre_x, centre_y, amplitude)`.
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
        "écran {largeur}x{hauteur} trop petit pour une amplitude de {amplitude} px \
         (pas={pas}, répétitions={repetitions}) : le clampage fausserait la mesure"
    );
    let coord = |dimension: i32| if amplitude >= 0 { marge } else { (dimension - 1 - marge).max(0) };
    Ok((coord(largeur), coord(hauteur), amplitude))
}

#[cfg(windows)]
pub(super) fn executer_vigem() -> Result<()> {
    let secondes: u64 = std::env::var("VIGEM_PROBE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    match crate::gamepad::probe(secondes) {
        Ok(rapport) => tracing::info!(rapport, "sonde ViGEmBus"),
        Err(e) => tracing::warn!(erreur = %e, "sonde ViGEmBus échouée"),
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
            Ok(rapport) => tracing::info!(rapport, "neutralisation appliquée"),
            Err(e) => tracing::warn!(erreur = %e, "neutralisation échouée"),
        }
    } else {
        tracing::warn!("neutralisation SAUTÉE (mesure de référence)");
    }

    // Écart au brief : le point fixe (960, 540) qu'il propose suppose un
    // écran 1920×1080, jamais vérifié, et son CENTRE s'est révélé
    // insuffisant à l'essai (voir `point_depart_lineaire`, testée sans
    // Windows). Remplacé par une lecture réelle de la résolution et un
    // point de départ biaisé dans le sens du déplacement.
    const MARGE: i32 = 20;
    let largeur = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let hauteur = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let (centre_x, centre_y, amplitude) =
        point_depart_lineaire(largeur, hauteur, pas, repetitions, MARGE)?;
    tracing::info!(largeur, hauteur, centre_x, centre_y, amplitude, "point de départ de la sonde");
    unsafe { SetCursorPos(centre_x, centre_y) }?;
    std::thread::sleep(std::time::Duration::from_millis(200));

    let mut avant = POINT::default();
    unsafe { GetCursorPos(&mut avant) }?;

    let injecteur_hwnd = windows::Win32::Foundation::HWND(std::ptr::null_mut());
    let mode = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let mut injecteur = input::InputInjector::new(injecteur_hwnd, mode);
    for _ in 0..repetitions {
        injecteur.inject(proto::input::InputMessage::MouseMoveRelative { dx: pas, dy: pas })?;
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    std::thread::sleep(std::time::Duration::from_millis(200));

    let mut apres = POINT::default();
    unsafe { GetCursorPos(&mut apres) }?;

    // Même formule que `point_depart_lineaire` : réutilisée telle
    // quelle plutôt que recalculée, pour ne pas risquer de la faire
    // diverger de celle qui a dimensionné le point de départ (revue 1).
    let attendu = amplitude;
    let obtenu_x = apres.x - avant.x;
    let obtenu_y = apres.y - avant.y;
    tracing::info!(
        attendu,
        obtenu_x,
        obtenu_y,
        ecart_x = obtenu_x - attendu,
        ecart_y = obtenu_y - attendu,
        "sonde de linéarité terminée"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::point_depart_lineaire;

    /// Résolution de la VM au moment des mesures (2400×1080) : le centre du
    /// bureau (1200, 540) ne laisse que 540 px de marge verticale, en dessous
    /// de l'amplitude de 1000 px que la tâche demande de mesurer — c'est
    /// exactement le clampage constaté en pratique avant la correction.
    const LARGEUR_VM: i32 = 2400;
    const HAUTEUR_VM: i32 = 1080;

    #[test]
    fn pas_positif_part_pres_du_bord_haut_gauche() {
        // pas=10, répétitions=100 : amplitude 1000, comme la mesure 2 du
        // brief.
        let (x, y, amplitude) =
            point_depart_lineaire(LARGEUR_VM, HAUTEUR_VM, 10, 100, 20).unwrap();
        assert_eq!(amplitude, 1000);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }

    #[test]
    fn pas_negatif_part_pres_du_bord_bas_droit() {
        // Jamais exercé par la sonde telle qu'appelée aujourd'hui (le brief
        // ne teste que des `pas` positifs), mais la formule doit rester
        // correcte pour ce cas : c'est justement ce que ce test vérifie.
        let (x, y, amplitude) =
            point_depart_lineaire(LARGEUR_VM, HAUTEUR_VM, -10, 100, 20).unwrap();
        assert_eq!(amplitude, -1000);
        assert_eq!(x, LARGEUR_VM - 1 - 20);
        assert_eq!(y, HAUTEUR_VM - 1 - 20);
    }

    #[test]
    fn grand_pas_faible_repetition_donne_la_meme_amplitude() {
        // pas=200, répétitions=5 : amplitude 1000, comme la mesure 3 du
        // brief — c'est là que l'accélération se verrait si la
        // neutralisation n'avait pas pris.
        let (x, y, amplitude) =
            point_depart_lineaire(LARGEUR_VM, HAUTEUR_VM, 200, 5, 20).unwrap();
        assert_eq!(amplitude, 1000);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }

    #[test]
    fn refuse_un_ecran_trop_petit_pour_l_amplitude_demandee() {
        // 100x100 ne laisse aucune place pour une amplitude de 1000 px :
        // la garde doit refuser plutôt que de laisser le clampage fausser
        // silencieusement la mesure.
        let erreur = point_depart_lineaire(100, 100, 200, 5, 20).unwrap_err();
        assert!(erreur.to_string().contains("trop petit"));
    }

    #[test]
    fn accepte_pile_a_la_limite_de_la_marge() {
        // largeur.min(hauteur) == 2*marge + amplitude exactement : la garde
        // compare avec `>=`, ce cas limite doit donc passer.
        let (x, y, amplitude) = point_depart_lineaire(1040, 2000, 10, 100, 20).unwrap();
        assert_eq!(amplitude, 1000);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }

    #[test]
    fn refuse_juste_sous_la_limite_de_la_marge() {
        let erreur = point_depart_lineaire(1039, 2000, 10, 100, 20).unwrap_err();
        assert!(erreur.to_string().contains("trop petit"));
    }

    #[test]
    fn amplitude_nulle_ne_demande_aucune_marge_particuliere() {
        let (x, y, amplitude) = point_depart_lineaire(50, 50, 0, 0, 20).unwrap();
        assert_eq!(amplitude, 0);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }
}
