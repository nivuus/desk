//! Capture de l'écran par DXGI Desktop Duplication, recadrée sur une fenêtre.
//!
//! `Windows.Graphics.Capture` aurait permis de capturer directement la fenêtre,
//! mais cette API est inutilisable sur Windows Server 2022 : le service système
//! qui l'implémente plante sur `CreateForWindow`. On duplique donc la sortie
//! écran et on recadre. Les images restent sur le GPU : le recadrage se fait
//! par `CopySubresourceRegion`, sans aller-retour en mémoire centrale.

#![cfg(windows)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET, D3D11_BOX,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, IDXGIOutputDuplication, IDXGIResource,
    DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};

use crate::geometry::Rect;

/// Compteurs de diagnostic de l'acquisition DXGI (voir `SOURCE_TRACE`).
///
/// `ACCUMULATED` est la somme de `DXGI_OUTDUPL_FRAME_INFO::AccumulatedFrames`,
/// c'est-à-dire le nombre de mises à jour du bureau que DXGI a fusionnées dans
/// les images qu'il nous a rendues. C'est la seule mesure qui distingue les
/// deux explications d'un `captured_hz` bas : si `ACCUMULATED` est nettement
/// supérieur à `HITS`, le bureau se met bien à jour vite et c'est nous qui
/// l'interrogeons trop rarement ; s'il le suit de près, c'est la source
/// (la fenêtre capturée) qui ne produit pas davantage.
pub static ATTEMPTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static HITS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static ACCUMULATED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Une image capturée et recadrée, résidente sur le GPU.
pub struct CapturedFrame {
    pub texture: ID3D11Texture2D,
    pub width: u32,
    pub height: u32,
}

pub struct DesktopCapture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    /// Posée à `None` par `rouvrir`, et PEUT y rester si sa réouverture
    /// échoue : plus un défaut, voir `duplication()` et `types::lire`.
    duplication: Option<IDXGIOutputDuplication>,
    /// Dernier HRESULT de perte d'accès, lu par `duplication()` si le champ
    /// ci-dessus est `None`.
    dernier_code_perdu: i32,
    /// Ce qu'il faut rouvrir après une perte d'accès. Retenu à l'ouverture :
    /// à l'instant où l'accès est perdu, la topologie a déjà changé et rien
    /// dans les objets DXGI encore détenus ne dit ce qu'on capturait.
    cible: CibleCapture,
    /// Fenêtre de reprise en cours (voir `next_frame`) : « ouverte à la
    /// première perte d'accès et **refermée par le premier succès** »
    /// (`capture::reprise`). `succes()` étant appelée sur `Ok(None)`, toute
    /// acquisition non refusée la referme — la durée court donc depuis le
    /// DERNIER refus. Est définitive une perte ININTERROMPUE au-delà d'elle.
    fenetre: crate::capture_reprise::FenetreDeReprise,
    desktop_width: u32,
    desktop_height: u32,
    /// Texture de destination, réallouée seulement quand la taille change.
    target: Option<(ID3D11Texture2D, u32, u32)>,
    /// Vrai tant qu'une image acquise n'a pas été relâchée.
    frame_held: bool,
    /// Étape courante publiée pour le fil de surveillance (voir
    /// `encode::PHASE_CAPTURE_*`). Absente hors mode diagnostic.
    phase: Option<Arc<AtomicU64>>,
}

impl DesktopCapture {
    /// Ouvre au DÉMARRAGE : la fenêtre de réessai est pleine, et l'appel peut
    /// donc bloquer jusqu'à `DUREE_FENETRE_OUVERTURE`.
    ///
    /// **N'employer que là où bloquer est légitime** — voir `ouvrir`. Pour la
    /// reconstruction d'une capture en cours de session, c'est
    /// `new_sans_attente` qu'il faut.
    pub fn new() -> Result<Self> {
        Self::ouvrir(CibleCapture::Bureau, crate::capture_reprise::DUREE_FENETRE_OUVERTURE)
    }

    /// Ouvre SANS attendre : un refus est rendu au premier essai.
    ///
    /// Pour les appelants qui courent sur un fil dont l'immobilisation se
    /// paierait — `WindowsSource::resize`, qui reconstruit sa capture depuis
    /// le fil bloquant de `Session::run`. Y dormir suspendrait du même coup
    /// les demandes de keyframe, l'adaptation réseau et les
    /// redimensionnements suivants : exactement l'arbitrage que `next_frame`
    /// refuse déjà (voir son commentaire « Pas de boucle interne »).
    ///
    /// Comportement **identique à celui d'avant l'ajout du réessai** : rien
    /// n'est retenté, seule la trace d'abandon est neuve.
    pub fn new_sans_attente() -> Result<Self> {
        Self::ouvrir(CibleCapture::Bureau, std::time::Duration::ZERO)
    }

