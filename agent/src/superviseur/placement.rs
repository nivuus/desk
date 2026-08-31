//! Apparier une sortie virtuelle fraîchement créée à une sortie DXGI, puis y
//! poser la fenêtre.
//!
//! 🔴 **« AUCUNE CORRESPONDANCE N'EST EXPOSÉE » ÉTAIT FAUX, ET CETTE PHRASE A
//! GOUVERNÉ LA CONCEPTION DE L'APPARIEMENT DEPUIS D1.** Elle disait : « Le
//! pilote rend un identifiant de cible qui lui appartient, DXGI énumère par
//! `(index_adaptateur, index_sortie)`. Aucune correspondance n'est exposée :
//! l'appariement se fait donc par dimensions et par élimination. » Le premier
//! fait est exact, le second aussi, **la conclusion ne l'est pas** — et c'est
//! d'elle que sortait l'appariement par différence d'ensembles, dont le lot 30
//! a mesuré qu'il refuse toute fenêtre quand la première sortie virtuelle
//! remplace une cible forcée.
//!
//! **Ce qui est vrai** : l'API CCD de Win32 expose la correspondance. Le
//! pilote rend `(adapterId, id de cible)` — `sudovda::SortieAjoutee`, trois
//! nombres dont le produit n'en gardait qu'un —, et ce couple se change en nom
//! GDI par `QueryDisplayConfig` puis `DisplayConfigGetDeviceInfo`. Voir
//! `moniteurs_virtuels::config_affichage`, qui le fait, et
//! `superviseur::designation`, qui s'en sert.
//!
//! **Les commandes qui l'établissent, pour que le prochain lecteur refasse le
//! contrôle sans croire personne** — la première montre les trois nombres que
//! le pilote rend, la seconde que l'API existe dans le crate épinglé :
//!
//! ```text
//! grep -n 'identifiant_cible\|adaptateur_bas' agent/src/moniteurs_virtuels/sudovda.rs
//! grep -rn 'pub unsafe fn QueryDisplayConfig' \
//!   ~/.cargo/registry/src/*/windows-0.62.2/src/Windows/Win32/Devices/Display/mod.rs
//! ```
//!
//! ⚠️ **Ce que la correction ne prétend PAS** : que le couple rendu par
//! SudoVDA soit celui qu'emploie CCD. C'est une hypothèse, elle est dite comme
//! telle dans `config_affichage`, et son échec fait retomber le produit sur
//! l'appariement par élimination décrit ci-dessous — qui reste donc vivant, et
//! reste la raison d'être de tout ce module.
//!
//! **`GetDesc`/`DesktopCoordinates` est la source de vérité, jamais WMI** —
//! le champ WMI a été vu périmé de 68 s sur ce terrain, et la sortie virtuelle
//! y était annoncée 5120×1440 quand DXGI la mesurait 3413×960 (facteur DPI de
//! 1,5). Un placement calculé sur la valeur WMI serait décalé d'autant.

use crate::geometry::Rect;
// `crate::sortie_dxgi`, pas `crate::capture` : `capture` est `#![cfg(windows)]`
// dans son ensemble et n'existe pas du tout à la compilation sur l'hôte Linux
// — voir le commentaire de tête de `sortie_dxgi.rs`. `capture.rs` réexporte ce
// même type sous `crate::capture::SortieDxgi` pour le code Windows.
use crate::sortie_dxgi::SortieDxgi;

