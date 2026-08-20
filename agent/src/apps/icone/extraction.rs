//! L'extraction de l'icône par le Shell, et son encodage PNG par WIC.
//!
//! 🔴 L'ENCODAGE PASSE PAR WIC, ET SÛREMENT PAS PAR UNE CONVERSION QUI PERD
//! L'ALPHA. La spécification §3.1 relève que sa propre sonde de coût passait
//! par `Image::FromHbitmap`, **qui perd le canal alpha** — et c'est
//! précisément ce que le critère ① de recette existe pour attraper. La ligne
//! qui décide est une CONSTANTE NOMMÉE, `WICBitmapUseAlpha`, ce qui rend sa
//! mutation triviale à jouer : `WICBitmapIgnoreAlpha` fait tomber à ZÉRO le
//! compte d'icônes à alpha non trivial.
//!
//! ⚠️ `SIIGBF_SCALEUP` N'EST PAS EMPLOYÉ, ET `SIIGBF_BIGGERSIZEOK` NON PLUS.
//! Ni l'un ni l'autre ne change quoi que ce soit — mesuré le 20 août 2026 : le
//! témoin qui ne contient QUE du 48×48 rend 256×256 32bpp dans les deux cas —
//! et les employer **suggérerait à tort que le drapeau protège de
//! l'agrandissement**. Le seul drapeau est `SIIGBF_ICONONLY`.
//!
//! ⚠️ IL N'EST VÉRIFIÉ QUE PAR `cargo check --target x86_64-pc-windows-gnu` :
//! types, emprunts, visibilités, durées de vie — **et PAS l'édition de liens**,
//! la cible réelle étant `msvc`. **Aucun test d'hôte ne peut couvrir ce
//! module**, et sa seule preuve de fonctionnement est la recette.

use std::path::Path;
use std::sync::OnceLock;

use anyhow::{bail, Context, Result};
use proto::plateforme::SourceMax;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::DeleteObject;
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

/// Le côté de l'image demandée. **Ce n'est PAS une preuve de provenance** :
/// le Shell rend cette taille quoi qu'il arrive.
const COTE: i32 = 256;

/// 🔴 `ICONES=0` DÉSARME ; UNE SIMPLE PRÉSENCE N'ACTIVE PAS.
///
/// Convention d'`APPS`, `PLEIN_ECRAN`, `AUDIO` et `PART_SONDAGE`. Tester
/// `is_ok()` ACTIVERAIT le mécanisme en écrivant `ICONES=0` **pour le
/// couper** — les deux erreurs se compensent au point que personne ne les
/// verrait.
///
/// ⚠️ LE PRÉDICAT LUI-MÊME EST RÉUTILISÉ, PAS RECOPIÉ : `apps::desarme` est
/// pur, testé, et porte sa propre rouge nommée. Deux prédicats identiques
/// divergeraient le jour où l'un accepterait `"false"`.
///
/// Trois raisons d'exister, et elles ne sont pas décoratives : l'extraction
/// est la première chose de ce projet qui fasse durer une réconciliation en
/// SECONDES (2 298 ms pour 153 icônes au premier tour, mesuré) ; elle donne au
/// critère ③ un témoin qui n'exige AUCUN rebâtissage du binaire ; et elle rend
/// le désarmement observable.
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

/// L'icône d'un raccourci, en PNG 256×256 avec son canal alpha.
///
/// 🔴 SUR LE `.lnk` LUI-MÊME, ET NON SUR LA CIBLE. **92 des 153 raccourcis
/// retenus de cette VM portent un `IconLocation` sans chemin** : leur icône
/// est celle de la cible, et les autres portent une icône PROPRE au raccourci.
/// Seul le Shell connaît toute cette chaîne, et c'est pourquoi on la lui
/// demande plutôt que de la reconstruire.
pub fn extraire(lnk: &Path) -> Result<Vec<u8>> {
    let large = vers_utf16(&lnk.to_string_lossy());
    // SÉCURITÉ : appel FFI. Le chemin est un tampon UTF-16 terminé par un nul
    // que nous possédons pour toute la durée de l'appel.
    let fabrique: IShellItemImageFactory = unsafe {
        SHCreateItemFromParsingName(windows::core::PCWSTR(large.as_ptr()), None)
    }
    .with_context(|| format!("SHCreateItemFromParsingName sur {}", lnk.display()))?;

    // SÉCURITÉ : appel FFI. `SIIGBF_ICONONLY` seul — voir l'en-tête du module.
    let hbm = unsafe { fabrique.GetImage(SIZE { cx: COTE, cy: COTE }, SIIGBF_ICONONLY) }
        .with_context(|| format!("GetImage 256 sur {}", lnk.display()))?;

    let png = encoder_png(hbm);

    // 🔴 `DeleteObject` SUR TOUS LES CHEMINS DE SORTIE, Y COMPRIS D'ERREUR.
    // Une fuite de HBITMAP ici coûterait 256×256×4 octets par icône et par
    // tour — 40 Mo par réconciliation sur ce corpus, toutes les trente
    // secondes.
    // SÉCURITÉ : appel FFI. Le bitmap vient de `GetImage` et n'est relâché
    // qu'ici ; `encoder_png` n'en prend pas la propriété.
    let _ = unsafe { DeleteObject(hbm.into()) };
    png
}