    /// Duplique une sortie DXGI précise, désignée par son nom
    /// (`\\.\DISPLAYn`, tel que `enumerer_sorties` le rend).
    ///
    /// **Par le nom et non par des index d'énumération** : ceux-ci sont
    /// positionnels et changent dès qu'une sortie apparaît ou disparaît — ce
    /// qui est le cas nominal en multi-fenêtres, où le superviseur crée une
    /// sortie par ouverture de fenêtre.
    ///
    /// Fenêtre de réessai pleine : cette forme n'est appelée qu'au démarrage
    /// d'un enfant du superviseur.
    pub fn sur_sortie(nom: &str) -> Result<Self> {
        Self::ouvrir(
            CibleCapture::Sortie(nom.to_string()),
            crate::capture_reprise::DUREE_FENETRE_OUVERTURE,
        )
    }

    /// `fenetre_ouverture` : durée pendant laquelle une duplication refusée
    /// pour indisponibilité passagère est retentée. **Un ARGUMENT et non la
    /// constante lue sur place, parce que le droit de bloquer se décide chez
    /// l'appelant** — `ouvrir` ne court pas qu'au démarrage d'un enfant. Voir
    /// `ouverture::dupliquer_avec_reprise` pour l'arbitrage complet.
    fn ouvrir(cible: CibleCapture, fenetre_ouverture: std::time::Duration) -> Result<Self> {
        let factory: IDXGIFactory1 =
            unsafe { CreateDXGIFactory1() }.context("création de la fabrique DXGI")?;

        // Sans cible (`new()`, `CibleCapture::Bureau`), on retient le premier
        // adaptateur possédant une sortie attachée au bureau : c'est celui qui
        // compose l'écran, et donc le seul duplicable. Sur la VM cible c'est la
        // RTX 4070, ce qui donne du même coup le bon périphérique pour
        // l'encodeur matériel de la tâche 10. Avec une cible
        // (`CibleCapture::Sortie`), c'est celle-ci qui est ouverte telle quelle.
        let (adapter, output) = ouvrir_sortie(&factory, &cible)?;

        let (device, context) = creer_peripherique(&adapter)?;

        let (duplication, desktop_width, desktop_height) =
            dupliquer_avec_reprise(&device, &output, &cible, fenetre_ouverture)?;
        tracing::info!(desktop_width, desktop_height, "duplication de sortie établie");

        Ok(Self {
            device,
            context,
            duplication: Some(duplication),
            dernier_code_perdu: 0,
            cible,
            fenetre: crate::capture_reprise::FenetreDeReprise::nouvelle(),
            desktop_width,
            desktop_height,
            target: None,
            frame_held: false,
            phase: None,
        })
    }

    pub fn device(&self) -> &ID3D11Device {
        &self.device
    }

    /// Reconstruit la duplication après une perte d'accès, **en conservant le
    /// périphérique D3D11**.
    ///
    /// Ce n'est pas une économie, c'est une nécessité. L'encodeur H.264 est lié
    /// à ce périphérique par l'`IMFDXGIDeviceManager` (`encode::share_device`) :
    /// en créer un neuf obligerait à détruire l'encodeur, donc à emprunter
    /// `Drop for H264Encoder`, dont le pire cas est borné à 8 s et où un gel a
    /// déjà été observé (`CLAUDE.md`). Une reprise censée passer inaperçue ne
    /// peut pas payer ce prix. La protection multifil posée sur le contexte à
    /// l'ouverture n'est pas rejouée : elle porte sur le contexte immédiat,
    /// qu'on conserve.
    ///
    /// **L'ancienne duplication est relâchée AVANT que la neuve ne soit
    /// demandée, et l'ordre est le fond de cette méthode.** DXGI n'autorise
    /// qu'**une** duplication par sortie. La version précédente appelait
    /// `dupliquer()` alors que `self.duplication` détenait encore l'objet
    /// périmé : l'appel réussissait — 378 fois sur 378 au relevé du
    /// 1ᵉʳ août 2026 — et rendait une duplication **mort-née**, qui refusait
    /// aussitôt toute acquisition. Huit secondes de réessais toutes les 150 ms
    /// n'en sortaient jamais.
    pub fn rouvrir(&mut self) -> Result<()> {
        // L'image détenue d'abord : `release_frame` appelle `ReleaseFrame` sur
        // la duplication qu'on s'apprête à relâcher.
        self.release_frame();

        // PUIS la duplication elle-même, et c'est cette ligne qui compte.
        // `None` la fait relâcher ici, pas à l'affectation d'après.
        self.duplication = None;

        let factory: IDXGIFactory1 =
            unsafe { CreateDXGIFactory1() }.context("création de la fabrique DXGI (réouverture)")?;
        let (_adapter, output) = ouvrir_sortie(&factory, &self.cible)
            .context("résolution de la sortie à rouvrir")?;
        let (duplication, largeur, hauteur) = dupliquer(&self.device, &output)?;

        // Les dimensions peuvent avoir changé : la texture de destination est
        // dimensionnée sur la RÉGION demandée par l'appelant, pas sur celles-ci,
        // mais `desktop_size()` est lue ailleurs et doit rester juste.
        self.duplication = Some(duplication);
        self.desktop_width = largeur;
        self.desktop_height = hauteur;
        Ok(())
    }