/// Tolérance de position et de taille, en pixels, avant de replacer.
///
/// Les bordures invisibles de DWM décalent couramment `GetWindowRect` de
/// quelques pixels par rapport à ce que `SetWindowPos` a demandé. Sans
/// tolérance, le superviseur replacerait la fenêtre à chaque tour de boucle.
///
/// **Quatre, et non deux** : la valeur retenue prend une marge délibérée
/// au-delà du décalage habituel — un écart de quatre pixels sur une fenêtre
/// plein cadre est invisible, là où un replacement en boucle ne l'est pas. (Le
/// commentaire disait « un ou deux pixels » face à une constante à 4 ; c'est le
/// texte qui était en retard, la constante est celle qu'on veut.)
///
/// Cette même tolérance sert désormais aussi à l'appariement d'une sortie
/// fraîchement créée (`sortie_assez_grande`) : elle doit être déclarée avant
/// cette fonction dans le fichier.
const TOLERANCE_PX: i64 = 4;

/// Vrai si une sortie peut servir un viewport donné.
///
/// **Une inégalité, plus une égalité, et c'est tout le sous-bloc D10.** Une
/// sortie virtuelle ne naît PAS à la taille demandée : elle naît à la dernière
/// taille laissée au registre par un `CDS_UPDATEREGISTRY` antérieur (D8,
/// tâche 3bis — confirmé, reproduit, jamais expliqué). Sur cette VM le registre
/// est resté à 3840×2160, et l'égalité à quatre pixels près refusait donc
/// TOUTE sortie : le produit plafonnait à trois fenêtres, aux six exécutions
/// de la recette ③ de D9, sans exception.
///
/// Le produit n'écrit plus au registre depuis D9, mais **rien ne nettoie ce qui
/// y est déjà écrit** — et la portée du blocage (par GUID ou globale) reste
/// inconnue. D'où le choix de tolérer plutôt que de nettoyer : ainsi la
/// question devient **sans objet**, et non résolue.
///
/// La tolérance de `TOLERANCE_PX` est conservée dans le sens du MANQUE, pour la
/// course de rattachement relevée par la recette D1 (sortie créée à 1280×713,
/// rendue à 1280×720 un essai sur deux).
pub fn sortie_assez_grande(sortie: (u32, u32), demandee: (u32, u32)) -> bool {
    let assez = |s: u32, d: u32| s as i64 + TOLERANCE_PX >= d as i64;
    assez(sortie.0, demandee.0) && assez(sortie.1, demandee.1)
}

/// La taille à laquelle la fenêtre est posée, et que la capture recadre.
///
/// `min` axe par axe, **sans préserver le rapport d'aspect** : on recadre une
/// texture, on ne la met pas à l'échelle. C'est l'inverse de
/// `windows_source_sortie::borner_a_la_taille_max`, qui redimensionne et doit
/// donc, lui, préserver ce rapport.
///
/// Dimensions paires (l'encodeur NV12 les exige) et jamais nulles (une boîte
/// vidéo repliée émet `(0, 0)`, cas réel relevé en D8).
pub fn taille_retenue(demandee: (u32, u32), sortie: (u32, u32)) -> (u32, u32) {
    let retenir = |d: u32, s: u32| (d.min(s).max(2)) & !1;
    (retenir(demandee.0, sortie.0), retenir(demandee.1, sortie.1))
}

/// Sortie DXGI capable de servir un viewport, parmi celles qui ne sont pas
/// déjà attribuées.
///
/// **`deja_prises` est ce qui empêche l'inégalité de tout casser.** Avec
/// l'égalité d'avant D10, deux fenêtres au même viewport se disputaient déjà
/// une sortie ; avec « au moins aussi grande », une seule grande sortie
/// conviendrait à TOUTES les fenêtres, et toutes montreraient la même image.
/// Le filtre désigne par NOM DXGI (`\\.\DISPLAYn`), stable, et non par un
/// couple d'index d'énumération, positionnel.
///
/// ⚠️ **L'appelant ne doit chercher QUE parmi les sorties APPARUES** (voir le
/// commentaire de `creation_sortie::creer_sortie`) : le viewport annoncé par le
/// navigateur peut égaler la résolution d'un moniteur PHYSIQUE, et l'inégalité
/// rend ce risque plus grand, pas moins — un moniteur 4K conviendrait
/// désormais à n'importe quel viewport.
pub fn sortie_pour_viewport(
    sorties: &[SortieDxgi],
    largeur: u32,
    hauteur: u32,
    deja_prises: &[String],
) -> Option<SortieDxgi> {
    sorties
        .iter()
        .find(|s| {
            s.attachee_au_bureau
                && sortie_assez_grande((s.rect.width, s.rect.height), (largeur, hauteur))
                && !deja_prises.contains(&s.nom_sortie)
        })
        .cloned()
}

