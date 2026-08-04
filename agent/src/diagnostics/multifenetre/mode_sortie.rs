//! P1 du sous-bloc D8 : une sortie virtuelle SudoVDA accepte-t-elle un autre
//! mode d'affichage que celui de sa création ?
//!
//! **L'énumération ne suffit pas, et c'est tout le sujet de cette sonde.** Un
//! pilote peut annoncer un mode et le refuser, comme il peut accepter un mode
//! qu'il n'énumère pas. Elle fait donc les TROIS, dans l'ordre : elle énumère
//! (`EnumDisplaySettingsExW`), PUIS elle change réellement
//! (`ChangeDisplaySettingsExW`), PUIS elle relit par
//! `crate::capture::enumerer_sorties` — la même relecture DXGI
//! (`GetDesc`/`DesktopCoordinates`) que tout ce module emploie déjà, jamais
//! WMI, dont le champ de résolution a été vu périmé de 68 s sur ce terrain
//! (voir `moniteurs_virtuels.rs`).
//!
//! **Le verdict est ce que la relecture DXGI établit, jamais ce que
//! `ChangeDisplaySettingsExW` retourne.** Un `DISP_CHANGE_SUCCESSFUL` sur une
//! sortie dont les dimensions n'ont pas bougé est un refus déguisé.
//!
//! `MULTIFENETRE_MODE_SORTIE=<L>x<H>` : crée une sortie à la résolution de
//! production (`montee::RESOLUTION`, 1280×720 — le chemin par lequel le
//! produit fait paraître ses sorties), tente de la faire passer à L×H par
//! trois combinaisons de drapeaux `CDS_*` croissantes, relit après chacune,
//! s'arrête à la première qui tient, puis rend la sortie au pilote.
//! `CDS_SET_PRIMARY` n'est employé dans AUCUNE combinaison : on ne touche pas
//! au moniteur primaire.
//!
//! Refus et acceptation sont deux résultats de mesure également valides —
//! aucun des deux n'est une panne de la sonde. Un refus a son repli déjà acté
//! ailleurs dans le plan (l'upscale, documenté puis accepté).

use std::collections::HashSet;

use anyhow::{Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExW, EnumDisplaySettingsExW, CDS_NORESET, CDS_RESET, CDS_TYPE,
    CDS_UPDATEREGISTRY, DEVMODEW, DISP_CHANGE_SUCCESSFUL, DM_PELSHEIGHT, DM_PELSWIDTH,
    ENUM_DISPLAY_SETTINGS_FLAGS, ENUM_DISPLAY_SETTINGS_MODE,
};

use super::capture_virtuelle::designer_sortie_neuve;
use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use crate::moniteurs_virtuels::pilote::{ouvrir_pilote, PiloteParIoctl};
use crate::moniteurs_virtuels::Sorties;

/// Modes que la sortie ANNONCE (`EnumDisplaySettingsExW`, énumération pure) —
/// ne dit rien de ce qu'elle accepte réellement, c'est tout le sujet du
/// module.
fn modes_annonces(nom_sortie: &str) -> Vec<(u32, u32)> {
    let nom: Vec<u16> = nom_sortie.encode_utf16().chain(std::iter::once(0)).collect();
    let mut modes = Vec::new();
    let mut index = 0u32;
    loop {
        let mut dm =
            DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
        let ok = unsafe {
            EnumDisplaySettingsExW(
                PCWSTR(nom.as_ptr()),
                ENUM_DISPLAY_SETTINGS_MODE(index),
                &mut dm,
                ENUM_DISPLAY_SETTINGS_FLAGS(0),
            )
        };
        if !ok.as_bool() {
            break;
        }
        modes.push((dm.dmPelsWidth, dm.dmPelsHeight));
        index += 1;
    }
    modes.sort_unstable();
    modes.dedup();
    modes
}

