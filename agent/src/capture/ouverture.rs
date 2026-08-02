//! Résolution d'une sortie DXGI par cible, et duplication de cette sortie.
//!
//! Extrait de `capture.rs` à la tâche 3 du sous-bloc D2 : l'ajout de
//! `EchecAcquisition` et de la reprise dans `next_frame` portait le fichier
//! parent au-dessus du plafond de 500 lignes (`CLAUDE.md`). Ces deux
//! fonctions ne touchent à aucun champ privé de `DesktopCapture` — elles
//! prennent le périphérique et la cible en paramètres — donc aucune raison
//! d'accès n'imposait de les garder dans le fichier parent.
//!
//! `dupliquer_avec_reprise` les y a rejointes à la tâche 11 bis, pour la même
//! raison de plafond et sans plus d'accès privé qu'elles.

use anyhow::{anyhow, bail, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIAdapter1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
};

use super::CibleCapture;

/// Ouvre une sortie désignée par son nom, ou — pour `CibleCapture::Bureau` —
/// trouve et ouvre la sortie qui compose le bureau.
pub(super) fn ouvrir_sortie(
    factory: &IDXGIFactory1,
    cible: &CibleCapture,
) -> Result<(IDXGIAdapter1, IDXGIOutput1)> {
    let mut index_adaptateur = 0u32;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(index_adaptateur) } {
        let mut index_sortie = 0u32;
        while let Ok(output) = unsafe { adapter.EnumOutputs(index_sortie) } {
            let desc = match unsafe { output.GetDesc() } {
                Ok(desc) => desc,
                Err(_) => {
                    index_sortie += 1;
                    continue;
                }
            };
            let nom = String::from_utf16_lossy(&desc.DeviceName)
                .trim_end_matches('\0')
                .to_string();
            let retenue = match cible {
                CibleCapture::Sortie(vise) => &nom == vise,
                CibleCapture::Bureau => desc.AttachedToDesktop.as_bool(),
            };
            if retenue {
                let name = match unsafe { adapter.GetDesc1() } {
                    Ok(adapter_desc) => String::from_utf16_lossy(&adapter_desc.Description)
                        .trim_end_matches('\0')
                        .to_string(),
                    Err(_) => "<inconnu>".to_string(),
                };
                tracing::info!(
                    adaptateur = %name, index_adaptateur, index_sortie, nom_sortie = %nom,
                    attachee = desc.AttachedToDesktop.as_bool(),
                    "sortie retenue pour la duplication"
                );
                return Ok((adapter.clone(), output.cast()?));
            }
            index_sortie += 1;
        }
        index_adaptateur += 1;
    }
    match cible {
        CibleCapture::Sortie(nom) => bail!("aucune sortie DXGI nommée {nom}"),
        CibleCapture::Bureau => {
            bail!("aucune sortie attachée au bureau : la session est-elle interactive ?")
        }
    }
}

/// Crée le périphérique D3D11 sur l'adaptateur qui porte la sortie retenue, et
/// le rend partageable avec Media Foundation.
///
/// Extraite d'`ouvrir` à la tâche 11 bis : le réessai d'ouverture ajoutait une
/// ligne à `capture.rs`, déjà à 500 lignes exactement (`CLAUDE.md`), et la
/// consigne est d'extraire, jamais de compresser. **Déplacement pur : aucun
/// appel, aucun ordre, aucune valeur n'a changé.**
pub(super) fn creer_peripherique(
    adapter: &IDXGIAdapter1,
) -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            adapter,
            // Un adaptateur explicite impose le type « inconnu ».
            windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .context("création du périphérique D3D11")?;
    }
    let device = device.ok_or_else(|| anyhow!("périphérique D3D11 absent"))?;
    let context = context.ok_or_else(|| anyhow!("contexte D3D11 absent"))?;

    // Le contexte immédiat D3D11 n'est PAS sûr en accès concurrent par
    // défaut : le pilote suppose un seul fil et ne pose aucun verrou. Or ce
    // périphérique ne reste pas privé — il est confié à Media Foundation
    // par un `IMFDXGIDeviceManager` (voir `encode::share_device`), et le
    // convertisseur BGRA→NV12 comme l'encodeur H.264 matériel s'en servent
    // depuis leurs propres fils de travail internes, pendant que notre fil
    // principal appelle `CopySubresourceRegion` dans `crop` et que la
    // duplication de sortie — bâtie sur ce même périphérique — sert
    // `AcquireNextFrame`.
    //
    // Sans cette protection, deux fils entrent en même temps dans le
    // pilote et l'un d'eux peut ne jamais ressortir. C'est le blocage
    // mesuré ici : quatre exécutions sur quatre figées dans
    // `AcquireNextFrame`, pourtant appelée avec un délai d'attente NUL,
    // donc censée ne jamais bloquer — l'attente ne venait pas de DXGI mais
    // du verrou interne du pilote. Aucune erreur n'est remontée, la
    // fonction ne rend simplement plus la main.
    //
    // `SetMultithreadProtected(TRUE)` fait prendre au pilote son verrou
    // interne autour de chaque commande : c'est la condition documentée
    // pour partager un périphérique D3D11 avec Media Foundation, et elle
    // doit être posée AVANT `DuplicateOutput`, la duplication héritant du
    // périphérique tel qu'il est à cet instant.
    let multithread: ID3D11Multithread = context
        .cast()
        .context("obtention de ID3D11Multithread depuis le contexte immédiat")?;
    let was_protected = unsafe { multithread.SetMultithreadProtected(true) };
    tracing::info!(
        protection_precedente = was_protected.as_bool(),
        "protection multi-fils activée sur le contexte immédiat D3D11"
    );

    Ok((device, context))
}

