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
    /// `None` seulement de façon transitoire, le temps d'un `resize` (voir son
    /// commentaire) : DXGI n'autorise qu'une seule instance vivante
    /// d'`IDXGIOutputDuplication` par sortie et par processus à la fois, donc
    /// l'ancienne capture doit être explicitement relâchée avant que
    /// `DesktopCapture::new()` ne rappelle `DuplicateOutput`. Ne vaut jamais
    /// `None` en dehors de cette fenêtre : `capture()`/`capture_mut()` le
    /// supposent et paniquent sinon, un signe de bug plutôt qu'un état à
    /// gérer silencieusement.
    capture: Option<DesktopCapture>,
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
    /// Vrai dès que `self.encoder` a rendu sa toute première sortie. Sert
    /// uniquement à borner `SUBMIT_POLL_BUDGET` (voir sa doc) à la phase de
    /// démarrage : remis à faux par `resize`, qui reconstruit un encodeur
    /// neuf n'ayant lui non plus encore rien produit.
    encoder_warmed_up: bool,
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
            capture: Some(capture),
            region,
            encoder,
            width,
            height,
            fps,
            bitrate,
            next_pts_90k: 0,
            fatal: false,
            encoder_warmed_up: false,
        })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Capture courante, mutable. Panique hors de `resize` (voir le
    /// commentaire du champ) : ce n'est alors jamais `None`, une panique ici
    /// trahirait un bug plutôt qu'un état normal à absorber silencieusement.
    fn capture_mut(&mut self) -> &mut DesktopCapture {
        self.capture.as_mut().expect("capture toujours présente hors de resize()")
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

        // Relâche explicitement l'ancienne capture (donc son
        // `IDXGIOutputDuplication`) AVANT d'en créer une nouvelle. DXGI
        // n'autorise qu'une seule instance vivante de la duplication pour une
        // sortie donnée, dans un même processus : une simple réaffectation
        // (`self.capture = Some(DesktopCapture::new()?)`) évaluerait le
        // membre droit — donc `DuplicateOutput` — avant de remplacer
        // l'ancien `Some`, laissant les deux exister en même temps le temps
        // de l'appel. `DuplicateOutput` échoue alors avec « duplication de
        // la sortie écran » — observé lors de l'essai bout en bout de la
        // tâche 13, avant ce correctif.
        self.capture = None;

        // Un NOUVEAU périphérique D3D11 est créé ici : `DesktopCapture::new`
        // pose `SetMultithreadProtected(TRUE)` sur CE périphérique à chaque
        // appel (voir capture.rs, champ `context`/`multithread`) — la
        // protection est donc reconstruite avec lui, pas seulement héritée de
        // l'ancien périphérique qui vient d'être libéré. Sans cela le
        // blocage intermittent d'`AcquireNextFrame` documenté à la tâche 10
        // réapparaîtrait après tout redimensionnement.
        let new_capture = DesktopCapture::new()?;
        let (dw, dh) = new_capture.desktop_size();
        self.region = crop_region(window_rect, dw, dh)
            .ok_or_else(|| anyhow::anyhow!("la fenêtre est hors de l'écran"))?;
        let (actual_width, actual_height) = (self.region.width, self.region.height);

        self.encoder = H264Encoder::new(
            new_capture.device(),
            actual_width,
            actual_height,
            self.fps,
            self.bitrate,
        )?;
        self.capture = Some(new_capture);
        self.encoder.request_keyframe()?;
        self.width = actual_width;
        self.height = actual_height;
        // Nouvel encodeur : sa toute première sortie retombe dans le même
        // cas que le démarrage initial (voir `SUBMIT_POLL_BUDGET`).
        self.encoder_warmed_up = false;

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
/// soumise** à l'encodeur, **uniquement tant qu'il n'a encore rien produit**
/// (voir `WindowsSource::encoder_warmed_up`) — jamais à l'attente d'une
/// nouvelle capture, et jamais non plus une fois l'encodeur établi comme
/// capable de répondre.
///
/// L'encodeur matériel est asynchrone (voir `encode.rs`) : après le tout
/// premier `submit()`, `poll_output()` n'a presque jamais encore de résultat
/// au premier essai, l'événement `METransformHaveOutput` mettant un ou deux
/// cycles à arriver. Sans ce court réessai, la toute première image (donc le
/// premier keyframe) n'était récupérée qu'au tour suivant de `Session::run`
/// (~16,7 ms plus tard) au mieux. Borné à quelques dizaines de millisecondes :
/// largement suffisant pour ce démarrage, sans jamais s'approcher de la
/// seconde qui affamait `Session::run` (ronde de correction 1).
///
/// **Ronde de diagnostic (débit plafonné ~25-30 im/s) :** repéré en relecture
/// que l'inverse de la valeur alors en vigueur (40 ms) tombait exactement sur
/// le plafond observé — hypothèse d'un plafond ARTIFICIEL si ce réessai
/// bloquait `act_on_timeout` (donc tout `Session::run`, capture ET
/// transport) sur la quasi-totalité des tours en régime établi. Mesuré
/// directement (instrumentation temporaire, retirée) : FAUX sur cette VM. Le
/// réessai résolvait systématiquement en 3-5 ms (jamais le budget de 40 ms
/// atteint, sur des centaines d'images), et réduire le budget de 40 ms à
/// 2 ms (vingt fois moins) n'a strictement rien changé au débit mesuré côté
/// navigateur (297/10 s dans les deux cas, contenu identique). Le plafond
/// réel se situe en amont : `DesktopCapture::next_frame` (donc
/// `AcquireNextFrame`, non bloquant) ne signale une image neuve qu'à ~30 Hz,
/// alors que la boucle l'interroge, elle, à 60 Hz exact (mesuré par
/// comptage — voir aussi `fix-debit-socket-report.md`) : la cadence de
/// composition/duplication du bureau sur cette VM est la vraie limite,
/// identique que le contenu change par animation de page ou par défilement
/// réel piloté à la molette. `SUBMIT_POLL_BUDGET` n'y est pour rien — mais
/// comme il ne coûtait donc jamais rien qu'au tout premier démarrage
/// (jamais revérifié une fois l'encodeur chaud), il est désormais borné à
/// ce seul cas : sur du matériel où l'encodeur répondrait plus lentement en
/// régime établi, l'ancienne version aurait pu réellement brider `run()`
/// jusqu'à ce budget à chaque image — ce que cette restriction élimine
/// structurellement, sans rien changer au débit mesuré ici.
const SUBMIT_POLL_BUDGET: std::time::Duration = std::time::Duration::from_millis(40);
const SUBMIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);

impl VideoSource for WindowsSource {
    /// Un seul essai de capture par appel, jamais d'attente pour une
    /// nouvelle image — avec un court réessai borné, réservé au tout
    /// premier démarrage de l'encodeur, pour en récupérer la sortie.
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
    /// mais dégradait fortement le débit observé côté navigateur au tout
    /// démarrage : la cadence externe de `Session::run` (~16,7 ms) est trop
    /// grossière pour rattraper à temps la sortie de la toute première image
    /// soumise, avant que l'encodeur ait prouvé qu'il répond vite. Le
    /// réessai réapparaît donc, mais borné à `SUBMIT_POLL_BUDGET`.
    ///
    /// **3e correctif, après diagnostic du plafond de débit (voir
    /// `SUBMIT_POLL_BUDGET`) :** ce réessai avait fini par s'appliquer à
    /// *chaque* image soumise, pas seulement à la première — sans
    /// conséquence mesurée sur cette VM (il ne consommait jamais son budget
    /// en régime établi) mais restant un risque latent sur du matériel plus
    /// lent, où il aurait réellement bridé `run()` à `1/SUBMIT_POLL_BUDGET`.
    /// Désormais réservé à la phase de démarrage (`encoder_warmed_up`) :
    /// une fois l'encodeur prouvé capable de répondre, chaque soumission ne
    /// fait plus qu'un seul essai immédiat, exactement comme le cas « rien
    /// de neuf à capturer » ci-dessous — une sortie non encore prête sort au
    /// tour suivant, 16,7 ms plus tard, sans jamais bloquer celui-ci.
    ///
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
        let region = self.region;
        match self.capture_mut().next_frame(region) {
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

        if !submitted || self.encoder_warmed_up {
            // Soit rien de neuf à capturer ce tour-ci (cas normal), soit
            // l'encodeur a déjà prouvé qu'il répond vite (voir la doc de
            // `SUBMIT_POLL_BUDGET`) : dans les deux cas, un seul essai,
            // retour immédiat, jamais d'attente.
            return match self.encoder.poll_output() {
                Ok(Some(unit)) => {
                    self.encoder_warmed_up = true;
                    Some(unit)
                }
                Ok(None) => None,
                Err(e) => {
                    tracing::warn!(erreur = %e, "récupération de l'image encodée échouée");
                    None
                }
            };
        }

        // Encodeur pas encore chaud : sa toute première sortie peut mettre
        // un peu plus d'un tour à arriver (voir la doc de
        // `SUBMIT_POLL_BUDGET`) — on l'attend brièvement plutôt que de
        // retarder le tout premier keyframe.
        let deadline = std::time::Instant::now() + SUBMIT_POLL_BUDGET;
        loop {
            match self.encoder.poll_output() {
                Ok(Some(unit)) => {
                    self.encoder_warmed_up = true;
                    return Some(unit);
                }
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        // Pas encore prête : elle sortira à un appel
                        // suivant. Pas un échec, juste une latence un peu
                        // plus longue que la normale à ce tout premier tour.
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

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        WindowsSource::resize(self, width, height)
    }

    fn is_alive(&self) -> bool {
        WindowsSource::is_alive(self)
    }
}
