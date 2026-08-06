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
//!
//! # Défaut corrigé le 4 août 2026 (F1 rejoué) — voir `p1-mode-sortie.log`
//!
//! La première exécution a rendu « P1 RECU » sans valeur : elle demandait à
//! la sortie la taille qu'elle avait **déjà** au moment de la création
//! (`sortie moment="après création" … largeur=1920 hauteur=1080`, alors que
//! la sonde croyait avoir créé du 1280×720 — persistance probable au
//! registre d'un `CDS_UPDATEREGISTRY` d'une exécution antérieure). Le critère
//! `derniere_taille == cible` était donc vrai AVANT toute tentative :
//! `ChangeDisplaySettingsExW` n'a jamais eu l'occasion de changer quoi que ce
//! soit, et rien ne pouvait faire échouer le contrôle. Exactement le défaut
//! que ce dépôt appelle F1 (un contrôle qui ne peut pas rendre l'autre
//! verdict), rejoué sur l'instrument censé l'éviter.
//!
//! **Le remède tient en deux points, tous deux appliqués ci-dessous :**
//! 1. la taille courante est relevée par DXGI (`GetDesc`/`DesktopCoordinates`,
//!    jamais WMI) **avant toute tentative**, sous une clé sans ambiguïté
//!    (`taille_avant_tentative_*`), et comparée à la cible ;
//! 2. la cible n'est plus un couple fixe reçu tel quel : elle est choisie
//!    dynamiquement parmi les modes que `EnumDisplaySettingsExW` annonce, **en
//!    excluant la taille courante** (`choisir_cible`). Si aucun mode
//!    n'en diffère — cas dégénéré, non rencontré en pratique avec les neuf
//!    modes de cette VM — la sonde REFUSE de mesurer (`P1 NON MESURABLE`)
//!    plutôt que de rendre un verdict vide ;
//! 3. **le verdict lui-même est un MOUVEMENT observé, pas une égalité à la
//!    cible.** `P1 RECU` ⟺ la taille relue par DXGI après une tentative
//!    diffère de `avant` — exactement la question que pose le titre du
//!    module (« un AUTRE mode que celui de sa création »), pas « CE mode
//!    précis-là ». Que le pilote honore ou non la valeur exacte demandée est
//!    journalisé à part (`cible_atteinte`/`cible_exacte_atteinte`) : une
//!    question plus fine, qui peut valoir `false` sous un verdict `P1 RECU`
//!    sans que ce soit une contradiction.

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

/// Choisit la cible à tenter : la résolution demandée si elle diffère déjà de
/// la taille courante et figure parmi les modes annoncés, sinon une cible
/// prise dynamiquement dans les modes annoncés, **en excluant systématiquement
/// la taille courante** (`avant`) — c'est ce qui rend impossible de rejouer le
/// défaut F1 documenté en tête du module : quel que soit l'état où la
/// persistance au registre a laissé la sortie, la cible retenue en diffère
/// par construction, sauf le cas dégénéré rendu par `None`.
///
/// Modes triés par `modes_annonces` (croissant, dédupliqués) : le repli
/// choisit le plus grand mode distinct de `avant`, pour un mouvement large et
/// donc sans ambiguïté de mesure.
fn choisir_cible(avant: (u32, u32), demande: (u32, u32), annonces: &[(u32, u32)]) -> Option<(u32, u32)> {
    if demande != avant && annonces.contains(&demande) {
        return Some(demande);
    }
    annonces.iter().copied().rev().find(|&mode| mode != avant)
}