/// Duplique la sortie et lit ses dimensions. Le seul morceau d'`ouvrir` que
/// `rouvrir` refait.
///
/// écart d'API windows-rs 0.62 : `GetDesc` ne prend plus de paramètre de
/// sortie ; elle renvoie directement la structure (par valeur pour
/// `IDXGIOutputDuplication`, dans un `Result` pour `IDXGIOutput1` et
/// `IDXGIAdapter1`, ces deux dernières pouvant échouer).
pub(super) fn dupliquer(
    device: &ID3D11Device,
    output: &IDXGIOutput1,
) -> Result<(IDXGIOutputDuplication, u32, u32)> {
    let duplication =
        unsafe { output.DuplicateOutput(device) }.context("duplication de la sortie écran")?;
    let desc = unsafe { duplication.GetDesc() };
    Ok((duplication, desc.ModeDesc.Width, desc.ModeDesc.Height))
}

/// Duplique la sortie, **en retentant tant que DXGI dit « pas maintenant »**,
/// et au plus pendant `fenetre`.
///
/// Employée par la seule construction de `DesktopCapture` (`ouvrir`), et non
/// par `rouvrir` : celle-ci est déjà appelée depuis la fenêtre de reprise de
/// `next_frame`, qui joue le même rôle par un autre montage.
///
/// **Extraite ici plutôt qu'écrite dans `capture.rs`** : ce fichier parent est
/// à 500 lignes exactement, marge nulle (`CLAUDE.md`), et cette boucle
/// n'enveloppe que `dupliquer`, qui vit déjà ici.
pub(super) fn dupliquer_avec_reprise(
    device: &ID3D11Device,
    output: &IDXGIOutput1,
    cible: &CibleCapture,
    fenetre: std::time::Duration,
) -> Result<(IDXGIOutputDuplication, u32, u32)> {
    // Retenter la SEULE duplication, et sur place.
    //
    // **Bloquer n'est pas légitime partout, d'où la `fenetre` reçue en
    // argument plutôt que lue ici.** Au démarrage d'un enfant du superviseur
    // elle est pleine : rien ne tourne encore — ni keyframe à servir, ni
    // adaptation réseau, ni redimensionnement en attente. Mais `ouvrir` est
    // AUSSI rappelée en pleine session par `WindowsSource::resize`
    // (`DesktopCapture::new`, deux fabriques dans `rebuild_or_recover`), sur
    // le fil bloquant de `Session::run` : celle-là passe une durée NULLE, et
    // se comporte donc exactement comme avant ce réessai. C'est le même
    // arbitrage que `next_frame`, qui refuse déjà de dormir pour cette raison.
    //
    // Sans ce réessai, une première `DuplicateOutput` tombée pendant que
    // Windows reconfigure sa topologie tuait l'enfant, que le superviseur
    // relançait en DÉTRUISANT puis RECRÉANT sa sortie — abandonnant du même
    // coup le mutex de toutes les duplications déjà ouvertes. La
    // compensation coûtait plus que la panne : 32 des 44 réouvertures du
    // relevé du 1ᵉʳ août 2026 venaient de cette seule étape.
    let debut = std::time::Instant::now();
    loop {
        match dupliquer(device, output) {
            Ok(rendu) => break Ok(rendu),
            Err(erreur) => {
                // `dupliquer` rend une `anyhow::Error` bâtie par `.context()`
                // sur une `windows::core::Error` : anyhow conserve la cause
                // sous-jacente et `downcast_ref` la retrouve. Si jamais elle
                // ne pouvait pas être lue, `retentable` vaudrait `false` et
                // l'ouverture échouerait sans réessai — dégradation sûre,
                // jamais une boucle.
                let code = erreur
                    .downcast_ref::<windows::core::Error>()
                    .map(|e| e.code().0);
                let retentable =
                    code.is_some_and(crate::capture_reprise::est_ouverture_retentable);
                if !retentable || debut.elapsed() >= fenetre {
                    // Bruyant à dessein : c'est ici que se lit un plafond
                    // de duplications concurrentes, indiscernable d'une
                    // reconfiguration par le seul HRESULT.
                    tracing::error!(
                        hresult = code.map(|c| format!("{c:#010x}")),
                        attendu_ms = debut.elapsed().as_millis() as u64,
                        cible = ?cible,
                        "ouverture de la duplication abandonnée"
                    );
                    break Err(erreur);
                }
                tracing::info!(
                    hresult = code.map(|c| format!("{c:#010x}")),
                    cible = ?cible,
                    "duplication indisponible à l'ouverture, nouvel essai"
                );
                std::thread::sleep(crate::capture_reprise::PAS_REPRISE);
            }
        }
    }
}