/// Le **lisère invisible de DWM** : ce dont `GetWindowRect` est plus grand que
/// la fenêtre réellement peinte.
///
/// 🔴 **MESURÉ SUR LE PRODUIT, EN SESSION 1, LE 31 AOÛT 2026** — c'est le
/// résidu que le propriétaire voyait encore après le correctif d'aspect
/// (« il y a moins de bordure, mais y en a toujours ») :
///
/// ```text
/// GetWindowRect = 1732x1032+1280+0   <- exactement la taille RETENUE
/// DWM frame     = 1718x1025+1287+0
/// lisere : gauche=7 haut=0 droite=7 bas=7
/// ```
///
/// Depuis Windows 10, les bordures de redimensionnement d'une fenêtre sont
/// **transparentes** : `GetWindowRect` les inclut, l'œil ne les voit pas.
/// Comme `poser` posait dans cet espace-là et que le recadrage suit la taille
/// posée, l'image contenait **7 px de bureau à gauche, 7 à droite, 7 en bas**
/// — constants, **insensibles au rapport d'aspect**, et donc parfaitement
/// distincts des bandes de letterbox que le correctif d'aspect a supprimées.
/// Deux défauts superposés, deux signatures différentes ; c'est le contraste
/// « varie avec la forme » / « constant » qui les a départagés.
///
/// ⚠️ **`haut = 0` et ce n'est pas une erreur** : la barre de titre est peinte,
/// donc le bord supérieur de `GetWindowRect` coïncide avec le cadre visible.
/// Le lisère n'est pas symétrique, et le supposer l'être décalerait l'image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lisere {
    pub gauche: i32,
    pub haut: i32,
    pub droite: i32,
    pub bas: i32,
}

impl Lisere {
    /// Le lisère nul — le repli quand DWM refuse de répondre, et donc
    /// **exactement le comportement d'avant ce correctif**.
    pub const NUL: Lisere = Lisere { gauche: 0, haut: 0, droite: 0, bas: 0 };

    /// Vrai s'il n'y a rien à compenser : évite un second `SetWindowPos` et,
    /// surtout, rend la correction inerte là où elle n'a pas lieu d'être.
    pub fn est_nul(self) -> bool {
        self == Lisere::NUL
    }
}

/// Le rectangle à passer à `SetWindowPos` pour que le cadre **VISIBLE** occupe
/// exactement `cible`.
///
/// 🔴 **TOUT LE CORRECTIF TIENT DANS CES QUATRE ADDITIONS**, et sa difficulté
/// n'est pas l'arithmétique : c'est que le dépôt raisonnait de bout en bout
/// dans l'espace de `GetWindowRect` — `poser` y écrivait, `rectangle_de` y
/// relisait, `doit_etre_replacee` y comparait — sans que rien ne dise que cet
/// espace **n'est pas celui qu'on voit**.
///
/// ⚠️ **`rectangle_de` rend désormais le cadre VISIBLE**, précisément pour que
/// la comparaison périodique se fasse dans le même espace que la cible. Les
/// changer séparément ferait replacer la fenêtre **chaque seconde** : le
/// contrôle verrait un écart permanent de 7 px et n'arriverait jamais à le
/// résorber. Les deux moitiés vont ensemble ou pas du tout.
pub fn rect_a_poser(cible: &Rect, lisere: Lisere) -> Rect {
    Rect {
        x: cible.x - lisere.gauche,
        y: cible.y - lisere.haut,
        // `saturating_add_signed` : un lisère aberrant ne doit pas faire
        // déborder la largeur, ce qui donnerait une fenêtre minuscule.
        width: cible.width.saturating_add_signed(lisere.gauche + lisere.droite),
        height: cible.height.saturating_add_signed(lisere.haut + lisere.bas),
    }
}