/// Essaie les combinaisons connues jusqu'à ce que la relecture DXGI confirme
/// la cible, ou jusqu'à épuisement. Rend toujours `Ok` : un refus — ou
/// l'impossibilité de choisir une cible mesurable — est une mesure, pas une
/// erreur de la sonde ; voir le commentaire de tête du module. Seule une
/// topologie devenue illisible fait remonter une erreur.
///
/// `avant` est la taille lue par DXGI juste après la création, PAS supposée
/// être la résolution de création : c'est exactement la valeur dont l'absence
/// a rendu le premier relevé vide de sens (voir le commentaire de tête).
fn essayer_les_modes(
    pilote: &PiloteParIoctl,
    nom_sortie: &str,
    avant: (u32, u32),
    demande: (u32, u32),
) -> Result<()> {
    let annonces = modes_annonces(nom_sortie);
    tracing::info!(
        nombre = annonces.len(),
        largeur_avant_tentative = avant.0,
        hauteur_avant_tentative = avant.1,
        contient_demande = annonces.contains(&demande),
        demande_egale_avant = demande == avant,
        modes = ?annonces,
        "modes annonces (EnumDisplaySettingsExW) avant tout changement"
    );

    let cible = match choisir_cible(avant, demande, &annonces) {
        Some(cible) => cible,
        None => {
            tracing::error!(
                verdict = "P1 NON MESURABLE",
                raison = "aucun mode annonce ne differe de la taille courante",
                largeur_avant_tentative = avant.0,
                hauteur_avant_tentative = avant.1,
                largeur_demandee = demande.0,
                hauteur_demandee = demande.1,
                modes = ?annonces,
                "verdict P1 : mesure impossible, aucune tentative effectuee"
            );
            return Ok(());
        }
    };
    if cible == demande {
        tracing::info!(
            largeur_cible = cible.0,
            hauteur_cible = cible.1,
            "cible retenue = resolution demandee (differe deja de la taille courante)"
        );
    } else {
        tracing::warn!(
            largeur_demandee = demande.0,
            hauteur_demandee = demande.1,
            largeur_avant_tentative = avant.0,
            hauteur_avant_tentative = avant.1,
            largeur_cible = cible.0,
            hauteur_cible = cible.1,
            "la resolution demandee egale deja la taille courante (persistance registre probable \
             d'une execution anterieure) -- cible substituee dynamiquement parmi les modes annonces"
        );
    }

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
    let mut cible_exacte_atteinte = false;
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
        // Le critère qui compte est le MOUVEMENT (`derniere_taille != avant`),
        // pas l'égalité à la cible choisie — voir le commentaire de tête du
        // module (défaut F1 corrigé). `cible_atteinte` reste journalisé,
        // séparément : il documente si le pilote honore la valeur exacte
        // demandée, une question plus fine que P1, jamais celle qui décide du
        // verdict.
        let mouvement = derniere_taille != avant;
        let cible_atteinte = derniere_taille == cible;
        tracing::info!(
            etiquette = combo.etiquette(),
            code_brut = dernier_code,
            api_annonce_succes = (dernier_code == DISP_CHANGE_SUCCESSFUL.0),
            largeur_relue = derniere_taille.0,
            hauteur_relue = derniere_taille.1,
            mouvement,
            cible_atteinte,
            "relecture DXGI (GetDesc/DesktopCoordinates) apres la tentative"
        );
        if mouvement {
            gagnante = Some(combo.etiquette());
            cible_exacte_atteinte = cible_atteinte;
            break;
        }
    }

    tracing::info!(
        verdict = if gagnante.is_some() { "P1 RECU" } else { "P1 REFUSE" },
        combinaison_gagnante = ?gagnante,
        code_brut_dernier_essai = dernier_code,
        largeur_avant_tentative = avant.0,
        hauteur_avant_tentative = avant.1,
        largeur_relue = derniere_taille.0,
        hauteur_relue = derniere_taille.1,
        largeur_cible = cible.0,
        hauteur_cible = cible.1,
        // Le verdict lui-même : un mouvement (A != B) a-t-il été observé ?
        // C'est CE champ qui gouverne "P1 RECU" ci-dessus, pas une égalité à
        // la cible (voir le commentaire de tête du module, défaut F1
        // corrigé) -- recalculé ici, redondant avec `gagnante.is_some()` par
        // construction, pour qu'un lecteur du journal n'ait pas à le déduire.
        mouvement_observe = derniere_taille != avant,
        // Secondaire : le pilote a-t-il honoré la valeur EXACTE demandée, ou
        // s'est-il arrêté à un mode intermédiaire ? Peut valoir `false` avec
        // un verdict "P1 RECU" -- ce n'est pas une contradiction, c'est une
        // question plus fine que celle de P1.
        cible_exacte_atteinte,
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
    // `demande` : la préférence de l'opérateur, PAS forcément la cible
    // retenue — voir `choisir_cible`. La conserver permet à un opérateur qui
    // connaît déjà la taille courante de viser directement une cible utile,
    // sans rien changer au format d'appel documenté.
    let demande: (u32, u32) = (
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
        // `taille_avant_tentative` : la taille RÉELLEMENT lue par DXGI juste
        // après la création — PAS supposée être
        // `largeur_creation`×`hauteur_creation`. C'est exactement le relevé
        // dont l'absence a rendu le premier passage de cette sonde vide de
        // sens (voir le commentaire de tête du module) : la sortie peut
        // naître à une taille différente de celle demandée au pilote, par
        // persistance au registre d'un `CDS_UPDATEREGISTRY` antérieur. Nommée
        // à part de `avant` (la topologie complète relevée plus haut, encore
        // en usage plus bas) pour ne rien masquer par ombrage.
        let taille_avant_tentative = (virtuelle.rect.width, virtuelle.rect.height);
        tracing::info!(
            nom = %nom_sortie,
            largeur_demandee_a_la_creation = largeur_creation,
            hauteur_demandee_a_la_creation = hauteur_creation,
            largeur_avant_tentative = taille_avant_tentative.0,
            hauteur_avant_tentative = taille_avant_tentative.1,
            "sortie de sonde créée -- taille relue par DXGI avant toute tentative de changement"
        );

        essayer_les_modes(&pilote, &nom_sortie, taille_avant_tentative, demande)
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
