//! Énumération des sorties DXGI, indépendamment de toute duplication ouverte.
//!
//! Extrait de `capture.rs` à la tâche 2 du sous-bloc D2 : l'ajout de
//! `CibleCapture` et `DesktopCapture::rouvrir` portait le fichier parent à
//! 535 lignes, au-dessus du plafond de 500 (`CLAUDE.md`). Ce module ne
//! touche à aucun champ privé de `DesktopCapture` — il n'avait donc pas
//! besoin d'être un module enfant pour l'accès, seulement pour rester
//! `capture::enumerer_sorties` aux yeux des appelants existants, via le
//! réexport en tête de `capture.rs`.

use anyhow::{Context, Result};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

use crate::sortie_dxgi::SortieDxgi;

/// Énumère toutes les sorties de tous les adaptateurs, **en journalisant** les
/// adaptateurs dépourvus de sortie.
///
/// Sert au relevé du temps 1 de la sonde multi-fenêtres : deux documents du
/// dépôt se contredisent sur l'adaptateur qui pilote réellement le bureau
/// (`plans/fix-debit-socket-report.md:163` contre le commit `4493b24`), et
/// c'est ce relevé qui tranche.
///
/// **Réservée aux relevés ponctuels.** Pour un appel répété — le contrôle de
/// placement du superviseur court à 1 Hz — voir `enumerer_sorties_silencieux`.
pub fn enumerer_sorties() -> Result<Vec<SortieDxgi>> {
    enumerer(true)
}

/// La même énumération, **sans une ligne de journal**.
///
/// **Correctif I2 de la revue finale de branche.** Le contrôle périodique de
/// `superviseur::boucle` appelait `enumerer_sorties`, dont le `tracing::info!`
/// par adaptateur sans sortie est inconditionnel : deux lignes par seconde,
/// indéfiniment, écrites sur un partage CIFS — 1220 lignes relevées dans un
/// journal de recette de cette branche. Le dépôt a déjà payé pour ce mode de
/// défaillance (« la mesure détruisait ce qu'elle mesurait », `CLAUDE.md`).
///
/// Une variante plutôt qu'une rétrogradation en `debug!` : la trace a une
/// valeur réelle pour les sondes, qui la relèvent une fois — c'est
/// précisément l'angle mort où se cacherait un adaptateur d'affichage virtuel
/// présent mais inactif.
pub fn enumerer_sorties_silencieux() -> Result<Vec<SortieDxgi>> {
    enumerer(false)
}

fn enumerer(journaliser: bool) -> Result<Vec<SortieDxgi>> {
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.context("création de la fabrique DXGI")?;
    let mut sorties = Vec::new();
    let mut index_adaptateur = 0u32;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(index_adaptateur) } {
        let adaptateur = match unsafe { adapter.GetDesc1() } {
            Ok(desc) => String::from_utf16_lossy(&desc.Description)
                .trim_end_matches('\0')
                .trim()
                .to_string(),
            Err(_) => "<inconnu>".to_string(),
        };
        let mut index_sortie = 0u32;
        let nombre_avant = sorties.len();
        while let Ok(output) = unsafe { adapter.EnumOutputs(index_sortie) } {
            if let Ok(desc) = unsafe { output.GetDesc() } {
                let r = desc.DesktopCoordinates;
                sorties.push(SortieDxgi {
                    index_adaptateur,
                    index_sortie,
                    adaptateur: adaptateur.clone(),
                    nom_sortie: String::from_utf16_lossy(&desc.DeviceName)
                        .trim_end_matches('\0')
                        .to_string(),
                    attachee_au_bureau: desc.AttachedToDesktop.as_bool(),
                    rect: crate::geometry::Rect {
                        x: r.left,
                        y: r.top,
                        width: (r.right - r.left).max(0) as u32,
                        height: (r.bottom - r.top).max(0) as u32,
                    },
                });
            }
            index_sortie += 1;
        }
        if journaliser && sorties.len() == nombre_avant {
            // Un adaptateur sans aucune sortie ne produit jamais de
            // `SortieDxgi` : sans cette trace, il resterait invisible du
            // relevé, qui ne journalise aujourd'hui que par sortie. C'est
            // précisément l'angle mort où se cacherait un adaptateur
            // d'affichage virtuel présent mais inactif.
            tracing::info!(
                adaptateur = %adaptateur,
                index_adaptateur,
                "adaptateur DXGI sans sortie"
            );
        }
        index_adaptateur += 1;
    }
    Ok(sorties)
}
