//! Repérage et pilotage de la fenêtre à capturer.

#![cfg(windows)]

use anyhow::{anyhow, bail, Context, Result};
// écart d'API windows-rs 0.62 : `BOOL` a été déplacé dans `windows::core`
// (il n'est plus réexporté sous `Win32::Foundation`), contrairement à `TRUE`
// qui y reste accessible.
use windows::core::BOOL;
// écart d'API windows-rs 0.62 : `ClientToScreen` vit dans `Win32::Graphics::Gdi`
// (module gdi32), pas dans `WindowsAndMessaging` (user32) où on l'attendrait
// par analogie avec `GetClientRect`. Elle renvoie en outre un `BOOL` brut
// (convention historique de gdi32), pas un `windows::core::Result<()>` comme
// les fonctions user32 annotées succès/échec du même fichier.
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsWindow,
    IsWindowVisible,
    SetWindowPos, SWP_NOMOVE, SWP_NOZORDER,
};

use crate::geometry::Rect;

struct SearchContext {
    fragment: String,
    found: Option<HWND>,
}

/// Cherche la première fenêtre visible dont le titre contient `fragment`.
///
/// La comparaison est insensible à la casse : les titres de navigateurs
/// changent au gré de la page affichée, on ne peut pas exiger un titre exact.
pub fn find_window_by_title(fragment: &str) -> Result<HWND> {
    let mut context = SearchContext {
        fragment: fragment.to_lowercase(),
        found: None,
    };

    unsafe {
        // EnumWindows renvoie une erreur si le rappel interrompt l'énumération,
        // ce qui est précisément ce que nous faisons en cas de succès.
        let _ = EnumWindows(
            Some(enum_callback),
            LPARAM(&mut context as *mut SearchContext as isize),
        );
    }

    context
        .found
        .ok_or_else(|| anyhow!("aucune fenêtre visible dont le titre contient « {fragment} »"))
}

unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam.0 as *mut SearchContext);

    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }
    let length = GetWindowTextLengthW(hwnd);
    if length <= 0 {
        return TRUE;
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let written = GetWindowTextW(hwnd, &mut buffer);
    if written <= 0 {
        return TRUE;
    }
    let title = String::from_utf16_lossy(&buffer[..written as usize]).to_lowercase();

    if title.contains(&context.fragment) {
        context.found = Some(hwnd);
        return BOOL(0); // interrompt l'énumération
    }
    TRUE
}

/// Dimensions de la zone client de la fenêtre, en pixels.
pub fn client_size(hwnd: HWND) -> Result<(u32, u32)> {
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect)? };
    let width = (rect.right - rect.left).max(0) as u32;
    let height = (rect.bottom - rect.top).max(0) as u32;
    if width == 0 || height == 0 {
        bail!("la fenêtre a une zone client vide");
    }
    Ok((width, height))
}

/// Zone client de la fenêtre, convertie en coordonnées écran.
///
/// Nécessaire au recadrage : Desktop Duplication renvoie une image de tout
/// l'écran, exprimée en coordonnées écran, alors que `GetClientRect` renvoie
/// un rectangle en coordonnées client (origine toujours à (0, 0)).
pub fn client_rect_on_screen(hwnd: HWND) -> Result<Rect> {
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect)? };
    let width = (rect.right - rect.left).max(0) as u32;
    let height = (rect.bottom - rect.top).max(0) as u32;
    if width == 0 || height == 0 {
        bail!("la fenêtre a une zone client vide");
    }

    // L'origine de la zone client (0, 0) suffit : GetClientRect garantit que
    // `left`/`top` valent toujours 0, donc ce point représente le coin
    // supérieur gauche de la zone client dans son propre repère.
    let mut origin = POINT { x: 0, y: 0 };
    unsafe { ClientToScreen(hwnd, &mut origin) }
        .ok()
        .context("ClientToScreen")?;

    Ok(Rect { x: origin.x, y: origin.y, width, height })
}

/// Le rectangle de la fenêtre tel que `GetWindowRect` le rend — **bordures
/// invisibles de DWM COMPRISES**.
///
/// ⚠️ **Ce n'est PAS le rectangle qu'on voit** : voir
/// `superviseur::placement::Lisere`. Cette fonction est le brut ; le cadre
/// visible est [`cadre_visible`].
pub fn rectangle_brut(hwnd: HWND) -> Result<Rect> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut r) }.context("GetWindowRect")?;
    Ok(depuis_rect(r))
}

/// Le cadre **VISIBLE** de la fenêtre, par `DWMWA_EXTENDED_FRAME_BOUNDS`.
///
/// 🔴 **C'EST LE SEUL RECTANGLE QUI CORRESPONDE À CE QUE L'ŒIL VOIT.** Depuis
/// Windows 10 les bordures de redimensionnement sont transparentes et
/// `GetWindowRect` les inclut : mesuré en session 1 le 31 août 2026, une
/// fenêtre servie rendait `1732x1032+1280+0` au brut et `1718x1025+1287+0`
/// au cadre visible — 7 px à gauche, à droite et en bas, 0 en haut.
///
/// `Err` quand DWM refuse (composition désactivée, fenêtre détruite) :
/// l'appelant retombe alors sur le brut, c'est-à-dire sur le comportement
/// d'avant ce correctif.
pub fn cadre_visible(hwnd: HWND) -> Result<Rect> {
    let mut r = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut RECT as *mut core::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        )
    }
    .context("DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)")?;
    Ok(depuis_rect(r))
}

