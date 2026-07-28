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
    /// Vrai après une erreur non récupérable (capture ou encodeur) : rend la
    /// source définitivement épuisée (voir `is_exhausted`), indépendamment
    /// de l'état de la fenêtre. Une fenêtre disparue (`!is_alive()`) est
    /// l'autre cas d'épuisement ; celui-ci couvre les pannes qui n'affectent
    /// pas forcément la fenêtre elle-même (périphérique GPU perdu...).
    fatal: bool,
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
            fatal: false,
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

/// Budget accordé à l'attente de la sortie d'une image **qui vient d'être
/// soumise** à l'encodeur — jamais à l'attente d'une nouvelle capture.
///
/// L'encodeur matériel est asynchrone (voir `encode.rs`) : après
/// `submit()`, `poll_output()` n'a presque jamais encore de résultat au
/// tout premier essai, l'événement `METransformHaveOutput` mettant un ou
/// deux cycles à arriver. Sans ce court réessai, une image tout juste
/// soumise n'était récupérée qu'au tour suivant de `Session::run`
/// (~16,7 ms plus tard) au mieux — et en pratique nettement plus tard,
/// mesuré : la file de sorties prêtes de l'encodeur se remplit plus vite
/// qu'elle n'est vidée (un seul `poll_output` par appel externe), si bien
/// que les images partent par rafales espacées de silences complets — un
/// « figement » observable côté navigateur (`freezeCount`,
/// `jitterBufferDelay` démesuré), pas seulement une cadence moyenne basse.
/// Borné à quelques dizaines de millisecondes : largement suffisant pour
/// la latence propre du pipeline en régime établi, sans jamais s'approcher
/// de la seconde qui affamait `Session::run` (ronde de correction 1).
const SUBMIT_POLL_BUDGET: std::time::Duration = std::time::Duration::from_millis(40);
const SUBMIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);

impl VideoSource for WindowsSource {
    /// Un seul essai de capture par appel, jamais d'attente pour une
    /// nouvelle image — mais un court réessai borné pour récupérer la
    /// sortie de celle qu'on vient tout juste de soumettre.
    ///
    /// **Ronde de correction 1 (revue), 1er correctif :** une première
    /// version de cette méthode retentait en boucle (sommeil de 1 ms)
    /// jusqu'à DEUX SECONDES avant d'abandonner, **que quelque chose ait été
    /// capturé ou non**. Deux défauts en découlaient, tous deux mesurés par
    /// la relecture : (1) elle ne distinguait pas « l'encodeur démarre » (le
    /// vrai bug visé) de « rien n'a bougé à l'écran » (le cas nominal d'une
    /// capture en direct — `DesktopCapture::next_frame` documente elle-même
    /// ce cas comme « courant et normal ») ; toute page statique ou tout
    /// instant sans mouvement de plus de deux secondes coupait donc le flux ;
    /// (2) pendant qu'elle bouclait, le fil unique de `Session::run` ne
    /// traitait plus ni ICE, ni RTCP, ni les canaux de données — observé en
    /// pratique par une déconnexion ICE spontanée ~20 s après la
    /// négociation.
    ///
    /// **2e correctif, après mesure :** supprimer TOUT réessai (un essai
    /// unique, quoi qu'il arrive) réglait bien les deux défauts ci-dessus,
    /// mais dégradait fortement le débit observé côté navigateur (~16 im/s
    /// au lieu de ~25, avec de longs figements) : la cadence externe de
    /// `Session::run` (~16,7 ms) est trop grossière pour rattraper à temps
    /// la sortie d'une image tout juste soumise, qui s'accumule alors par
    /// rafales. Le réessai réapparaît donc, mais **seulement quand une
    /// image a réellement été soumise ce tour-ci** (`submitted`), et borné à
    /// `SUBMIT_POLL_BUDGET` (quelques dizaines de ms, pas deux secondes).
    /// Quand rien n'a été capturé (cas normal, bureau immobile), retour
    /// immédiat, sans boucle ni attente, comme l'exige la revue. `None` ne
    /// signifie donc jamais « rien cette fois » ; il reste possible pour
    /// deux causes réellement définitives (`is_exhausted` en informe
    /// l'appelant) : la fenêtre a disparu, ou une erreur de capture non
    /// récupérable s'est produite.
    fn next_frame(&mut self) -> Option<AccessUnit> {
        // Alimenter l'encodeur avec l'image la plus récente, si le bureau a
        // changé depuis le dernier appel (Desktop Duplication ne rend une
        // image que sur changement — cas courant et normal, voir
        // capture.rs).
        let mut submitted = false;
        match self.capture.next_frame(self.region) {
            Ok(Some(frame)) => {
                let pts = self.next_pts_90k;
                if let Err(e) = self.encoder.submit(&frame, pts) {
                    tracing::warn!(erreur = %e, "soumission à l'encodeur échouée");
                } else {
                    self.next_pts_90k += CLOCK_RATE_HZ / self.fps.max(1) as u64;
                    submitted = true;
                }
            }
            Ok(None) => {}
            Err(e) => {
                // Erreur de capture réelle (pas une absence de changement,
                // déjà traduite en `Ok(None)`) : périphérique perdu ou autre
                // panne non récupérable — fin légitime et définitive.
                tracing::error!(erreur = %e, "capture interrompue, source déclarée épuisée");
                self.fatal = true;
                return None;
            }
        }

        if !submitted {
            // Rien de neuf à capturer ce tour-ci : cas normal. On tente
            // quand même de récupérer une sortie déjà en attente d'un appel
            // précédent (l'encodeur peut avoir 1-2 images en vol), mais sans
            // jamais attendre — un seul essai, retour immédiat.
            return match self.encoder.poll_output() {
                Ok(unit) => unit,
                Err(e) => {
                    tracing::warn!(erreur = %e, "récupération de l'image encodée échouée");
                    None
                }
            };
        }

        // Une image vient d'être soumise : sa sortie est imminente (latence
        // propre au pipeline asynchrone), on l'attend brièvement plutôt que
        // de la laisser s'accumuler jusqu'au tour suivant.
        let deadline = std::time::Instant::now() + SUBMIT_POLL_BUDGET;
        loop {
            match self.encoder.poll_output() {
                Ok(Some(unit)) => return Some(unit),
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        // Pas encore prête : elle sortira à un appel
                        // suivant. Pas un échec, juste une latence un peu
                        // plus longue que la normale ce tour-ci.
                        return None;
                    }
                    std::thread::sleep(SUBMIT_POLL_INTERVAL);
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

    /// Épuisée pour de bon seulement si la fenêtre a disparu ou qu'une
    /// erreur de capture non récupérable a été observée — jamais pour un
    /// simple bureau immobile (voir `next_frame`).
    fn is_exhausted(&self) -> bool {
        self.fatal || !self.is_alive()
    }
}
