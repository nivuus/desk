//! Pair a freshly created virtual output with a DXGI output, then put
//! the window on it.
//!
//! 🔴 **"NO MAPPING IS EXPOSED" WAS WRONG, AND THIS SENTENCE
//! GOVERNED THE DESIGN OF PAIRING SINCE D1.** It said: "The
//! driver returns a target identifier of its own, DXGI enumerates by
//! `(adapter_index, output_index)`. No mapping is exposed:
//! pairing is therefore done by dimensions and by elimination." The first
//! fact is exact, so is the second, **the conclusion is not** — and it is
//! from it that pairing by set difference came, which batch 30
//! measured refuses any window when the first virtual output
//! replaces a forced target.
//!
//! **What is true**: Win32's CCD API exposes the mapping. The
//! driver returns `(adapterId, target id)` — `sudovda::SortieAjoutee`, three
//! numbers of which the product kept only one —, and this pair turns into a GDI
//! name through `QueryDisplayConfig` then `DisplayConfigGetDeviceInfo`. See
//! `moniteurs_virtuels::config_affichage`, which does it, and
//! `superviseur::designation`, which uses it.
//!
//! **The commands establishing it, so that the next reader redoes the
//! check without believing anyone** — the first shows the three numbers
//! the driver returns, the second that the API exists in the pinned crate:
//!
//! ```text
//! grep -n 'identifiant_cible\|adaptateur_bas' agent/src/moniteurs_virtuels/sudovda.rs
//! grep -rn 'pub unsafe fn QueryDisplayConfig' \
//!   ~/.cargo/registry/src/*/windows-0.62.2/src/Windows/Win32/Devices/Display/mod.rs
//! ```
//!
//! ⚠️ **What the fix does NOT claim**: that the pair returned by
//! SudoVDA is the one CCD uses. It is a hypothesis, stated as
//! such in `config_affichage`, and its failure makes the product fall back on
//! the pairing by elimination described below — which therefore stays alive, and
//! stays the raison d'être of this whole module.
//!
//! **`GetDesc`/`DesktopCoordinates` is the source of truth, never WMI** —
//! the WMI field was seen 68 s stale on this ground, and the virtual output
//! was announced there as 5120×1440 while DXGI measured 3413×960 (DPI factor of
//! 1.5). A placement computed on the WMI value would be off by as much.

use crate::geometry::Rect;
// `crate::sortie_dxgi`, not `crate::capture`: `capture` is `#![cfg(windows)]`
// as a whole and does not exist at all when compiling on the Linux host
// — see the header comment of `sortie_dxgi.rs`. `capture.rs` re-exports this
// same type as `crate::capture::SortieDxgi` for Windows code.
use crate::sortie_dxgi::SortieDxgi;

/// Position and size tolerance, in pixels, before replacing.
///
/// DWM's invisible borders commonly shift `GetWindowRect` by
/// a few pixels relative to what `SetWindowPos` requested. Without
/// tolerance, the supervisor would replace the window at each loop turn.
///
/// **Four, and not two**: the retained value takes a deliberate margin
/// beyond the usual shift — a four-pixel gap on a full-frame
/// window is invisible, whereas a replacement loop is not. (The
/// comment said "one or two pixels" facing a constant of 4; it is the
/// text that lagged behind, the constant is the one we want.)
///
/// This same tolerance now also serves the pairing of a freshly
/// created output (`sortie_assez_grande`): it must be declared before
/// that function in the file.
const TOLERANCE_PX: i64 = 4;