    /// La duplication courante, ou le dernier HRESULT perdu si absente — voir `types::lire`.
    fn duplication(&self) -> std::result::Result<&IDXGIOutputDuplication, EchecAcquisition> {
        types::lire(&self.duplication, self.dernier_code_perdu)
    }

    /// Branche le marqueur d'étape partagé avec le fil de surveillance.
    ///
    /// Sans lui, un blocage dans `next_frame` reste anonyme : les trois appels
    /// Windows qu'elle enchaîne se confondent en une seule étape.
    pub fn set_phase_marker(&mut self, phase: Arc<AtomicU64>) {
        self.phase = Some(phase);
    }

    fn set_phase(&self, value: u64) {
        if let Some(phase) = &self.phase {
            phase.store(value, Ordering::Relaxed);
        }
    }

    pub fn desktop_size(&self) -> (u32, u32) {
        (self.desktop_width, self.desktop_height)
    }

    /// Acquiert l'image suivante et la recadre sur `region`, **en se rouvrant
    /// si DXGI lui a révoqué l'accès**.
    ///
    /// La reprise est ici, et non chez l'appelant, à dessein : le banc
    /// multi-fenêtres capture par cette fonction sans passer par
    /// `WindowsSource` (`diagnostics/multifenetre/voies.rs`). Une reprise logée
    /// plus haut laisserait le banc hors du chemin de production, et la mesure
    /// qui doit valider cette voie ne vaudrait rien.
    pub fn next_frame(
        &mut self,
        region: Rect,
    ) -> std::result::Result<Option<CapturedFrame>, EchecAcquisition> {
        match self.tenter_acquisition(region) {
            Ok(issue) => {
                self.fenetre.succes();
                Ok(issue)
            }
            Err(EchecAcquisition::AccesPerdu(code_perdu)) => {
                // Pas de boucle interne, et c'est le point de conception :
                // cette fonction est appelée depuis la boucle de
                // `Session::run`, et y dormir plusieurs secondes suspendrait
                // du même coup les demandes de keyframe, les changements de
                // barreau de l'adaptation réseau et les redimensionnements.
                // La reprise s'étale donc sur plusieurs appels.
                match self.fenetre.tenter(std::time::Instant::now()) {
                    crate::capture_reprise::Tentative::Rouvrir => {
                        // `info!` et non `debug!`, pour les deux traces de
                        // cette branche : l'exploitation tourne en
                        // RUST_LOG=info, et une mitigation muette n'en est pas
                        // une (même règle que `encode/arret.rs`, `CLAUDE.md`
                        // — ne pas les redescendre).
                        //
                        // Le HRESULT nu, et non seulement inféré à la lecture
                        // du code (comme a dû le faire le rapport de la
                        // mesure du 1ᵉʳ août 2026) : c'est la seule pièce qui
                        // dit CE QUI a été perdu.
                        tracing::info!(
                            tentative = self.fenetre.tentatives(),
                            cible = ?self.cible,
                            hresult = format!("{code_perdu:#010x}"),
                            "accès à la duplication perdu, réouverture"
                        );
                        if let Err(erreur) = self.rouvrir() {
                            // Un échec de réouverture n'est PAS définitif : la
                            // sortie peut n'être pas encore réapparue dans la
                            // topologie. On le dit et on laisse la fenêtre
                            // courir — c'est elle qui tranchera. VRAI depuis
                            // le correctif de relecture de la tâche 6 quater
                            // (`types::lire`) seulement : avant lui, l'appel
                            // suivant trouvait `self.duplication` à `None` et
                            // rompait la fenêtre en panne malgré ce texte.
                            tracing::info!(
                                erreur = %erreur,
                                cible = ?self.cible,
                                "réouverture de la duplication échouée, la fenêtre de reprise court toujours"
                            );
                        }
                        Ok(None)
                    }
                    crate::capture_reprise::Tentative::Patienter => Ok(None),
                    crate::capture_reprise::Tentative::Expiree => {
                        Err(EchecAcquisition::AccesPerdu(code_perdu))
                    }
                }
            }
            Err(panne) => Err(panne),
        }
    }