/// La taille à passer à `SetWindowPos` pour que le cadre **VISIBLE** mesure
/// `taille`. Le pendant de [`rect_a_poser`] pour le chemin du CAPTEUR, qui
/// retaille sans déplacer (`SWP_NOMOVE`) — l'origine étant déjà compensée par
/// la pose du superviseur, seule la taille reste à corriger.
pub fn taille_a_poser(taille: (u32, u32), lisere: Lisere) -> (u32, u32) {
    (
        taille.0.saturating_add_signed(lisere.gauche + lisere.droite),
        taille.1.saturating_add_signed(lisere.haut + lisere.bas),
    )
}

/// Vrai si la fenêtre a quitté sa sortie ou changé de taille au point qu'il
/// faille la remettre en place.
///
/// C'est le cas que la spec §6 prévoit : une application peut se déplacer ou
/// se retailler d'elle-même, et une fenêtre qui déborde de sa sortie donne une
/// capture tronquée sans que rien ne le signale.
pub fn doit_etre_replacee(actuel: &Rect, cible: &Rect) -> bool {
    let ecart = |a: i64, b: i64| (a - b).abs() > TOLERANCE_PX;
    ecart(actuel.x as i64, cible.x as i64)
        || ecart(actuel.y as i64, cible.y as i64)
        || ecart(actuel.width as i64, cible.width as i64)
        || ecart(actuel.height as i64, cible.height as i64)
}

#[cfg(windows)]
mod win {
    use anyhow::{Context, Result};