/// True if an output can serve a given viewport.
///
/// **One inequality, plus one equality, and that is the whole of sub-block D10.** A
/// virtual output is NOT born at the requested size: it is born at the last
/// size left in the registry by an earlier `CDS_UPDATEREGISTRY` (D8,
/// task 3bis — confirmed, reproduced, never explained). On this VM the registry
/// stayed at 3840×2160, and equality within four pixels therefore refused
/// EVERY output: the product capped at three windows, in all six runs
/// of D9's acceptance run ③, without exception.
///
/// The product no longer writes to the registry since D9, but **nothing cleans what
/// is already written there** — and the scope of the blockage (per GUID or global) stays
/// unknown. Hence the choice to tolerate rather than clean: that way the
/// question becomes **moot**, not resolved.
///
/// The `TOLERANCE_PX` tolerance is kept in the SHORTFALL direction, for the
/// attach race recorded by acceptance run D1 (output created at 1280×713,
/// returned at 1280×720 one try out of two).
pub fn sortie_assez_grande(sortie: (u32, u32), demandee: (u32, u32)) -> bool {
    let assez = |s: u32, d: u32| s as i64 + TOLERANCE_PX >= d as i64;
    assez(sortie.0, demandee.0) && assez(sortie.1, demandee.1)
}

/// The size at which the window is put, and which the capture crops.
///
/// `min` axis by axis, **without preserving the aspect ratio**: we crop a
/// texture, we do not scale it. It is the opposite of
/// `windows_source_sortie::borner_a_la_taille_max`, which resizes and must
/// therefore preserve that ratio.
///
/// Even dimensions (the NV12 encoder requires them) and never zero (a collapsed
/// video box emits `(0, 0)`, a real case recorded in D8).
pub fn taille_retenue(demandee: (u32, u32), sortie: (u32, u32)) -> (u32, u32) {
    let retenir = |d: u32, s: u32| (d.min(s).max(2)) & !1;
    (retenir(demandee.0, sortie.0), retenir(demandee.1, sortie.1))
}

/// DXGI output able to serve a viewport, among those not
/// already assigned.
///
/// **`deja_prises` is what prevents the inequality from breaking everything.** With
/// the equality from before D10, two windows with the same viewport already contended for
/// one output; with "at least as large", a single large output
/// would suit ALL windows, and all would show the same image.
/// The filter designates by DXGI NAME (`\\.\DISPLAYn`), stable, and not by a
/// positional pair of enumeration indexes.
///
/// ⚠️ **The caller must look ONLY among the outputs that APPEARED** (see the
/// comment of `creation_sortie::creer_sortie`): the viewport announced by the
/// browser can equal the resolution of a PHYSICAL monitor, and the inequality
/// makes this risk larger, not smaller — a 4K monitor would now suit
/// any viewport.
///
/// 🔴 **`designee` EXEMPTS FROM THE SIZE CRITERION ALONE, AND IT IS INDEED A GUARD
/// BEING LOOSENED — said rather than disguised.** The header of
/// `superviseur::designation` writes that "designation narrows the set
/// of candidates, it loosens no guard": that sentence stops being
/// true here, and here is what refuted it.
///
/// **Measured on the production agent on August 31st, 2026**, eight times in a loop,
/// in the same run: `demande="1614x1080" designee="\\.\DISPLAY6"
/// candidates=["\\.\DISPLAY6 1428x1080"]`. The refused output was
/// **ours, named by CCD** — the SudoVDA driver does not create it at the
/// requested size, and the same request returned 1860×1080 at 12:14Z (served)
/// then 1428×1080 at 20:46Z (refused). Create → refuse → destroy, and **no
/// window showed any more, whatever the application**.
///
/// On a **designated** output, size is therefore no longer a refusal criterion
/// but a CONSTRAINT: `windows_source_sortie::taille_pour_viewport` already
/// fits the window there with the aspect ratio preserved (batch 33). Refusing meant
/// refusing the only output we could serve.
///
/// ⚠️ **WHAT IS NOT LOOSENED, and without which it would be a regression**:
/// `attachee_au_bureau` (an unattached output has nothing to duplicate) and
/// `deja_prises` — it is THAT, and not size, which prevents two windows from
/// showing the same image. The exemption is **by name**: it only applies
/// to the output designation named, never to its neighbours.
///
/// ⚠️ **`None` stays yesterday's product, line for line** — so the fallback by
/// set difference keeps refusing a too-small PHYSICAL screen, and
/// it is the exemption's negative witness.
pub fn sortie_pour_viewport(
    sorties: &[SortieDxgi],
    largeur: u32,
    hauteur: u32,
    deja_prises: &[String],
    designee: Option<&str>,
) -> Option<SortieDxgi> {
    sorties
        .iter()
        .find(|s| {
            let notre = designee == Some(s.nom_sortie.as_str());
            s.attachee_au_bureau
                && (notre || sortie_assez_grande((s.rect.width, s.rect.height), (largeur, hauteur)))
                && !deja_prises.contains(&s.nom_sortie)
        })
        .cloned()
}