    /// Acquiert l'image suivante et la recadre sur `region`, sans reprise.
    ///
    /// Renvoie `Ok(None)` si aucune image nouvelle n'est disponible — cas
    /// courant et normal : le bureau ne change pas à chaque appel.
    fn tenter_acquisition(
        &mut self,
        region: Rect,
    ) -> std::result::Result<Option<CapturedFrame>, EchecAcquisition> {
        self.release_frame();

        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        ATTEMPTS.fetch_add(1, Ordering::Relaxed);
        // `duplication()` D'ABORD, la phase ENSUITE — et non l'inverse. Ce `?`
        // sort de la fonction à chaque tour de la fenêtre de reprise (jusqu'à
        // `DUREE_FENETRE_REPRISE`, 8 s), sans jamais atteindre le retour à
        // `PHASE_CAPTURE` posé après l'acquisition : le fil de surveillance
        // journalisait `étape = capture/AcquireNextFrame` pendant ces 8 s alors
        // que l'appel n'était pas fait une seule fois. Ce dépôt a déjà perdu
        // une campagne à attribuer un blocage au mauvais appel — une étape
        // publiée ne doit désigner que du code réellement en cours.
        let duplication = self.duplication()?;
        // Attente nulle : la cadence est pilotée par la boucle appelante, pas
        // par un blocage ici.
        self.set_phase(crate::encode::PHASE_CAPTURE_ACQUIRE);
        let acquired = unsafe { duplication.AcquireNextFrame(0, &mut info, &mut resource) };
        self.set_phase(crate::encode::PHASE_CAPTURE);

        if acquired.is_ok() {
            HITS.fetch_add(1, Ordering::Relaxed);
            ACCUMULATED.fetch_add(info.AccumulatedFrames as u64, Ordering::Relaxed);
        }

        if let Err(e) = acquired {
            if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                return Ok(None);
            }
            // Classer sur le CODE, jamais sur le texte du message : `CLAUDE.md`
            // porte le précédent d'un libellé Windows qui a fait attribuer un
            // refus au mauvais appel pendant tout un chantier.
            if crate::capture_reprise::est_acces_perdu(e.code().0) {
                self.dernier_code_perdu = e.code().0;
                return Err(EchecAcquisition::AccesPerdu(e.code().0));
            }
            return Err(EchecAcquisition::Panne(anyhow!("acquisition d'image : {e}")));
        }
        self.frame_held = true;