    use super::*;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, ShowWindow, HWND_TOP, SWP_NOACTIVATE, SW_SHOWNORMAL,
    };

    /// Pose la fenêtre sur la sortie et lui donne exactement sa taille.
    ///
    /// **Pas de maximisation** — mais ❌ **LA RAISON ÉCRITE ICI EST DEVENUE
    /// FAUSSE, ET LE LOT 33 LA CORRIGE.** Elle disait : « `SW_MAXIMIZE` ferait
    /// adopter à la fenêtre la zone de travail du moniteur, barre des tâches
    /// déduite : l'image capturée ne remplirait alors pas la sortie, et le bas
    /// du flux serait une bande de bureau vide. On pose la taille exacte de la
    /// sortie. »
    ///
    /// **Cet argument ne vaut QUE si le recadrage reste à la taille de la
    /// sortie**, ce qui n'est plus le cas : depuis le lot 33, l'appelant borne
    /// la cible par la ZONE DE TRAVAIL (`placement_periodique::borne_de`) et
    /// le capteur recadre sur cette même borne
    /// (`windows_source_sortie::borne_de_la_sortie`). Il n'y a donc plus de
    /// « bande de bureau vide » à craindre : la bande de 48 px que la
    /// maximisation aurait laissée est précisément celle qu'on RETIRE
    /// désormais du recadrage, parce qu'elle contenait la barre des tâches.
    ///
    /// **Ce qui reste vrai, et pourquoi il n'y a toujours pas de
    /// `SW_MAXIMIZE`** : une fenêtre maximisée ignore silencieusement
    /// `SetWindowPos` (voir le paragraphe suivant), donc le suivi de viewport
    /// — qui retaille la fenêtre à chaque `Resize` — ne pourrait plus rien
    /// poser. On pose la taille exacte, et c'est ce qui rend le suivi
    /// possible.
    ///
    /// **Les commandes qui établissent la mesure**, pour que le prochain
    /// lecteur ne croie personne — elles doivent être jouées EN SESSION 1, par
    /// une tâche planifiée `/it` (un relevé WinRM est en session 0, où
    /// `EnumWindows` ne rend rien) :
    ///
    /// ```text
    /// GetMonitorInfo(\\.\DISPLAY8) -> mon=1428x1080  work=1428x1032
    /// EnumWindows: Shell_SecondaryTrayWnd rect=(1280,1032)-(2708,1080) 1428x48
    /// ```
    ///
    /// Relevé du 31 août 2026 :
    /// `docs/superpowers/plans/2026-08-31-barre-des-taches-diagnostic.md`.
    ///
    /// La fenêtre est d'abord restaurée : une fenêtre minimisée ou déjà
    /// maximisée ignore silencieusement `SetWindowPos`.
    pub fn poser(hwnd: HWND, cible: &Rect) -> Result<()> {
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            // Le lisère est relu APRÈS `ShowWindow` : sur une fenêtre
            // minimisée, DWM rend un cadre qui ne veut rien dire. Un échec de
            // DWM rend `Lisere::NUL`, donc le comportement d'avant.
            let pose = rect_a_poser(cible, crate::window::lisere_dwm(hwnd).unwrap_or_default());
            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                pose.x,
                pose.y,
                pose.width as i32,
                pose.height as i32,
                // `SWP_NOACTIVATE` : poser une fenêtre ne doit pas voler le
                // premier plan à celle que l'utilisateur manipule.
                SWP_NOACTIVATE,
            )
            .context("SetWindowPos vers la sortie virtuelle")?;
        }
        Ok(())
    }

    /// Rectangle **VISIBLE** de la fenêtre, en coordonnées du bureau virtuel.
    ///
    /// ❌ **CETTE FONCTION RENDAIT `GetWindowRect`, ET C'ÉTAIT LA MOITIÉ
    /// LECTURE DU DÉFAUT DU LISÈRE.** Son commentaire disait « `GetWindowRect`
    /// et non `GetClientRect` : c'est la position dans l'espace du bureau
    /// qu'on compare à celle de la sortie » — vrai, mais incomplet : depuis
    /// Windows 10, `GetWindowRect` inclut des bordures **transparentes** de
    /// ~7 px, si bien que l'espace où l'on comparait n'était **pas celui qu'on
    /// voit** (mesure en session 1, voir [`Lisere`]).
    ///
    /// 🔴 **ELLE DOIT CHANGER EN MÊME TEMPS QUE `poser`, JAMAIS SÉPARÉMENT.**
    /// `poser` écrit désormais un rectangle gonflé du lisère ; si la relecture
    /// rendait encore `GetWindowRect`, `doit_etre_replacee` verrait un écart
    /// permanent de 7 px et replacerait la fenêtre **chaque seconde**, sans
    /// jamais converger.
    ///
    /// Le repli sur `GetWindowRect` quand DWM refuse est **exactement le
    /// comportement d'avant**, et il est cohérent avec celui de `poser`, qui
    /// retombe alors sur un lisère nul : les deux moitiés dégradent ensemble.
    pub fn rectangle_de(hwnd: HWND) -> Result<Rect> {
        let brut = crate::window::rectangle_brut(hwnd)?;
        Ok(match crate::window::cadre_visible(hwnd) {
            Ok(visible) => visible,
            Err(_) => brut,
        })
    }
}

#[cfg(windows)]
pub use win::{poser, rectangle_de};

// Les tests d'hôte de ce module vivent dans `placement/tests.rs` — extraction
// du lot 33, qui a ramené ce fichier de 500 lignes EXACTEMENT (sa porte) à sa
// marge. `#[path]` plutôt qu'un sous-répertoire de module : précédent de
// `superviseur/table.rs`. L'en-tête du fichier extrait dit ce qu'il doit dire.
#[cfg(test)]
#[path = "placement/tests.rs"]
mod tests;