/// DWM's **invisible fringe**: how much larger `GetWindowRect` is than
/// the window actually painted.
///
/// 🔴 **MEASURED ON THE PRODUCT, IN SESSION 1, ON AUGUST 31ST, 2026** — it is the
/// residue the owner still saw after the aspect fix
/// ("there is less border, but there still is some"):
///
/// ```text
/// GetWindowRect = 1732x1032+1280+0   <- exactly the RETAINED size
/// DWM frame     = 1718x1025+1287+0
/// fringe: left=7 top=0 right=7 bottom=7
/// ```
///
/// Since Windows 10, a window's resize borders are
/// **transparent**: `GetWindowRect` includes them, the eye does not see them.
/// Since `poser` placed in that space and the crop follows the placed
/// size, the image contained **7 px of desktop on the left, 7 on the right, 7 at the bottom**
/// — constant, **insensitive to the aspect ratio**, and therefore perfectly
/// distinct from the letterbox bands the aspect fix removed.
/// Two superposed defects, two different signatures; it is the contrast
/// "varies with the shape" / "constant" that told them apart.
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
    #[cfg(test)]
    pub const NUL: Lisere = Lisere {
        gauche: 0,
        haut: 0,
        droite: 0,
        bas: 0,
    };

    /// Vrai s'il n'y a rien à compenser : évite un second `SetWindowPos` et,
    /// surtout, rend la correction inerte là où elle n'a pas lieu d'être.
    #[cfg(test)]
    pub fn est_nul(self) -> bool {
        self == Lisere::NUL
    }
}

/// L'enveloppe totale à ajouter autour du recadrage : le lisère **invisible**
/// de DWM, **plus** la bordure que Windows **PEINT** autour de la fenêtre.
///
/// 🔴 **LA SECONDE MOITIÉ A ÉTÉ TROUVÉE EN REGARDANT L'IMAGE, ce que ce lot
/// n'avait jamais fait.** Capture de la sortie virtuelle en session 1, le
/// 31 août 2026, et relevé des couleurs sur les bords du recadrage
/// (1548×1032) :
///
/// ```text
/// rangee 0    (bord HAUT)   : #494949    | rangee 1    (voisine) : #F3F3F3
/// rangee 1031 (bord BAS)    : #2F2F2F    | rangee 1030 (voisine) : #F0F0F0
/// colonne 0   (bord GAUCHE) : #2F2F2F    | colonne 1   (voisine) : #FFFFFF
/// colonne 1547(bord DROIT)  : #2F2F2F    | colonne 1546(voisine) : #F0F0F0
/// ```
///
/// **Exactement UN pixel sombre sur les quatre bords, et la voisine immédiate
/// est claire.** `#2F2F2F` est la bordure de fenêtre de Windows en thème
/// sombre. En faisant coïncider le recadrage avec
/// `DWMWA_EXTENDED_FRAME_BOUNDS` **au pixel près**, le correctif précédent a
/// cadré pile dessus : ce n'est pas une erreur de calcul, c'est **la
/// définition du rectangle qu'on avait choisi pour cible**.
///
/// 🔴 **ET CETTE MESURE ÉLIMINE AUSSI LA PISTE DU CACHE NAVIGATEUR** : les
/// pixels sont lus **dans la VM**, sans navigateur d'aucune sorte. La bordure
/// est DANS l'image, quel que soit ce que la page affiche.
///
/// ⚠️ **`bordure` N'EST PAS UN NOMBRE ÉCRIT ICI** : elle vient de
/// `GetSystemMetrics(SM_CXBORDER/SM_CYBORDER)`, une métrique documentée qui
/// **suit le DPI** — relevée à `1` pour un DPI système de `96` sur cette
/// machine. Écrire `1` en dur serait le naufrage du 487.
pub fn enveloppe(dwm: Lisere, bordure: (i32, i32)) -> Lisere {
    Lisere {
        gauche: dwm.gauche + bordure.0,
        haut: dwm.haut + bordure.1,
        droite: dwm.droite + bordure.0,
        bas: dwm.bas + bordure.1,
    }
}

