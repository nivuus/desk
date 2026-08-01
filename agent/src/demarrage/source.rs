//! Choix et construction de la source vidéo Windows d'une session.
//!
//! Extrait de `demarrage.rs` au sous-bloc D1 : ce chemin y a gagné deux
//! embranchements (fenêtre imposée ou cherchée par titre, sortie DXGI entière
//! ou recadrage de fenêtre) qui portaient le fichier parent au-dessus du
//! plafond de 500 lignes du projet. L'addition s'accompagne donc de son
//! extraction, comme la règle l'exige.

#![cfg(windows)]

use anyhow::Result;
use windows::Win32::Foundation::HWND;

use crate::source::VideoSource;
use crate::{window, windows_source};
use crate::Config;

/// Ce que `demarrage` a besoin de savoir de la source construite.
///
/// `hwnd_addr` est une adresse brute (`isize`, qui est `Send`) et non un
/// `HWND` : `HWND` enveloppe un `*mut c_void` non `Send` en windows-rs 0.62,
/// et cette valeur traverse la fermeture `move` de `spawn_blocking` côté
/// appelant. Un `HWND` n'est qu'un identifiant opaque, jamais déréférencé.
pub(super) struct SourceWindows {
    pub source: Box<dyn VideoSource + Send>,
    pub hwnd_addr: isize,
    pub bitrate: u32,
}

pub(super) fn construire(
    config: &Config,
    clock_origin: std::time::Instant,
) -> Result<SourceWindows> {
    // Fenêtre imposée par le superviseur, ou recherche par titre pour un agent
    // lancé à la main.
    let hwnd = match config.fenetre_hwnd {
        Some(brut) => {
            let hwnd = HWND(brut as *mut core::ffi::c_void);
            anyhow::ensure!(
                window::is_window_alive(hwnd),
                "la fenêtre {brut:#x} imposée par le superviseur n'existe plus"
            );
            hwnd
        }
        None => {
            let title = std::env::var("WINDOW_TITLE").unwrap_or_else(|_| "firefox".into());
            window::find_window_by_title(&title)?
        }
    };

    let bitrate: u32 = std::env::var("BITRATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12_000_000);

    // 90 et non 60 : cette valeur n'est pas une cadence cible, c'est le
    // `MF_MT_FRAME_RATE` annoncé aux deux MFT — et le Video Processor s'en sert
    // comme cadence de SORTIE, qu'il tient en rejouant la dernière image
    // convertie quand rien de neuf ne lui est arrivé. Annoncer 60 plafonnait
    // donc tout le pipeline à 60 sorties/s pour un bureau qui en produit 68,5,
    // d'où 47,5 images/s encodées et ~44 i/s au navigateur.
    //
    // Mesuré (bureau à 68,5 Hz) : 60 → 44,3 i/s · 75 → 53,1 · 90 → 58,5 ·
    // 120 → 63,0. Au-delà de 90, le gain est du rejeu : à 120, les images
    // NEUVES converties retombent de 62 à 54/s parce que le convertisseur,
    // occupé à tenir sa cadence déclarée, refuse davantage d'entrées.
    let fps: u32 = std::env::var("ENCODER_FPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);

    let source: Box<dyn VideoSource + Send> = match &config.sortie_dxgi {
        Some(nom_sortie) => {
            tracing::info!(
                nom_sortie = %nom_sortie,
                bitrate,
                fps,
                "capture d'une sortie DXGI entière (mode multi-fenêtres)"
            );
            Box::new(windows_source::WindowsSource::sur_sortie(
                hwnd,
                nom_sortie,
                fps,
                bitrate,
                clock_origin,
            )?)
        }
        None => {
            tracing::info!(bitrate, fps, "capture de la fenêtre Windows (recadrage)");
            Box::new(windows_source::WindowsSource::new(
                hwnd,
                fps,
                bitrate,
                clock_origin,
            )?)
        }
    };

    Ok(SourceWindows { source, hwnd_addr: hwnd.0 as isize, bitrate })
}
