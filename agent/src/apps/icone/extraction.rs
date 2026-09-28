//! Extracting the icon through the Shell, and its PNG encoding through WIC.
//!
//! 🔴 THE ENCODING GOES THROUGH WIC, AND CERTAINLY NOT THROUGH A CONVERSION THAT LOSES
//! ALPHA. Specification §3.1 notes that its own cost probe went
//! through `Image::FromHbitmap`, **which loses the alpha channel** — and that is
//! precisely what acceptance criterion ① exists to catch. The line
//! that decides is a NAMED CONSTANT, `WICBitmapUseAlpha`, which makes its
//! mutation trivial to play: `WICBitmapIgnoreAlpha` brings the count of
//! icons with non-trivial alpha down to ZERO.
//!
//! ⚠️ `SIIGBF_SCALEUP` IS NOT USED, AND NEITHER IS `SIIGBF_BIGGERSIZEOK`.
//! Neither changes anything — measured on 20 August 2026: the
//! witness that contains ONLY 48×48 renders 256×256 32bpp in both cases —
//! and using them **would wrongly suggest that the flag protects against
//! upscaling**. The only flag is `SIIGBF_ICONONLY`.
//!
//! ⚠️ IT IS ONLY CHECKED BY `cargo check --target x86_64-pc-windows-gnu`:
//! types, borrows, visibility, lifetimes — **and NOT linking**,
//! the real target being `msvc`. **No host test can cover this
//! module**, and its only proof of working is the acceptance run.

use std::path::Path;
use std::sync::OnceLock;

use anyhow::{bail, Context, Result};
use proto::plateforme::SourceMax;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, IWICImagingFactory, WICBitmapUseAlpha,
};
use windows::Win32::System::Com::{
    CoCreateInstance, IStream, StructuredStorage::CreateStreamOnHGlobal, CLSCTX_INPROC_SERVER,
    STREAM_SEEK_SET,
};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY,
};

use super::super::lecture::vers_utf16;
use super::{ressource, source};

/// The side of the requested image. **It is NOT a proof of provenance**:
/// the Shell renders this size whatever happens.
const COTE: i32 = 256;

/// 🔴 `ICONES=0` DISARMS; MERE PRESENCE DOES NOT ENABLE.
///
/// Convention of `APPS`, `PLEIN_ECRAN`, `AUDIO` and `PART_SONDAGE`. Testing
/// `is_ok()` WOULD ENABLE the mechanism when `ICONES=0` is written **to
/// turn it off** — the two errors cancel out so well that nobody would
/// see them.
///
/// ⚠️ THE PREDICATE ITSELF IS REUSED, NOT COPIED: `apps::desarme` is
/// pure, tested, and carries its own named red. Two identical predicates
/// would diverge the day one of them accepted `"false"`.
///
/// Three reasons to exist, and they are not decorative: extraction
/// is the first thing in this project that makes a reconciliation last
/// SECONDS (2,298 ms for 153 icons on the first tick, measured); it gives
/// criterion ③ a witness that requires NO rebuild of the binary; and it makes
/// the disarm observable.
pub fn armee() -> bool {
    static ARMEE: OnceLock<bool> = OnceLock::new();
    *ARMEE.get_or_init(|| {
        let armee = !crate::apps::desarme(std::env::var("ICONES").ok().as_deref());
        if !armee {
            tracing::warn!(
                "extraction d'icones DESARMEE (ICONES=0) : le catalogue reste \
                 complet, mais aucune application ne portera d'icone"
            );
        }
        armee
    })
}

/// The icon of a shortcut, as a 256×256 PNG with its alpha channel.
///
/// 🔴 ON THE `.lnk` ITSELF, AND NOT ON THE TARGET. **92 of the 153 retained
/// shortcuts of this VM carry an `IconLocation` without a path**: their icon
/// is the target's, and the others carry an icon OF THEIR OWN.
/// Only the Shell knows this whole chain, and that is why it is asked
/// rather than rebuilt.
/// Returns the PNG **and** the dominant colour of the icon (`#rrggbb`), the
/// latter possibly being `None` — see [`dominante_du_bitmap`].
pub fn extraire(lnk: &Path) -> Result<(Vec<u8>, Option<String>)> {
    let large = vers_utf16(&lnk.to_string_lossy());
    // SAFETY: FFI call. The path is a null-terminated UTF-16 buffer
    // that we own for the whole duration of the call.
    let fabrique: IShellItemImageFactory =
        unsafe { SHCreateItemFromParsingName(windows::core::PCWSTR(large.as_ptr()), None) }
            .with_context(|| format!("SHCreateItemFromParsingName sur {}", lnk.display()))?;

    // SAFETY: FFI call. `SIIGBF_ICONONLY` alone — see the module header.
    let hbm = unsafe { fabrique.GetImage(SIZE { cx: COTE, cy: COTE }, SIIGBF_ICONONLY) }
        .with_context(|| format!("GetImage 256 sur {}", lnk.display()))?;

    // 🔴 THE PIXELS ARE READ BEFORE ENCODING, AND ON THE SAME HBITMAP.
    // Recomputing the accent from the PNG would require a decoder; the bitmap
    // is already there, and it carries exactly what `accent::dominante` needs.
    let accent = dominante_du_bitmap(hbm);
    let png = encoder_png(hbm);

    // 🔴 `DeleteObject` ON EVERY EXIT PATH, ERROR PATHS INCLUDED.
    // An HBITMAP leak here would cost 256×256×4 bytes per icon and per
    // tick — 40 MB per reconciliation on this corpus, every thirty
    // seconds.
    // SAFETY: FFI call. The bitmap comes from `GetImage` and is only released
    // here; `encoder_png` does not take ownership of it.
    let _ = unsafe { DeleteObject(hbm.into()) };
    png.map(|p| (p, accent))
}

