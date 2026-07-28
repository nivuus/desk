//! Source vidéo réunissant la capture de fenêtre et l'encodage matériel.

#![cfg(windows)]

use anyhow::Result;
use windows::Win32::Foundation::HWND;

use crate::capture::DesktopCapture;
use crate::encode::H264Encoder;
use crate::geometry::{crop_region, Rect};
use crate::h264::{AccessUnit, CLOCK_RATE_HZ};
use crate::source::VideoSource;
use crate::window;

pub struct WindowsSource {
    hwnd: HWND,
    capture: DesktopCapture,
    /// Région de l'écran à recadrer, recalculée à chaque redimensionnement.
    region: Rect,
    encoder: H264Encoder,
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
    /// Horodatage attribué à la prochaine image soumise. Jamais remis à zéro,
    /// y compris lorsque `resize` reconstruit la chaîne d'encodage : le
    /// décodeur du navigateur rejetterait un horodatage qui recule.
    next_pts_90k: u64,
}

// SÉCURITÉ : les types COM enveloppés ici (`HWND`, `ID3D11Device`,
// `IMFTransform`...) ne sont pas `Send` par défaut dans windows-rs, mais
// `Session` (voir `transport.rs`) exige `Box<dyn VideoSource + Send>` car
// elle est déplacée une seule fois vers le fil dédié de `Session::run`
// (`tokio::task::spawn_blocking`, voir `main.rs`) — jamais partagée ni
// utilisée concurremment. Le périphérique D3D11 est explicitement protégé
// pour un accès multi-fils (`SetMultithreadProtected(TRUE)`, posé dans
// `DesktopCapture::new`, voir son commentaire) précisément parce qu'il est
// aussi sollicité par les fils internes de Media Foundation ; les objets MF
// eux-mêmes sont documentés agiles (utilisables depuis n'importe quel fil).
// Aucun de ces objets n'est donc accédé depuis deux fils à la fois : ce
// `WindowsSource` est seulement transféré, une fois, avant tout usage.
unsafe impl Send for WindowsSource {}

impl WindowsSource {
    pub fn new(hwnd: HWND, fps: u32, bitrate: u32) -> Result<Self> {
        let window_rect = window::client_rect_on_screen(hwnd)?;

        let capture = DesktopCapture::new()?;
        let (dw, dh) = capture.desktop_size();
        // `crop_region` aligne déjà les dimensions sur des valeurs paires,
        // exigées par l'encodeur H.264.
        let region = crop_region(window_rect, dw, dh)
            .ok_or_else(|| anyhow::anyhow!("la fenêtre est hors de l'écran"))?;
        let (width, height) = (region.width, region.height);

        let mut encoder = H264Encoder::new(capture.device(), width, height, fps, bitrate)?;
        encoder.request_keyframe()?;

        Ok(Self {
            hwnd,
            capture,
            region,
            encoder,
            width,
            height,
            fps,
            bitrate,
            next_pts_90k: 0,
        })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Redimensionne la fenêtre et reconstruit la chaîne d'encodage.
    ///
    /// Media Foundation n'autorise pas le changement de résolution en cours de
    /// route : il faut repartir d'un encodeur neuf. L'horodatage, lui, reste
    /// continu — le décodeur du navigateur rejetterait un retour en arrière
    /// (`next_pts_90k` n'est jamais réinitialisé ici).
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        let (width, height) = (width.max(160) & !1, height.max(120) & !1);
        if (width, height) == (self.width, self.height) {
            return Ok(());
        }

        window::resize_window(self.hwnd, width, height)?;
        // Laisser la fenêtre atteindre sa nouvelle taille avant de recapturer.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let window_rect = window::client_rect_on_screen(self.hwnd)?;

        // Un NOUVEAU périphérique D3D11 est créé ici : `DesktopCapture::new`
        // pose `SetMultithreadProtected(TRUE)` sur CE périphérique à chaque
        // appel (voir capture.rs, champ `context`/`multithread`) — la
        // protection est donc reconstruite avec lui, pas seulement héritée de
        // l'ancien périphérique qui va être libéré. Sans cela le blocage
        // intermittent d'`AcquireNextFrame` documenté à la tâche 10
        // réapparaîtrait après tout redimensionnement.
        self.capture = DesktopCapture::new()?;
        let (dw, dh) = self.capture.desktop_size();
        self.region = crop_region(window_rect, dw, dh)
            .ok_or_else(|| anyhow::anyhow!("la fenêtre est hors de l'écran"))?;
        let (actual_width, actual_height) = (self.region.width, self.region.height);

        self.encoder = H264Encoder::new(
            self.capture.device(),
            actual_width,
            actual_height,
            self.fps,
            self.bitrate,
        )?;
        self.encoder.request_keyframe()?;
        self.width = actual_width;
        self.height = actual_height;

        tracing::info!(self.width, self.height, "chaîne d'encodage reconstruite");
        Ok(())
    }

