//! La lecture de l'icône d'une fenêtre, par `hwnd` — **la moitié Windows de A1**.
//!
//! 🔴 **AUCUNE DÉCISION NE VIT ICI.** Ce module rend des octets RGBA et une
//! géométrie ; les seuils, les filtres et le format de sortie appartiennent à
//! `accent.rs`, qui est **pur** et testé sur l'hôte. C'est la même séparation
//! que `presse_papier/win32.rs` (les deux appels Win32, aucune décision).
//!
//! 🔴 **AUCUN TEST D'HÔTE NE COUVRE CE FICHIER**, et c'est déclaré plutôt que
//! tu : il est `#[cfg(windows)]`, et `cargo check --target
//! x86_64-pc-windows-gnu` vérifie **types, emprunts, visibilités et durées de
//! vie — jamais le comportement**. Sa seule preuve de fonctionnement est la
//! recette. C'est le patron d'`apps/icone/extraction.rs`, qui le déclare de
//! lui-même.
//!
//! **Aucune dépendance ni feature Cargo neuve** : `Win32_UI_WindowsAndMessaging`
//! (`agent/Cargo.toml:157`) porte `SendMessageTimeoutW`, `WM_GETICON`,
//! `GetClassLongPtrW` et `GetIconInfo` ; `Win32_Graphics_Gdi` (`:60`) porte
//! `GetDIBits`, `GetObjectW` et `DeleteObject`. **Relu, pas supposé.**

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassLongPtrW, GetIconInfo, SendMessageTimeoutW, GCLP_HICON, GCLP_HICONSM, HICON, ICONINFO,
    ICON_BIG, ICON_SMALL, ICON_SMALL2, SMTO_ABORTIFHUNG,
};

/// Le délai laissé à `WM_GETICON`, en millisecondes.
///
/// 🔴 **`WM_GETICON` est un `SendMessage` SYNCHRONE VERS UNE AUTRE
/// APPLICATION** (R4 de la spec, et il n'est pas théorique) : une application
/// figée ne répond pas, et l'appel **bloquerait indéfiniment le fil qui l'a
/// émis**. D'où `SMTO_ABORTIFHUNG` **et** ce délai.
///
/// ⚠️ **La gravité est moindre que ce que la spec redoutait, et elle ne
/// disparaît pas.** Sous D-A1-1 la lecture vit sur le **fil de fenêtre**, pas
/// sur le tour de roue : un blocage ne gèle **que cette fenêtre**. Mais ce fil
/// est celui qui produit ses images — la session se figerait, exactement comme
/// le pont ProjFS s'est figé neuf minutes en F1.
///
/// ⚠️ **NON CALIBRÉ**, et **JAMAIS EXERCÉ** : `SMTO_ABORTIFHUNG` est **posé**,
/// aucune application figée n'est provoquée par la recette de A1.
const DELAI_MS: u32 = 200;

/// Rend l'icône de `hwnd` en **RGBA**, avec sa largeur et sa hauteur.
///
/// La cascade, dans l'ordre décroissant de taille (D-A1-8) :
/// 1. `WM_GETICON` en `ICON_BIG`, puis `ICON_SMALL2`, puis `ICON_SMALL`, tous
///    trois par `SendMessageTimeoutW` + `SMTO_ABORTIFHUNG` ;
/// 2. repli **non bloquant** `GetClassLongPtrW(GCLP_HICON)` puis `GCLP_HICONSM`.
///
/// 🔴 **Un délai dépassé, un `HICON` nul, ou un échec de `GetIconInfo` /
/// `GetDIBits` se traitent comme « PAS D'ICÔNE » : `None`. Le tour ne produit
/// aucune annonce, et il ne produit pas d'erreur non plus.** Une icône qu'on ne
/// sait pas lire n'est pas une panne du produit.
///
/// ⚠️ **Le repli `GetClassLongPtrW` ne courra que si `WM_GETICON` échoue, ce
/// qu'aucun protocole de recette ne provoque : code livré, chemin probablement
/// jamais emprunté** — comme `borner_a_la_taille_max` l'a été un sous-bloc
/// entier.
pub fn lire_icone(hwnd: HWND) -> Option<(Vec<u8>, u32, u32)> {
    let icone = trouver_icone(hwnd)?;
    pixels_de(icone)
}

/// La cascade de la doc de [`lire_icone`], et rien d'autre.
fn trouver_icone(hwnd: HWND) -> Option<HICON> {
    for taille in [ICON_BIG, ICON_SMALL2, ICON_SMALL] {
        let mut resultat: usize = 0;
        // `SendMessageTimeoutW` rend 0 sur expiration comme sur échec : les
        // deux se traitent pareil — « pas d'icône par cette voie ».
        let rendu = unsafe {
            SendMessageTimeoutW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::WM_GETICON,
                WPARAM(taille as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                DELAI_MS,
                Some(&mut resultat as *mut usize),
            )
        };
        if rendu.0 != 0 && resultat != 0 {
            return Some(HICON(resultat as *mut _));
        }
    }
    // Repli NON BLOQUANT : la classe de fenêtre, qui est une donnée locale au
    // processus appelant et ne demande rien à l'application cible.
    for index in [GCLP_HICON, GCLP_HICONSM] {
        let brut = unsafe { GetClassLongPtrW(hwnd, index) };
        if brut != 0 {
            return Some(HICON(brut as *mut _));
        }
    }
    None
}