        // À partir d'ici l'image est détenue : tout chemin de sortie doit la
        // relâcher, sans quoi la duplication refuse toute acquisition
        // ultérieure. `release_frame` en tête de la prochaine itération ne
        // couvre pas le cas d'une erreur qui remonte et arrête la boucle,
        // ni celui d'un appelant qui réessaie après avoir avalé l'erreur.
        let cropped: Result<CapturedFrame> = (|| {
            let resource = resource.ok_or_else(|| anyhow!("ressource d'image absente"))?;
            let desktop: ID3D11Texture2D = resource.cast()?;
            self.crop(&desktop, region)
        })();
        match cropped {
            Ok(frame) => {
                // Relâcher TOUT DE SUITE, pas au tour suivant.
                //
                // `crop` a déjà recopié les pixels dans notre propre texture
                // (`CopySubresourceRegion` vers `self.target`) : l'image du
                // bureau ne sert plus à rien passé cette ligne. La garder
                // jusqu'à l'appel suivant, comme le faisait la version
                // précédente via le seul `release_frame()` en tête de
                // fonction, immobilisait la duplication pendant tout
                // l'intervalle entre deux tours — soit ~16,7 ms sur 16,7 à la
                // cadence de `Session::run`.
                //
                // Mesuré : la même fenêtre Firefox, avec le même contenu,
                // rendait 74 images/s à `CAPTURE_TEST` (boucle serrée, donc
                // relâchement toutes les ~2 ms) contre seulement 22 images/s
                // dans `Session::run` (relâchement toutes les ~16,7 ms). Ce
                // n'était donc ni la composition du bureau, ni le pilote, ni
                // la contention GPU avec l'encodeur (`ENCODE_TEST` tient
                // 66 i/s encodeur compris) : c'était la durée de détention.
                self.release_frame();
                Ok(Some(frame))
            }
            Err(e) => {
                self.release_frame();
                Err(EchecAcquisition::Panne(e))
            }
        }
    }

    /// Copie la région demandée dans une texture dédiée, sur le GPU.
    fn crop(&mut self, source: &ID3D11Texture2D, region: Rect) -> Result<CapturedFrame> {
        let (width, height) = (region.width, region.height);
        if width == 0 || height == 0 {
            bail!("région de recadrage vide");
        }

        // Réallouer seulement si la taille a changé : un redimensionnement est
        // rare, une image ne l'est pas.
        let need_alloc = !matches!(self.target, Some((_, w, h)) if w == width && h == height);
        if need_alloc {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut texture: Option<ID3D11Texture2D> = None;
            unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut texture)) }
                .context("allocation de la texture de recadrage")?;
            self.target = Some((
                texture.ok_or_else(|| anyhow!("texture de recadrage absente"))?,
                width,
                height,
            ));
        }

        let (texture, _, _) = self.target.as_ref().expect("texture allouée");
        let box_ = D3D11_BOX {
            left: region.x.max(0) as u32,
            top: region.y.max(0) as u32,
            front: 0,
            right: region.x.max(0) as u32 + width,
            bottom: region.y.max(0) as u32 + height,
            back: 1,
        };
        self.set_phase(crate::encode::PHASE_CAPTURE_CROP);
        unsafe {
            self.context
                .CopySubresourceRegion(texture, 0, 0, 0, 0, source, 0, Some(&box_));
        }
        self.set_phase(crate::encode::PHASE_CAPTURE);

        Ok(CapturedFrame { texture: texture.clone(), width, height })
    }

    fn release_frame(&mut self) {
        if self.frame_held {
            self.set_phase(crate::encode::PHASE_CAPTURE_RELEASE);
            // Défensif plutôt que `duplication()` (qui rend un `Result`) :
            // cette méthode tourne aussi dans `Drop`, où rien ne doit remonter.
            if let Some(duplication) = self.duplication.as_ref() {
                // Un échec ici n'est pas récupérable et ne doit pas masquer la suite.
                let _ = unsafe { duplication.ReleaseFrame() };
            }
            self.set_phase(crate::encode::PHASE_CAPTURE);
            self.frame_held = false;
        }
    }
}

impl Drop for DesktopCapture {
    fn drop(&mut self) {
        self.release_frame();
    }
}

// `SortieDxgi` vit dans `crate::sortie_dxgi` (portable, hors `#[cfg(windows)]`)
// pour que `superviseur::placement::sortie_par_dimensions` (tâche 7) puisse la
// consommer sur l'hôte Linux — `crate::capture` n'existe pas du tout hors
// Windows. Ce réexport garde `crate::capture::SortieDxgi` valide et identique
// au type portable pour tout le code qui, lui, ne compile que sous Windows.
pub use crate::sortie_dxgi::SortieDxgi;

// `enumerer_sorties`, `enumerer_sorties_silencieux` et le reste de
// l'énumération DXGI vivent dans ce module enfant, extrait à la tâche 2 du
// sous-bloc D2 pour ramener ce fichier sous le plafond de 500 lignes
// (`CLAUDE.md`) — voir l'en-tête de `capture/enumeration.rs`. Réexportées ici
// pour que `crate::capture::enumerer_sorties` reste valide sans toucher à un
// seul appelant.
mod enumeration;
pub use enumeration::{enumerer_sorties, enumerer_sorties_silencieux};

// `ouvrir_sortie` et `dupliquer` vivent dans ce module enfant, extrait à la
// tâche 3 du sous-bloc D2 pour ramener ce fichier sous le plafond de 500
// lignes (`CLAUDE.md`) après l'ajout d'`EchecAcquisition` et de la reprise
// dans `next_frame` — voir l'en-tête de `capture/ouverture.rs`.
// `creer_peripherique` et `dupliquer_avec_reprise` les y ont rejointes à la
// tâche 11 bis, la seconde portant le réessai d'ouverture et la première n'y
// étant descendue que pour rendre au fichier parent la marge que ce réessai
// lui prenait.
pub mod ouverture;
use ouverture::{creer_peripherique, dupliquer, dupliquer_avec_reprise, ouvrir_sortie};

// `EchecAcquisition`, `CibleCapture` et l'aide `lire` vivent dans ce module
// enfant, extrait à la tâche 6 quater du sous-bloc D2 — voir l'en-tête de
// `capture/types.rs`. `capture/ouverture.rs` continue de résoudre
// `super::CibleCapture` sans changement.
mod types;
pub use types::{CibleCapture, EchecAcquisition};