/// The DOMINANT colour of an icon bitmap, as `#rrggbb`, or `None`.
///
/// 🔴 IT DECIDES NOTHING: it reads pixels and delegates. The BGRA → RGBA
/// conversion and the choice of the dominant colour are two PURE and TESTED rules
/// (`crate::accent`), and **they are not copied here** — sub-project
/// ① carried the first behind its own `#[cfg(windows)]` without any
/// test (legacy RA1-6), and writing a second copy would have duplicated a rule
/// that nobody checked.
///
/// ⚠️ `None` IS NOT AN ERROR: an icon that is too pale, too dark or too
/// transparent has no dominant colour — that is clause 5 of `dominante`. The
/// manifest then OMITS `theme_color` rather than inventing one.
///
/// ⚠️ A READ FAILURE IS TREATED AS NO ACCENT, AND NOT AS
/// AN EXTRACTION FAILURE: the icon itself was obtained. Failing
/// the extraction for an unreadable accent would lose a perfectly
/// good image — "an application without an icon is better than a missing
/// application", and all the more so without an accent.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn dominante_du_bitmap(hbm: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<String> {
    let largeur = COTE;
    let hauteur = COTE;
    let mut entete = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: largeur,
            // ⚠️ NEGATIVE HEIGHT: the bitmap is rendered TOP TO BOTTOM. The
            // direction changes nothing for a dominant colour — it is computed on a
            // multiset of pixels —, but leaving it positive would give
            // upside-down rows to anyone who reused this read.
            biHeight: -hauteur,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut tampon = vec![0u8; (largeur as usize) * (hauteur as usize) * 4];
    // SAFETY: FFI call. The buffer is sized by the header above,
    // and the screen DC is released on every path.
    let ecran = unsafe { GetDC(None) };
    let lignes = unsafe {
        GetDIBits(
            ecran,
            hbm,
            0,
            hauteur as u32,
            Some(tampon.as_mut_ptr().cast()),
            &mut entete,
            DIB_RGB_COLORS,
        )
    };
    unsafe { ReleaseDC(None, ecran) };
    if lignes == 0 {
        return None;
    }
    crate::accent::bgra_en_rgba(&mut tampon);
    crate::accent::dominante(&tampon, largeur as u32, hauteur as u32).map(crate::accent::en_hexa)
}