/// Le rectangle **du recadrage** correspondant à un cadre visible donné :
/// le cadre, **débarrassé de la bordure peinte**.
///
/// 🔴 **LE PENDANT EXACT DE `enveloppe`, ET IL DOIT LE RESTER.** `poser` pose
/// la fenêtre 1 px plus au large que le recadrage ; si `rectangle_de` rendait
/// le cadre visible **brut**, le contrôle périodique comparerait `crop + 1` à
/// `crop` et verrait un écart permanent. Il tomberait sous `TOLERANCE_PX`
/// aujourd'hui — mais s'appuyer là-dessus serait faire reposer une propriété
/// sur une tolérance faite pour autre chose (les arrondis de DWM). Les deux
/// fonctions se répondent, et le test d'aller-retour les tient ensemble.
pub fn sans_la_bordure(cadre: &Rect, bordure: (i32, i32)) -> Rect {
    Rect {
        x: cadre.x + bordure.0,
        y: cadre.y + bordure.1,
        width: cadre.width.saturating_add_signed(-2 * bordure.0),
        height: cadre.height.saturating_add_signed(-2 * bordure.1),
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
        width: cible
            .width
            .saturating_add_signed(lisere.gauche + lisere.droite),
        height: cible.height.saturating_add_signed(lisere.haut + lisere.bas),
    }
}

/// La taille à passer à `SetWindowPos` pour que le cadre **VISIBLE** mesure
/// `taille`. Le pendant de [`rect_a_poser`] pour le chemin du CAPTEUR, qui
/// retaille sans déplacer (`SWP_NOMOVE`) — l'origine étant déjà compensée par
/// la pose du superviseur, seule la taille reste à corriger.
pub fn taille_a_poser(taille: (u32, u32), lisere: Lisere) -> (u32, u32) {
    (
        taille
            .0
            .saturating_add_signed(lisere.gauche + lisere.droite),
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
            // L'enveloppe TOTALE : le lisère invisible de DWM, plus la
            // bordure que Windows peint (mesurée à 1 px sur les quatre bords
            // du recadrage — voir `enveloppe`). La fenêtre est donc posée
            // légèrement PLUS AU LARGE que le recadrage, et sa bordure tombe
            // hors de l'image.
            let enveloppe_totale = enveloppe(
                crate::window::lisere_dwm(hwnd).unwrap_or_default(),
                crate::window::bordure_peinte(),
            );
            let pose = rect_a_poser(cible, enveloppe_totale);
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
            // Le cadre visible INCLUT la bordure peinte ; le recadrage, lui,
            // s'arrête juste en dedans. On rend donc ce à quoi la cible est
            // comparable — voir `sans_la_bordure`.
            Ok(visible) => sans_la_bordure(&visible, crate::window::bordure_peinte()),
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

// Le lisère de DWM et la bordure peinte ont leurs propres cas, sortis de
// `placement/tests.rs` le 31 août 2026 : ce fichier-là était à 485 lignes,
// donc à sa porte, et le correctif de la sortie DÉSIGNÉE devait y écrire.
// Extraction JOUÉE AVANT l'addition qu'elle préparait, et dans sa propre
// tâche — la forme forte que ce dépôt s'impose après l'avoir manquée six fois.
#[cfg(test)]
#[path = "placement/tests_lisere.rs"]
mod tests_lisere;
