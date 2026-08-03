//! La mesure pivot du sous-bloc D5 : **détruire un encodeur libère-t-il la
//! place ?**
//!
//! La question est ouverte depuis le 31 juillet 2026 et conditionne tout le
//! sous-bloc — le remède au défaut de D4 comme la mise en sommeil elle-même.
//! La séquence « créer 8 → en détruire 1 → tenter un 9ᵉ » n'avait jamais été
//! jouée.
//!
//! **Le montage reproduit l'arrangement de PRODUCTION** : un processus, un
//! périphérique D3D11 par encodeur (chaque `DesktopCapture` crée le sien, voir
//! `capture/ouverture.rs`). Ce n'est ni le mode `partage` ni le mode `separe`
//! de `nvenc.rs`, et c'est le seul montage dont la réponse engage le produit.
//!
//! **Le cycle répété est le cœur de la mesure, pas un supplément.** Un seul
//! recyclage ne distingue pas un plafond de CONCURRENCE (8 vivants à la fois)
//! d'un plafond de CRÉATIONS CUMULÉES avec du mou : le premier cycle passerait
//! dans les deux cas. Le vivier de D5 recycle des encodeurs par construction —
//! c'est précisément lui qui déclencherait la seconde panne.

#![cfg(windows)]

use anyhow::Result;

use super::nvenc::peripherique_autonome;
use crate::encode::H264Encoder;

/// Les paramètres exacts de la seconde recette de D4, pour que le chiffre soit
/// opposable au sien.
const LARGEUR: u32 = 1280;
const HAUTEUR: u32 = 720;
const FPS: u32 = 60;
const DEBIT: u32 = 8_000_000;

/// On cesse de chercher au-delà : le plafond attendu est 8.
const PLAFOND_RECHERCHE: usize = 16;

/// Un encodeur et le périphérique qui le porte. Le périphérique DOIT vivre
/// aussi longtemps que l'encodeur ; les relâcher séparément ferait mesurer
/// autre chose que ce qu'on croit.
///
/// Les trois champs ne sont que des porteurs de durée de vie : aucun n'est
/// jamais lu, ils existent uniquement pour que leurs objets restent vivants
/// jusqu'au `drop` de l'`Instance`, d'où le préfixe `_` sur les trois — un
/// champ `_x` reste possédé et se détruit normalement, seule la lecture est
/// tue.
struct Instance {
    _peripherique: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    _contexte: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    _encodeur: H264Encoder,
}

/// Construit une instance complète, ou rend l'erreur du refus.
fn construire() -> Result<Instance> {
    let (peripherique, contexte) = peripherique_autonome()?;
    let encodeur = H264Encoder::new(
        &peripherique,
        (LARGEUR, HAUTEUR),
        (LARGEUR, HAUTEUR),
        FPS,
        DEBIT,
    )?;
    Ok(Instance { _peripherique: peripherique, _contexte: contexte, _encodeur: encodeur })
}

pub(super) fn mesurer(cycles: usize) -> Result<()> {
    tracing::info!(cycles, largeur = LARGEUR, hauteur = HAUTEUR, fps = FPS, debit = DEBIT,
        "mesure pivot D5 : recyclage d'encodeurs, un périphérique D3D11 par encodeur");

    // ── Phase 1 : monter jusqu'au refus, et le NOMMER.
    //
    // Sans ce témoin, rien de ce qui suit ne prouve quoi que ce soit : un 9ᵉ
    // qui réussit après une destruction ne dit rien si le 9ᵉ réussissait déjà
    // avant.
    let mut vivants: Vec<Instance> = Vec::new();
    let mut plafond = 0usize;
    for rang in 1..=PLAFOND_RECHERCHE {
        match construire() {
            Ok(instance) => {
                vivants.push(instance);
                tracing::info!(rang, vivants = vivants.len(), "phase 1 : encodeur créé");
            }
            Err(erreur) => {
                plafond = rang - 1;
                tracing::info!(
                    phase = 1,
                    plafond,
                    rang_refuse = rang,
                    causes = %super::causes(erreur),
                    "phase 1 : plafond atteint — création refusée"
                );
                break;
            }
        }
    }
    if plafond == 0 {
        tracing::warn!(
            plafond_recherche = PLAFOND_RECHERCHE,
            "phase 1 : aucun refus sous le plafond de recherche — la mesure pivot est SANS OBJET"
        );
        drop(vivants);
        return Ok(());
    }

    // ── Phase 2 : le cycle. Détruire un, en construire un, k fois.
    //
    // `vivants` contient exactement `plafond` instances. À chaque tour on en
    // retire une (destruction réelle : `drop` explicite, tracé de part et
    // d'autre pour qu'un gel de `Drop for H264Encoder` se lise comme tel) puis
    // on tente d'en construire une neuve.
    let mut reussis = 0usize;
    let mut premier_echec: Option<usize> = None;
    for cycle in 1..=cycles {
        let retiree = vivants.pop().expect("le vivier ne peut pas être vide ici");
        tracing::info!(cycle, restants = vivants.len(), "cycle : relâchement d'un encodeur");
        drop(retiree);
        tracing::info!(cycle, restants = vivants.len(), "cycle : relâchement terminé");

        match construire() {
            Ok(instance) => {
                vivants.push(instance);
                reussis += 1;
                tracing::info!(cycle, vivants = vivants.len(), "cycle : reconstruction RÉUSSIE");
            }
            Err(erreur) => {
                premier_echec = Some(cycle);
                tracing::info!(
                    cycle,
                    vivants = vivants.len(),
                    causes = %super::causes(erreur),
                    "cycle : reconstruction REFUSÉE"
                );
                break;
            }
        }
    }

    // ── Verdict, en une ligne lisible sans le reste du journal.
    match premier_echec {
        None => tracing::info!(
            plafond,
            cycles_demandes = cycles,
            cycles_reussis = reussis,
            verdict = "concurrence",
            "VERDICT : détruire un encodeur libère la place, et les cycles tiennent"
        ),
        Some(0) | Some(1) => tracing::info!(
            plafond,
            cycles_reussis = reussis,
            verdict = "aucune-liberation",
            "VERDICT : détruire un encodeur ne libère PAS la place"
        ),
        Some(k) => tracing::info!(
            plafond,
            cycles_reussis = reussis,
            premier_echec = k,
            verdict = "cumule",
            "VERDICT : plafond de créations CUMULÉES — le recyclage lâche au cycle {k}"
        ),
    }

    tracing::info!(vivants = vivants.len(), "relâchement final");
    drop(vivants);
    tracing::info!("relâchement final terminé — le processus a survécu");
    Ok(())
}