/// L'encodage lui-même. **Séparé pour que le `DeleteObject` de l'appelant
/// courre quoi qu'il arrive.**
fn encoder_png(hbm: windows::Win32::Graphics::Gdi::HBITMAP) -> Result<Vec<u8>> {
    // SÉCURITÉ : appel FFI. L'appartement COM est celui du fil d'apps, ouvert
    // une fois par `lecture::initialiser_com`.
    let fabrique: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
            .context("CoCreateInstance(WICImagingFactory)")?;

    // 🔴 LA LIGNE QUI DÉCIDE DU CRITÈRE ①. `WICBitmapUseAlpha` conserve le
    // canal ; `WICBitmapIgnoreAlpha` le jette, et c'est la mutation qui joue
    // la rouge.
    // SÉCURITÉ : appel FFI. Le bitmap appartient à l'appelant et lui survit à
    // cet appel — WIC en fait sa propre copie.
    let bitmap = unsafe {
        fabrique.CreateBitmapFromHBITMAP(
            hbm,
            windows::Win32::Graphics::Gdi::HPALETTE(std::ptr::null_mut()),
            WICBitmapUseAlpha,
        )
    }
        .context("CreateBitmapFromHBITMAP")?;

    // SÉCURITÉ : appel FFI. Un flux sur HGLOBAL, dont WIC prend la charge.
    let flux: IStream = unsafe {
        CreateStreamOnHGlobal(windows::Win32::Foundation::HGLOBAL(std::ptr::null_mut()), true)
    }
        .context("CreateStreamOnHGlobal")?;

    // SÉCURITÉ : appel FFI.
    let encodeur = unsafe { fabrique.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null()) }
        .context("CreateEncoder(PNG)")?;
    // SÉCURITÉ : appel FFI.
    unsafe { encodeur.Initialize(&flux, windows::Win32::Graphics::Imaging::WICBitmapEncoderNoCache) }
        .context("Initialize de l'encodeur PNG")?;

    let mut cadre = None;
    // SÉCURITÉ : appel FFI. `cadre` est rempli par l'appel.
    unsafe { encodeur.CreateNewFrame(&mut cadre, std::ptr::null_mut()) }
        .context("CreateNewFrame")?;
    let cadre = cadre.context("l'encodeur n'a rendu aucun cadre")?;
    // SÉCURITÉ : appel FFI.
    unsafe { cadre.Initialize(None) }.context("Initialize du cadre")?;
    // SÉCURITÉ : appel FFI.
    unsafe { cadre.WriteSource(&bitmap, std::ptr::null()) }.context("WriteSource")?;
    // SÉCURITÉ : appel FFI.
    unsafe { cadre.Commit() }.context("Commit du cadre")?;
    // SÉCURITÉ : appel FFI.
    unsafe { encodeur.Commit() }.context("Commit de l'encodeur")?;

    relire(&flux)
}

/// Relit le flux du début, et rend ses octets.
fn relire(flux: &IStream) -> Result<Vec<u8>> {
    // SÉCURITÉ : appel FFI. Se replacer au début : `Commit` laisse le curseur
    // à la fin, et lire de là rendrait ZÉRO octet — un PNG vide qui aurait
    // l'air d'un succès.
    unsafe { flux.Seek(0, STREAM_SEEK_SET, None) }.context("Seek au début du flux PNG")?;
    let stat = {
        let mut s = Default::default();
        // SÉCURITÉ : appel FFI.
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
    // SÉCURITÉ : appel FFI. Le tampon fait exactement `taille` octets.
    unsafe { flux.Read(octets.as_mut_ptr().cast(), taille as u32, Some(&mut lus)) }
        .ok()
        .context("Read du flux PNG")?;
    if lus as usize != taille {
        bail!("flux PNG tronqué : {lus} octets lus sur {taille}");
    }
    Ok(octets)
}

/// La PROVENANCE de l'image — la plus grande entrée réellement PRÉSENTE dans
/// le répertoire d'icônes de la source.
///
/// 🔴 ELLE NE VIENT JAMAIS DU PNG. Un code qui la déduirait de la taille
/// rendue donnerait `256` à TOUT, y compris à un `.ico` qui ne contient que du
/// 48×48 — c'est mesuré, deux fois, et c'est tout l'objet du sous-bloc.
///
/// ⚠️ ELLE REND `NonMesuree` SANS ERREUR quand la source n'est ni un module PE
/// ni un `.ico` lisible. **Mesuré : 37 des 153 applications de cette VM.** Ce
/// n'est pas une panne, et le journaliser comme telle noierait le vrai signal.
pub fn provenance_de(icon_location: &str, cible: &str) -> SourceMax {
    match source::provenance(icon_location, cible) {
        source::Provenance::Ico(chemin) => match std::fs::read(&chemin) {
            // ⚠️ SEULS LES PREMIERS OCTETS SONT NÉCESSAIRES, mais un `.ico`
            // pèse quelques dizaines de kilooctets : le lire en entier coûte
            // moins qu'une ouverture partielle, et c'est plus simple à relire.
            Ok(octets) => ressource::maximum(
                &ressource::tailles_icondir(&octets).unwrap_or_default(),
            ),
            Err(erreur) => {
                tracing::debug!(chemin, %erreur, "ico illisible, provenance non mesuree");
                SourceMax::NonMesuree
            }
        },
        source::Provenance::Module(chemin) => {
            let index = source::index(icon_location);
            match lecture_pe_grpicondir(&chemin, index) {
                Ok(octets) => ressource::maximum(
                    &ressource::tailles_grpicondir(&octets).unwrap_or_default(),
                ),
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
