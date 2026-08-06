//! Les combinaisons de drapeaux `CDS_*` que la sonde P1 essaie, du moins au
//! plus insistant, et le mécanisme qui les applique.
//!
//! Extrait de `mode_sortie.rs` à la tâche 1 du sous-bloc D9 : la quatrième
//! combinaison (et son commentaire de justification, qui doit rester auprès
//! d'elle) poussait le fichier parent au-dessus du plafond de 500 lignes
//! (`CLAUDE.md`). Aucune de ces fonctions ne touche à un champ privé de
//! `mode_sortie` — elles prennent le nom de sortie et les dimensions en
//! paramètres — donc aucune raison d'accès n'imposait de les garder dans le
//! fichier parent.

use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExW, CDS_NORESET, CDS_RESET, CDS_TYPE, CDS_UPDATEREGISTRY, DEVMODEW,
    DM_PELSHEIGHT, DM_PELSWIDTH,
};

/// Une combinaison de drapeaux `CDS_*` à essayer, du moins au plus insistant.
/// `CDS_SET_PRIMARY` est délibérément absent des deux variantes : on ne
/// touche pas au moniteur primaire (brief, étape 7 de D8).
pub(super) enum Combo {
    /// Un seul appel `ChangeDisplaySettingsExW`, drapeaux directs.
    Simple(&'static str, CDS_TYPE),
    /// L'idiome multi-écran standard de l'API Win32 : la sortie ciblée est
    /// modifiée avec `CDS_NORESET` (le changement est différé et n'est pas
    /// appliqué), puis un second appel — sans nom de périphérique, sans
    /// `DEVMODE` — applique tout ce qui est en attente avec `CDS_RESET` seul.
    NoresetPuisReset(&'static str),
}

impl Combo {
    pub(super) fn etiquette(&self) -> &'static str {
        match self {
            Combo::Simple(etiquette, _) | Combo::NoresetPuisReset(etiquette) => etiquette,
        }
    }
}

/// Les quatre bras, dans l'ordre croissant d'insistance.
///
/// Reconstruite à chaque appel plutôt que mémorisée : `CDS_TYPE` et
/// `&'static str` sont `Copy`, la reconstruction ne coûte rien, et cela évite
/// un état partagé entre le tour principal et le témoin (`combo_pour_temoin`),
/// qui a besoin de sa propre copie pour en extraire UN bras sans emprunter
/// celle du tour principal.
pub(super) fn combos() -> [Combo; 4] {
    [
        // `CDS_TYPE(0)` — changement DYNAMIQUE, non écrit au registre. C'est
        // le remède candidat de C1 : le produit écrit aujourd'hui
        // `CDS_UPDATEREGISTRY` à chaque plein écran réussi, et une sortie
        // NAÎT à la dernière taille laissée au registre (chaîne
        // `avant(N) = après(N-1)`, tâche 3bis de D8) — si bien qu'il
        // bloquerait ses propres ouvertures de fenêtre ultérieures. Les
        // trois combinaisons éprouvées par D8 portaient TOUTES
        // `CDS_UPDATEREGISTRY` : ce bras-ci n'a jamais été tenté.
        Combo::Simple("aucun drapeau (dynamique, non persisté)", CDS_TYPE(0)),
        Combo::Simple("CDS_UPDATEREGISTRY seul", CDS_UPDATEREGISTRY),
        Combo::Simple("CDS_UPDATEREGISTRY | CDS_RESET", CDS_UPDATEREGISTRY | CDS_RESET),
        Combo::NoresetPuisReset(
            "CDS_UPDATEREGISTRY|CDS_NORESET puis CDS_RESET seul (idiome multi-ecran)",
        ),
    ]
}

/// Les combos à essayer CE tour : les quatre, dans l'ordre, sauf si `imposee`
/// restreint le tour à une seule (tâche 2bis, sous-bloc D9,
/// `MULTIFENETRE_MODE_SORTIE_DRAPEAUX`) — `CDS_TYPE(0)` réussissant TOUJOURS
/// au premier essai (relevé de la tâche 2), les trois autres bras, dont
/// `CDS_UPDATEREGISTRY`, ne sont sinon jamais sollicités.
///
/// `imposee` doit correspondre EXACTEMENT à l'une des quatre étiquettes de
/// `combos()` ; sinon le tour n'essaie rien (vecteur vide), plutôt qu'un repli
/// silencieux sur les quatre qui masquerait une étiquette mal orthographiée —
/// d'où l'avertissement si le filtre ne retient rien.
pub(super) fn combos_du_tour(imposee: Option<&str>) -> Vec<Combo> {
    let toutes = combos();
    let Some(etiquette) = imposee else {
        return toutes.into_iter().collect();
    };
    let filtrees: Vec<Combo> =
        toutes.into_iter().filter(|combo| combo.etiquette() == etiquette).collect();
    if filtrees.is_empty() {
        tracing::warn!(
            etiquette_demandee = etiquette,
            etiquettes_connues = ?combos().map(|c| c.etiquette()),
            "MULTIFENETRE_MODE_SORTIE_DRAPEAUX ne correspond à AUCUNE combinaison connue -- \
             le tour n'essaiera rien"
        );
    }
    filtrees
}

/// Le bras à rejouer pour le témoin (étape 3, brief D9) : le MÊME qui a gagné
/// le tour principal, ou le premier si aucun n'a gagné. Rend une valeur
/// possédée — pas une référence dans le tableau du tour principal, qui a déjà
/// fini de vivre à l'endroit où le témoin s'exécute.
pub(super) fn combo_pour_temoin(gagnante: Option<&'static str>) -> Combo {
    match gagnante {
        Some(etiquette) => combos()
            .into_iter()
            .find(|combo| combo.etiquette() == etiquette)
            // Ne peut pas se produire : `etiquette` vient forcément d'un
            // appel antérieur à `combos()`, qui rend toujours les mêmes
            // quatre littéraux `&'static str`. Repli sur le premier bras
            // plutôt qu'un `expect` : un témoin dégradé vaut mieux qu'une
            // sonde qui panique sur son propre repli.
            .unwrap_or_else(|| combos().into_iter().next().unwrap()),
        None => combos().into_iter().next().unwrap(),
    }
}

/// Construit le `DEVMODEW` ciblant `largeur`×`hauteur` et tente le
/// changement avec les drapeaux donnés. Rend le code brut de
/// `ChangeDisplaySettingsExW` — un `DISP_CHANGE_SUCCESSFUL` ici ne prouve
/// rien par lui-même, voir le commentaire de tête de `mode_sortie.rs`.
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
pub(super) fn appliquer_combo(nom_sortie: &str, largeur: u32, hauteur: u32, combo: &Combo) -> i32 {
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
