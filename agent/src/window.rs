//! Repérage et pilotage de la fenêtre à capturer.

#![cfg(windows)]

use anyhow::{anyhow, bail, Result};
// écart d'API windows-rs 0.62 : `BOOL` a été déplacé dans `windows::core`
// (il n'est plus réexporté sous `Win32::Foundation`), contrairement à `TRUE`
// qui y reste accessible.
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, TRUE};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetWindowTextLengthW, GetWindowTextW, IsWindow, IsWindowVisible,
    SetWindowPos, SWP_NOMOVE, SWP_NOZORDER,
};

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