/// Le lisère invisible de CETTE fenêtre : `GetWindowRect` moins le cadre
/// visible, côté par côté.
///
/// ⚠️ **À relire APRÈS `ShowWindow`** : sur une fenêtre minimisée, DWM rend un
/// cadre qui ne veut rien dire (rectangle à `-32000`).
pub fn lisere_dwm(hwnd: HWND) -> Result<crate::superviseur::placement::Lisere> {
    let brut = rectangle_brut(hwnd)?;
    let vu = cadre_visible(hwnd)?;
    Ok(crate::superviseur::placement::Lisere {
        gauche: vu.x - brut.x,
        haut: vu.y - brut.y,
        droite: (brut.x + brut.width as i32) - (vu.x + vu.width as i32),
        bas: (brut.y + brut.height as i32) - (vu.y + vu.height as i32),
    })
}

/// Le rectangle du moniteur et sa **zone de travail**, pour le moniteur qui
/// contient un point du bureau virtuel.
///
/// 🔴 **PREMIER LECTEUR DE `rcWork` DE TOUT LE DÉPÔT** (lot 33) : avant lui,
/// `grep -rni 'rcWork\|SPI_GETWORKAREA\|zone_de_travail'` sur `agent/`,
/// `client/`, `plateforme/` et `proto/` rendait **zéro**, et la distinction
/// moniteur / zone de travail n'existait donc nulle part dans ce produit.
/// C'est ce qui mettait les 48 rangées de la barre des tâches secondaire dans
/// le recadrage de chaque fenêtre servie — voir
/// `windows_source_sortie::borne_de_la_sortie`, qui porte la mesure.
///
/// **Deux points d'entrée, un seul corps, et c'est délibéré** : le SUPERVISEUR
/// interroge par l'ORIGINE de la sortie (il connaît le rectangle DXGI avant
/// même d'avoir posé la fenêtre), le CAPTEUR par SA FENÊTRE (il ne connaît que
/// le nom de la sortie, mais sa fenêtre est posée dessus). Les deux tombent
/// sur le même `HMONITOR`, donc sur la même réponse — c'est ce qui permet aux
/// deux processus de calculer la même borne sans échanger un message.
pub fn zones_du_moniteur_au_point(x: i32, y: i32) -> Result<(Rect, Rect)> {
    // `MONITOR_DEFAULTTONEAREST` : un point hors de tout moniteur — une sortie
    // que Windows vient de détacher — rendrait `NULL` avec
    // `MONITOR_DEFAULTTONULL`, et l'appelant retomberait sur le rectangle de
    // la sortie. Le plus proche est une réponse, pas une devinette : le point
    // vient de l'origine d'une sortie que DXGI énumère encore.
    let moniteur = unsafe {
        MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST)
    };
    zones_de_l_hmoniteur(moniteur)
}

/// Le rectangle du moniteur et sa zone de travail, pour le moniteur qui porte
/// une fenêtre. Voir [`zones_du_moniteur_au_point`] pour le pourquoi des deux
/// points d'entrée.
pub fn zones_du_moniteur_de(hwnd: HWND) -> Result<(Rect, Rect)> {
    let moniteur = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    zones_de_l_hmoniteur(moniteur)
}

fn zones_de_l_hmoniteur(moniteur: HMONITOR) -> Result<(Rect, Rect)> {
    if moniteur.is_invalid() {
        bail!("aucun moniteur pour ce repère");
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // `GetMonitorInfoW` rend un `BOOL` brut : un `false` n'est pas une
    // `windows::core::Error`, et un `?` ne l'attraperait pas.
    if !unsafe { GetMonitorInfoW(moniteur, &mut info) }.as_bool() {
        bail!("GetMonitorInfoW a refusé");
    }
    Ok((depuis_rect(info.rcMonitor), depuis_rect(info.rcWork)))
}

fn depuis_rect(r: RECT) -> Rect {
    Rect {
        x: r.left,
        y: r.top,
        width: (r.right - r.left).max(0) as u32,
        height: (r.bottom - r.top).max(0) as u32,
    }
}

/// Redimensionne la fenêtre sans la déplacer ni changer son ordre d'affichage.
pub fn resize_window(hwnd: HWND, width: u32, height: u32) -> Result<()> {
    // Les dimensions nulles font échouer la capture ; on impose un plancher.
    let width = width.max(160) as i32;
    let height = height.max(120) as i32;
    unsafe { SetWindowPos(hwnd, None, 0, 0, width, height, SWP_NOMOVE | SWP_NOZORDER)? };
    Ok(())
}

/// Vrai tant que la fenêtre existe.
pub fn is_window_alive(hwnd: HWND) -> bool {
    unsafe { IsWindow(Some(hwnd)).as_bool() }
}