    /// Vrai tant que la fenêtre capturée existe.
    pub fn is_alive(&self) -> bool {
        window::is_window_alive(self.hwnd)
    }

    /// Demande une image clé, par exemple sur requête du navigateur.
    pub fn request_keyframe(&mut self) -> Result<()> {
        self.encoder.request_keyframe()
    }
}

/// Budget maximal accordé à un seul appel à `next_frame` pour produire une
/// unité d'accès avant d'abandonner.
///
/// Nécessaire pour une raison structurelle, pas seulement un cas limite :
/// `VideoSource::next_frame` renvoyant `None` met fin à la session (voir
/// `transport.rs`, « I5 : source épuisée »), un contrat pensé pour
/// `FileSource` qui ne renvoie jamais `None`. Or l'encodeur matériel est
/// asynchrone (voir encode.rs) : juste après avoir soumis la toute première
/// image capturée, `poll_output` n'a quasiment jamais encore de sortie prête
/// — l'événement `METransformHaveOutput` met quelques appels à arriver. Sans
/// retenter, le tout premier appel (déclenché à la négociation de la piste
/// vidéo, bien après la construction de la source) renvoyait `None` et
/// terminait la session avant qu'aucune image n'ait jamais été envoyée.
/// Retenter en boucle, borné par ce budget, laisse le temps au pipeline de
/// produire sa première sortie tout en bornant le temps pendant lequel le
/// fil unique de `Session::run` reste indisponible pour le reste du
/// transport (ICE, RTCP...).
const POLL_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);
/// Intervalle entre deux tentatives, le temps que le pipeline progresse.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);

impl VideoSource for WindowsSource {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        let deadline = std::time::Instant::now() + POLL_BUDGET;
        loop {
            // Alimenter l'encodeur avec l'image la plus récente, si le bureau
            // a changé depuis le dernier appel (Desktop Duplication ne rend
            // une image que sur changement, voir capture.rs).
            match self.capture.next_frame(self.region) {
                Ok(Some(frame)) => {
                    let pts = self.next_pts_90k;
                    if let Err(e) = self.encoder.submit(&frame, pts) {
                        tracing::warn!(erreur = %e, "soumission à l'encodeur échouée");
                    } else {
                        self.next_pts_90k += CLOCK_RATE_HZ / self.fps.max(1) as u64;
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::error!(erreur = %e, "capture interrompue");
                    return None;
                }
            }

            match self.encoder.poll_output() {
                Ok(Some(unit)) => return Some(unit),
                Ok(None) => {
                    // Rien n'était prêt ce tour-ci. Une fenêtre disparue est
                    // une fin légitime ; sinon on retente tant que le budget
                    // n'est pas écoulé — un bureau réellement figé plus de
                    // deux secondes est le seul cas où l'on referme la
                    // session pour ce motif.
                    if !self.is_alive() {
                        tracing::info!("fenêtre capturée disparue : fin de la source vidéo");
                        return None;
                    }
                    if std::time::Instant::now() >= deadline {
                        tracing::warn!(
                            budget = ?POLL_BUDGET,
                            "aucune image encodée disponible avant l'échéance : fin de la source vidéo"
                        );
                        return None;
                    }
                    std::thread::sleep(POLL_INTERVAL);
                }
                Err(e) => {
                    tracing::warn!(erreur = %e, "récupération de l'image encodée échouée");
                    return None;
                }
            }
        }
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