/// The encoding itself. **Split out so that the caller's `DeleteObject`
/// runs whatever happens.**
fn encoder_png(hbm: windows::Win32::Graphics::Gdi::HBITMAP) -> Result<Vec<u8>> {
    // SAFETY: FFI call. The COM apartment is the apps thread's, opened
    // once by `lecture::initialiser_com`.
    let fabrique: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
            .context("CoCreateInstance(WICImagingFactory)")?;

    // 🔴 THE LINE THAT DECIDES CRITERION ①. `WICBitmapUseAlpha` keeps the
    // channel; `WICBitmapIgnoreAlpha` throws it away, and that is the mutation that plays
    // the red.
    // SAFETY: FFI call. The bitmap belongs to the caller and outlives
    // this call — WIC makes its own copy.
    let bitmap = unsafe {
        fabrique.CreateBitmapFromHBITMAP(
            hbm,
            windows::Win32::Graphics::Gdi::HPALETTE(std::ptr::null_mut()),
            WICBitmapUseAlpha,
        )
    }
    .context("CreateBitmapFromHBITMAP")?;

    // SAFETY: FFI call. A stream over an HGLOBAL, which WIC takes charge of.
    let flux: IStream = unsafe {
        CreateStreamOnHGlobal(
            windows::Win32::Foundation::HGLOBAL(std::ptr::null_mut()),
            true,
        )
    }
    .context("CreateStreamOnHGlobal")?;

    // SAFETY: FFI call.
    let encodeur = unsafe { fabrique.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null()) }
        .context("CreateEncoder(PNG)")?;
    // SAFETY: FFI call.
    unsafe {
        encodeur.Initialize(
            &flux,
            windows::Win32::Graphics::Imaging::WICBitmapEncoderNoCache,
        )
    }
    .context("Initialize de l'encodeur PNG")?;

    let mut cadre = None;
    // SAFETY: FFI call. `cadre` is filled by the call.
    unsafe { encodeur.CreateNewFrame(&mut cadre, std::ptr::null_mut()) }
        .context("CreateNewFrame")?;
    let cadre = cadre.context("l'encodeur n'a rendu aucun cadre")?;
    // SAFETY: FFI call.
    unsafe { cadre.Initialize(None) }.context("Initialize du cadre")?;
    // SAFETY: FFI call.
    unsafe { cadre.WriteSource(&bitmap, std::ptr::null()) }.context("WriteSource")?;
    // SAFETY: FFI call.
    unsafe { cadre.Commit() }.context("Commit du cadre")?;
    // SAFETY: FFI call.
    unsafe { encodeur.Commit() }.context("Commit de l'encodeur")?;

    relire(&flux)
}

/// Re-reads the stream from the start, and returns its bytes.
fn relire(flux: &IStream) -> Result<Vec<u8>> {
    // SAFETY: FFI call. Seek back to the start: `Commit` leaves the cursor
    // at the end, and reading from there would return ZERO bytes — an empty PNG that would
    // look like a success.
    unsafe { flux.Seek(0, STREAM_SEEK_SET, None) }.context("Seek au début du flux PNG")?;
    let stat = {
        let mut s = Default::default();
        // SAFETY: FFI call.
        unsafe { flux.Stat(&mut s, windows::Win32::System::Com::STATFLAG_NONAME) }
            .context("Stat du flux PNG")?;
        s
    };
    let taille = stat.cbSize as usize;
    if taille == 0 {
        bail!("l'encodeur PNG a rendu un flux VIDE");
    }
    let mut octets = vec![0u8; taille];
    let mut lus = 0u32;
    // SAFETY: FFI call. The buffer is exactly `taille` bytes.
    unsafe { flux.Read(octets.as_mut_ptr().cast(), taille as u32, Some(&mut lus)) }
        .ok()
        .context("Read du flux PNG")?;
    if lus as usize != taille {
        bail!("flux PNG tronqué : {lus} octets lus sur {taille}");
    }
    Ok(octets)
}

/// The PROVENANCE of the image — the largest entry actually PRESENT in
/// the icon directory of the source.
///
/// 🔴 IT NEVER COMES FROM THE PNG. Code that deduced it from the rendered
/// size would give `256` to EVERYTHING, including an `.ico` that only contains
/// 48×48 — it is measured, twice, and it is the whole purpose of the sub-block.
///
/// ⚠️ IT RETURNS `NonMesuree` WITHOUT ERROR when the source is neither a PE module
/// nor a readable `.ico`. **Measured: 37 of the 153 applications of this VM.** It
/// is not a failure, and logging it as one would drown the real signal.
pub fn provenance_de(icon_location: &str, cible: &str) -> SourceMax {
    match source::provenance(icon_location, cible) {
        source::Provenance::Ico(chemin) => match std::fs::read(&chemin) {
            // ⚠️ ONLY THE FIRST BYTES ARE NEEDED, but an `.ico`
            // weighs a few dozen kilobytes: reading it whole costs
            // less than a partial open, and it is simpler to re-read.
            Ok(octets) => {
                ressource::maximum(&ressource::tailles_icondir(&octets).unwrap_or_default())
            }
            Err(erreur) => {
                tracing::debug!(chemin, %erreur, "ico illisible, provenance non mesuree");
                SourceMax::NonMesuree
            }
        },
        source::Provenance::Module(chemin) => {
            let index = source::index(icon_location);
            match lecture_pe_grpicondir(&chemin, index) {
                Ok(octets) => {
                    ressource::maximum(&ressource::tailles_grpicondir(&octets).unwrap_or_default())
                }
                Err(erreur) => {
                    tracing::debug!(chemin, %erreur, "ressource illisible, provenance non mesuree");
                    SourceMax::NonMesuree
                }
            }
        }
        source::Provenance::Aucune => SourceMax::NonMesuree,
    }
}

fn lecture_pe_grpicondir(chemin: &str, index: i32) -> Result<Vec<u8>> {
    super::lecture_pe::grpicondir(Path::new(chemin), index)
}
