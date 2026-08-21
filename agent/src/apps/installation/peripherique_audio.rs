//! Le périphérique de rendu par défaut, relevé AVANT et APRÈS une installation.
//!
//! 🔴 POURQUOI CE MODULE EXISTE, ET CE N'EST PAS UNE PRÉCAUTION THÉORIQUE.
//! **Le 19 août 2026, l'installation de VB-Cable a fait basculer le rendu par
//! défaut de Windows sur un câble virtuel que rien n'alimente**, et le loopback
//! du chantier A — qui suivait ce défaut — s'est mis à capter du silence **sans
//! qu'aucune ligne de journal ne dise pourquoi**. Le diagnostic a coûté une
//! campagne. **C'est le cas NOMINAL, pas un cas limite** : tout installeur
//! audio le rejouera.
//!
//! 🔴 DEUX LIGNES, AVANT ET APRÈS, AVEC LE MÊME NOM DE CHAMP ET UN
//! `moment=avant|apres`. Ne tracer qu'APRÈS rendrait un changement
//! INATTRIBUABLE — on saurait quel périphérique est le défaut, jamais s'il
//! l'était déjà. C'est exactement ce qui a coûté la campagne.
//!
//! ⚠️ **AUCUNE RESTAURATION, AUCUN GARDE-FOU** (spec D8, décision D19 du plan).
//! Remettre d'autorité le périphérique d'avant serait décider à la place de
//! l'utilisateur qui vient d'installer un périphérique audio EXPRÈS. Ce module
//! observe ; il ne répare pas, et il ne prétend pas le faire.
//!
//! ⚠️ CE MODULE PORTE UN `cfg`, comme `execution`. Il n'y a rien de pur à en
//! sortir : la règle de choix du périphérique est déjà PURE et déjà testée
//! ailleurs (`wasapi_peripherique`, chantier A-bis) ; ici on ne fait que
//! l'interroger et écrire une ligne.

use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

/// Où l'on en est de l'installation quand on relève.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    Avant,
    Apres,
}

impl Moment {
    fn mot(self) -> &'static str {
        match self {
            Self::Avant => "avant",
            Self::Apres => "apres",
        }
    }
}

/// Relève le périphérique de rendu capté et l'écrit au journal.
///
/// ⚠️ **ELLE NE REND RIEN, ET NE PEUT PAS ÉCHOUER POUR L'APPELANT.** Un relevé
/// impossible ne doit pas empêcher une installation : il se journalise en
/// `warn!` et l'on continue. L'inverse — refuser d'installer parce qu'on n'a
/// pas su nommer une carte son — serait une panne bien pire que le risque
/// qu'elle prétend couvrir.
pub fn tracer(installation: &str, moment: Moment) {
    match relever() {
        Ok(identifiant) => tracing::info!(
            installation,
            moment = moment.mot(),
            peripherique_rendu = %identifiant,
            "peripherique de rendu par defaut, autour d'une installation"
        ),
        Err(erreur) => tracing::warn!(
            installation,
            moment = moment.mot(),
            %erreur,
            "peripherique de rendu par defaut illisible : le changement \
             eventuel ne sera pas attribuable"
        ),
    }
}

fn relever() -> anyhow::Result<String> {
    // ⚠️ L'APPARTEMENT COM EST CELUI DE L'APPELANT. Ce fil est le fil
    // d'installation, qui a déjà appelé `CoInitializeEx` — comme le fil de
    // découverte le fait pour `IShellLinkW`. Le refaire ici serait au mieux
    // inutile, au pire un changement de modèle d'appartement sous les pieds
    // d'un objet vivant.
    //
    // SAFETY : `CoCreateInstance` sur un fil dont l'appartement est initialisé.
    let enumerateur: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
    crate::wasapi::rendu::identifiant_capte(&enumerateur)
}