/// Une combinaison de drapeaux `CDS_*` à essayer, du moins au plus insistant.
/// `CDS_SET_PRIMARY` est délibérément absent des deux variantes : on ne
/// touche pas au moniteur primaire (brief, étape 7).
enum Combo {
    /// Un seul appel `ChangeDisplaySettingsExW`, drapeaux directs.
    Simple(&'static str, CDS_TYPE),
    /// L'idiome multi-écran standard de l'API Win32 : la sortie ciblée est
    /// modifiée avec `CDS_NORESET` (le changement est différé et n'est pas
    /// appliqué), puis un second appel — sans nom de périphérique, sans
    /// `DEVMODE` — applique tout ce qui est en attente avec `CDS_RESET` seul.
    NoresetPuisReset(&'static str),
}

impl Combo {
    fn etiquette(&self) -> &'static str {
        match self {
            Combo::Simple(etiquette, _) | Combo::NoresetPuisReset(etiquette) => etiquette,
        }
    }
}

/// Construit le `DEVMODEW` ciblant `largeur`×`hauteur` et tente le
/// changement avec les drapeaux donnés. Rend le code brut de
/// `ChangeDisplaySettingsExW` — un `DISP_CHANGE_SUCCESSFUL` ici ne prouve
/// rien par lui-même, voir le commentaire de tête du module.
fn changer_mode(nom_sortie: &str, largeur: u32, hauteur: u32, drapeaux: CDS_TYPE) -> i32 {
    let nom: Vec<u16> = nom_sortie.encode_utf16().chain(std::iter::once(0)).collect();
    let dm = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        dmFields: DM_PELSWIDTH | DM_PELSHEIGHT,
        dmPelsWidth: largeur,
        dmPelsHeight: hauteur,
        ..Default::default()
    };
    unsafe {
        ChangeDisplaySettingsExW(PCWSTR(nom.as_ptr()), Some(&dm as *const DEVMODEW), None, drapeaux, None)
            .0
    }
}

/// Applique une combinaison et rend le code qui compte pour le verdict — le
/// second appel pour `NoresetPuisReset`, puisque c'est lui qui applique
/// effectivement le changement différé par le premier.
fn appliquer_combo(nom_sortie: &str, largeur: u32, hauteur: u32, combo: &Combo) -> i32 {
    match combo {
        Combo::Simple(etiquette, drapeaux) => {
            let code = changer_mode(nom_sortie, largeur, hauteur, *drapeaux);
            tracing::info!(etiquette, code, "combinaison de drapeaux tentee (appel unique)");
            code
        }
        Combo::NoresetPuisReset(etiquette) => {
            let premier = changer_mode(nom_sortie, largeur, hauteur, CDS_UPDATEREGISTRY | CDS_NORESET);
            // Second appel : NUL nom de périphérique, NUL DEVMODE — c'est
            // ainsi que Win32 documente l'application groupée des
            // changements différés par CDS_NORESET.
            let second =
                unsafe { ChangeDisplaySettingsExW(PCWSTR::null(), None, None, CDS_RESET, None).0 };
            tracing::info!(
                etiquette,
                code_premier_appel = premier,
                code_second_appel = second,
                "combinaison de drapeaux tentee (deux appels : NORESET puis RESET seul)"
            );
            second
        }
    }
}

/// Essaie les combinaisons connues jusqu'à ce que la relecture DXGI confirme
/// la cible, ou jusqu'à épuisement. Rend toujours `Ok` : un refus est une
/// mesure, pas une erreur — voir le commentaire de tête du module. Seule une
/// topologie devenue illisible fait remonter une erreur.
fn essayer_les_modes(pilote: &PiloteParIoctl, nom_sortie: &str, cible: (u32, u32)) -> Result<()> {
    let annonces = modes_annonces(nom_sortie);
    tracing::info!(
        nombre = annonces.len(),
        contient_cible = annonces.contains(&cible),
        modes = ?annonces,
        "modes annonces (EnumDisplaySettingsExW) avant tout changement"
    );

    let combos = [
        Combo::Simple("CDS_UPDATEREGISTRY seul", CDS_UPDATEREGISTRY),
        Combo::Simple("CDS_UPDATEREGISTRY | CDS_RESET", CDS_UPDATEREGISTRY | CDS_RESET),
        Combo::NoresetPuisReset(
            "CDS_UPDATEREGISTRY|CDS_NORESET puis CDS_RESET seul (idiome multi-ecran)",
        ),
    ];

    let mut dernier_code = 0i32;
    let mut derniere_taille = (0u32, 0u32);
    let mut gagnante: Option<&'static str> = None;
    for combo in &combos {
        dernier_code = appliquer_combo(nom_sortie, cible.0, cible.1, combo);
        // Windows reconfigure sa topologie d'affichage de façon asynchrone —
        // exactement pourquoi `montee.rs` observe le même délai de grâce
        // après une création. Interroger DXGI trop tôt ferait conclure à un
        // refus là où il n'y a qu'un délai, et battre le chien de garde
        // pendant l'attente comme le fait le reste de ce module.
        attendre_en_pinguant(pilote, DELAI_TOPOLOGIE)?;
        let releve = relever_topologie(&format!("après tentative « {} »", combo.etiquette()))?;
        derniere_taille = releve
            .iter()
            .find(|sortie| sortie.nom_sortie == nom_sortie)
            .map(|sortie| (sortie.rect.width, sortie.rect.height))
            .unwrap_or((0, 0));
        let conforme = derniere_taille == cible;
        tracing::info!(
            etiquette = combo.etiquette(),
            code_brut = dernier_code,
            api_annonce_succes = (dernier_code == DISP_CHANGE_SUCCESSFUL.0),
            largeur_relue = derniere_taille.0,
            hauteur_relue = derniere_taille.1,
            conforme,
            "relecture DXGI (GetDesc/DesktopCoordinates) apres la tentative"
        );
        if conforme {
            gagnante = Some(combo.etiquette());
            break;
        }
    }

    tracing::info!(
        verdict = if gagnante.is_some() { "P1 RECU" } else { "P1 REFUSE" },
        combinaison_gagnante = ?gagnante,
        code_brut_dernier_essai = dernier_code,
        largeur_relue = derniere_taille.0,
        hauteur_relue = derniere_taille.1,
        largeur_cible = cible.0,
        hauteur_cible = cible.1,
        "verdict P1 : une sortie virtuelle accepte-t-elle un autre mode que celui de sa creation"
    );
    Ok(())
}

