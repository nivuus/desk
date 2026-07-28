//! Observation du curseur Windows : décision de mode (absolu / relatif) et
//! forme à afficher.
//!
//! La décision de mode est stabilisée ici, hors de tout appel système, pour
//! être testable sous Linux — même raison que `geometry.rs` pour le mapping
//! de coordonnées.

/// Nombre d'observations consécutives cohérentes avant qu'un changement de
/// mode soit retenu.
///
/// Les transitions d'écran font clignoter le curseur : sans ce filtre, le
/// pointeur se verrouillerait et se déverrouillerait pendant les chargements.
/// À 50 ms par sondage, trois observations coûtent ~150 ms de latence de
/// bascule — imperceptible, puisqu'elle accompagne un changement de scène.
pub const SEUIL: u8 = 3;

/// Filtre de stabilité sur un état booléen observé périodiquement.
///
/// Rend `Some(nouvel_état)` au moment précis où un changement est retenu, et
/// `None` sinon — y compris pour toutes les observations qui suivent le
/// changement. L'appelant n'a donc rien à mémoriser : il émet un message
/// chaque fois qu'on lui rend `Some`.
pub struct Hysteresis {
    courant: bool,
    compte_contraire: u8,
}

impl Hysteresis {
    pub fn new(initial: bool) -> Self {
        Self { courant: initial, compte_contraire: 0 }
    }

    pub fn observer(&mut self, observe: bool) -> Option<bool> {
        if observe == self.courant {
            self.compte_contraire = 0;
            return None;
        }
        self.compte_contraire += 1;
        if self.compte_contraire < SEUIL {
            return None;
        }
        self.courant = observe;
        self.compte_contraire = 0;
        Some(observe)
    }

    /// État actuellement retenu. Utile au fil de sondage pour renseigner le
    /// drapeau partagé avec l'injecteur d'entrées.
    pub fn courant(&self) -> bool {
        self.courant
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

    /// Période de sondage. 20 Hz : assez pour que la bascule accompagne un
    /// changement de scène, assez peu pour être invisible au profileur.
    const PERIODE: Duration = Duration::from_millis(50);

    /// Table des curseurs système, chargée une fois. `LoadCursorW` sur un
    /// `IDC_*` rend un HANDLE PARTAGÉ, stable pour la durée du processus :
    /// comparer le handle courant à cette table identifie la forme sans
    /// inspecter le bitmap.
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
        // Un curseur applicatif custom ne correspond à aucun curseur système
        // et retombe sur `default` : c'est le prix assumé du choix « forme
        // seule », qui évite tout rendu d'overlay côté client.
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

    /// Lance le fil de sondage. Il émet un `AgentControl::Pointer` à chaque
    /// changement retenu — de visibilité comme de forme — et tient à jour le
    /// drapeau `mode_relatif` que lit l'injecteur d'entrées.
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
                        let forme_changee = derniere_forme != Some(forme);

                        if bascule.is_some() || forme_changee {
                            derniere_forme = Some(forme);
                            let visible_retenu = hysteresis.courant();
                            mode_relatif.store(!visible_retenu, Ordering::Relaxed);
                            // Le récepteur est tombé : la session est finie,
                            // ce fil n'a plus de raison d'être.
                            if tx.send(AgentControl::pointer(visible_retenu, forme)).is_err() {
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        // On reste en absolu : jamais de bascule à l'aveugle.
                        if !echec_signale {
                            echec_signale = true;
                            tracing::warn!(erreur = %e, "sondage du curseur indisponible (avertissement unique)");
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
    fn un_retour_a_l_etat_courant_remet_le_compteur_a_zero() {
        let mut h = Hysteresis::new(true);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(true), None); // le compteur repart de zéro
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), Some(false));
    }

    #[test]
    fn bascule_dans_les_deux_sens() {
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