/// Décode un `HICON` en RGBA.
///
/// 🔴 **`GetDIBits` REND DU BGRA. LA CONVERSION SE FAIT ICI, ET NULLE PART
/// AILLEURS.** Se tromper de sens échangerait le rouge et le bleu — un défaut
/// **plausible et silencieux**, qu'**aucun test d'hôte ne verrait** puisqu'il
/// vivrait derrière ce `#[cfg(windows)]` (RA1-6). Le module pur reçoit du RGBA,
/// et son test le dit ; **le seul contrôle réel est le critère ① de la
/// recette**, et il ne le verrait que si les deux applications choisies ont des
/// icônes de teintes opposées.
fn pixels_de(icone: HICON) -> Option<(Vec<u8>, u32, u32)> {
    let mut info = ICONINFO::default();
    if unsafe { GetIconInfo(icone, &mut info) }.is_err() {
        return None;
    }
    // 🔴 `DeleteObject` SUR TOUS LES CHEMINS DE SORTIE, Y COMPRIS D'ERREUR :
    // `GetIconInfo` crée DEUX bitmaps dont l'appelant devient propriétaire, et
    // les oublier est une fuite par tour de lecture — soit une fuite toutes les
    // `PERIODE_ACCENT`, pour toute la vie de la session. Le patron est
    // `apps/icone/extraction.rs`.
    let resultat = decoder(info.hbmColor);
    unsafe {
        if !info.hbmColor.is_invalid() {
            let _ = DeleteObject(info.hbmColor.into());
        }
        if !info.hbmMask.is_invalid() {
            let _ = DeleteObject(info.hbmMask.into());
        }
    }
    resultat
}

/// La lecture des octets d'un `HBITMAP` 32 bits, top-down.
///
/// **Séparée pour que le `DeleteObject` de l'appelant coure sur TOUS les
/// chemins**, y compris ceux qui rendent `None` — c'est la raison d'être de la
/// même séparation dans `apps/icone/extraction.rs`.
fn decoder(bitmap: HBITMAP) -> Option<(Vec<u8>, u32, u32)> {
    if bitmap.is_invalid() {
        // Une icône monochrome n'a pas de plan couleur : elle n'a pas de
        // teinte à donner, et ce n'est pas une erreur.
        return None;
    }
    let mut brut = BITMAP::default();
    let lu = unsafe {
        GetObjectW(
            bitmap.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut brut as *mut BITMAP as *mut _),
        )
    };
    if lu == 0 || brut.bmWidth <= 0 || brut.bmHeight <= 0 {
        return None;
    }
    let (largeur, hauteur) = (brut.bmWidth as u32, brut.bmHeight as u32);
    let octets = (largeur as usize).checked_mul(hauteur as usize)?.checked_mul(4)?;
    let mut tampon = vec![0u8; octets];

    let mut entete = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: largeur as i32,
            // Négatif : bitmap TOP-DOWN. Sans cela les lignes reviendraient
            // dans l'ordre inverse — sans conséquence pour une couleur
            // dominante, qui ne dépend pas de l'ordre, mais le dire évite qu'un
            // successeur qui réemploierait ce module pour rendre une IMAGE
            // hérite d'une image retournée. Le modèle est
            // `diagnostics/multifenetre/voies.rs`.
            biHeight: -(hauteur as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let ecran = unsafe { GetDC(None) };
    let lignes = unsafe {
        GetDIBits(
            ecran,
            bitmap,
            0,
            hauteur,
            Some(tampon.as_mut_ptr() as *mut _),
            &mut entete,
            DIB_RGB_COLORS,
        )
    };
    unsafe { ReleaseDC(None, ecran) };
    if lignes == 0 {
        return None;
    }

    // 🔴 BGRA -> RGBA, PAR LA RÈGLE PURE ET NON PAR UNE COPIE. Cette boucle
    // vivait ici, derrière le `#[cfg(windows)]`, et n'était couverte par rien
    // (legs RA1-6). Le sous-bloc G5 en a eu besoin une seconde fois, pour
    // l'icône d'une APPLICATION : elle est descendue dans `accent.rs`, où elle
    // est testée, plutôt que d'être recopiée — deux copies d'une règle que
    // personne ne vérifie divergeraient sans que rien ne le dise.
    crate::accent::bgra_en_rgba(&mut tampon);
    Some((tampon, largeur, hauteur))
}