/// Sonde `MULTIFENETRE_MODE_SORTIE=<L>x<H>`.
pub(super) fn executer(consigne: &str) -> Result<()> {
    let (l, h) = consigne.split_once('x').with_context(|| {
        format!(
            "MULTIFENETRE_MODE_SORTIE='{consigne}' invalide, attendu <largeur>x<hauteur> \
             (par exemple 1920x1080)"
        )
    })?;
    let cible: (u32, u32) = (
        l.trim().parse().context("largeur invalide dans MULTIFENETRE_MODE_SORTIE")?,
        h.trim().parse().context("hauteur invalide dans MULTIFENETRE_MODE_SORTIE")?,
    );

    // Relevé AVANT toute création, comme les sondes voisines : sans lui, une
    // restauration manuelle après plantage se ferait à l'aveugle.
    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);
    let connues: HashSet<String> = avant.iter().map(|sortie| sortie.nom_sortie.clone()).collect();

    let pilote = ouvrir_pilote()?;
    let (largeur_creation, hauteur_creation, hertz) = RESOLUTION;

    // Portée explicite : la garde `Sorties` doit avoir détruit AVANT le
    // relevé final, sans quoi celui-ci décrirait un état transitoire — même
    // discipline que `montee.rs` et `capture_virtuelle.rs`.
    let issue = {
        let mut sorties = Sorties::nouvelles(&pilote);
        let id = sorties.creer(largeur_creation, hauteur_creation, hertz)?;
        attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;

        let apres_creation = relever_topologie("après création")?;
        let virtuelle = designer_sortie_neuve(&apres_creation, &connues, id)?;
        let nom_sortie = virtuelle.nom_sortie.clone();
        tracing::info!(nom = %nom_sortie, "sortie de sonde créée à 1280x720 (résolution de production)");

        essayer_les_modes(&pilote, &nom_sortie, cible)
        // La garde `sorties` rend la sortie au pilote ici, à la sortie de
        // portée.
    };

    // Second essai des retraits que la garde n'a pas obtenus : dernière
    // chance de CE processus, au-delà seule la purge inter-processus les
    // atteindra.
    let rejoues = crate::moniteurs_virtuels::purge::rejouer_purge_due(&pilote);
    if rejoues > 0 {
        tracing::info!(rejoues, "retraits dus rejoués avec succès après la garde");
    }

    // Une sortie virtuelle survit au processus. Ce contrôle reste celui du
    // processus mesureur, donc juge et partie — le contrôle qui vaut est un
    // relevé `MULTIFENETRE_DXGI=1` depuis un processus NEUF, après coup, en
    // comparant des ENSEMBLES DE NOMS et jamais des cardinaux.
    std::thread::sleep(DELAI_TOPOLOGIE);
    let final_ = relever_topologie("après destruction")?;
    let noms_final = noms_attaches(&final_);
    if noms_final == noms_avant && final_.len() == avant.len() {
        tracing::info!(noms = ?noms_final, "état initial restauré — mêmes sorties, nommément");
    } else {
        tracing::error!(
            noms_avant = ?noms_avant,
            noms_apres = ?noms_final,
            total_avant = avant.len(),
            total_apres = final_.len(),
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }

    issue
}
