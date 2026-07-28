//! Source vidéo réunissant la capture de fenêtre et l'encodage matériel.

#![cfg(windows)]

use anyhow::Result;
use windows::Win32::Foundation::HWND;

use crate::capture::DesktopCapture;
use crate::encode::H264Encoder;
use crate::geometry::{crop_region, Rect};
use crate::h264::{AccessUnit, CLOCK_RATE_HZ};
use crate::rebuild::{rebuild_or_recover, RebuildOutcome};
use crate::source::VideoSource;
use crate::window;

pub struct WindowsSource {
    hwnd: HWND,
    /// `None` seulement de façon transitoire, à l'intérieur de `resize` (voir
    /// son commentaire) : DXGI n'autorise qu'une seule instance vivante
    /// d'`IDXGIOutputDuplication` par sortie et par processus à la fois, donc
    /// l'ancienne capture doit être explicitement relâchée avant que
    /// `DesktopCapture::new()` ne rappelle `DuplicateOutput`.
    ///
    /// **Ronde de correction 1 (revue) :** une première version de ce
    /// correctif vidait ce champ puis tentait la reconstruction via `?` — si
    /// celle-ci échouait (GPU transitoirement indisponible, fenêtre déplacée
    /// hors écran pendant le geste...), le champ restait `None` pour de bon,
    /// et l'appel suivant à `next_frame` paniquait sur
    /// `capture.as_mut().expect(...)`. Un panic traverse `spawn_blocking` et
    /// termine tout le processus agent — exactement ce que le brief demande
    /// de ne jamais faire pour un échec de redimensionnement censé être
    /// toléré. `resize` garantit désormais qu'un échec retombe sur une
    /// capture de secours (voir `rebuild::rebuild_or_recover`) plutôt que de
    /// laisser ce champ vide ; le seul cas où il reste `None` après `resize`
    /// est le double échec (ni la reconstruction complète, ni le secours),
    /// auquel cas `fatal` passe à vrai et `next_frame` court-circuite AVANT
    /// de toucher ce champ (voir son garde en tête de fonction) — jamais de
    /// panique, y compris dans ce pire cas.
    capture: Option<DesktopCapture>,
    /// Région de l'écran à recadrer, recalculée à chaque redimensionnement.
    region: Rect,
    encoder: H264Encoder,
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
    /// Origine de l'horloge de présentation. Jamais réinitialisée, y compris
    /// lorsque `resize` reconstruit la chaîne d'encodage : le décodeur du
    /// navigateur rejetterait un horodatage qui recule.
    clock_origin: std::time::Instant,
    /// Dernier horodatage attribué, pour garantir la stricte croissance même
    /// si deux captures tombaient dans la même graduation de 1/90000 s.
    last_pts_90k: Option<u64>,
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
    /// Unités d'accès déjà récupérées de l'encodeur mais pas encore rendues à
    /// l'appelant : `VideoSource::next_frame` n'en rend qu'une par tour, alors
    /// que le drainage peut en sortir plusieurs (voir `drain_ready_output`).
    /// Jamais purgée par `resize` : ces unités-là sont valides et déjà
    /// horodatées, les jeter ne ferait que trouer la vidéo.
    ready: std::collections::VecDeque<AccessUnit>,
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
            clock_origin: std::time::Instant::now(),
            last_pts_90k: None,
            fatal: false,
            encoder_warmed_up: false,
            ready: std::collections::VecDeque::new(),
        })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Capture courante, mutable. Le seul appelant (`next_frame`) garde le
    /// garde `if self.fatal { return None; }` avant tout appel : ce champ ne
    /// vaut `None` que pendant `resize`, et `resize` ne rend jamais la main
    /// avec `capture` à `None` sans avoir aussi mis `fatal` à vrai (voir le
    /// commentaire du champ). Panique donc seulement sur un bug réel de cet
    /// invariant, jamais en usage normal.
    fn capture_mut(&mut self) -> &mut DesktopCapture {
        self.capture.as_mut().expect("capture toujours présente quand fatal est faux")
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
        // la sortie écran » — observé lors du premier essai bout en bout de
        // la tâche 13.
        //
        // Cette libération anticipée ouvre en retour une fenêtre où
        // `self.capture` peut rester `None` si la reconstruction échoue : on
        // ne la referme jamais avec un simple `?` (voir la ronde de
        // correction 1 au commentaire du champ `capture`). `rebuild_or_recover`
        // (module `rebuild`, testé sans dépendance Windows) porte cette
        // logique : tenter la reconstruction complète, et si elle échoue,
        // retenter EXPLICITEMENT une capture de secours — avec les anciens
        // `region`/`encoder`/dimensions, encore valides puisqu'eux n'ont pas
        // été touchés — avant de renvoyer l'erreur à l'appelant.
        self.capture = None;
        let fps = self.fps;
        let bitrate = self.bitrate;

        // Un NOUVEAU périphérique D3D11 est créé dans `DesktopCapture::new` :
        // elle pose `SetMultithreadProtected(TRUE)` sur CE périphérique à
        // chaque appel (voir capture.rs, champ `context`/`multithread`) — la
        // protection est donc reconstruite avec lui, pas seulement héritée de
        // l'ancien périphérique qui vient d'être libéré. Sans cela le
        // blocage intermittent d'`AcquireNextFrame` documenté à la tâche 10
        // réapparaîtrait après tout redimensionnement.
        let outcome = rebuild_or_recover(
            || -> Result<(DesktopCapture, Rect, H264Encoder)> {
                let new_capture = DesktopCapture::new()?;
                let (dw, dh) = new_capture.desktop_size();
                let region = crop_region(window_rect, dw, dh)
                    .ok_or_else(|| anyhow::anyhow!("la fenêtre est hors de l'écran"))?;
                let mut encoder =
                    H264Encoder::new(new_capture.device(), region.width, region.height, fps, bitrate)?;
                encoder.request_keyframe()?;
                Ok((new_capture, region, encoder))
            },
            // Fabrique de secours : juste une capture valide, pour ne jamais
            // laisser `self.capture` à `None` sans `fatal` à vrai en retour.
            // `region`/`encoder`/`width`/`height` restent ceux d'avant :
            // seule la capture avait dû être relâchée, pas les paramètres qui
            // en dépendent, qui n'ont jamais cessé d'être valides.
            DesktopCapture::new,
        );

        match outcome {
            RebuildOutcome::Rebuilt((new_capture, region, encoder)) => {
                self.capture = Some(new_capture);
                self.region = region;
                self.encoder = encoder;
                self.width = region.width;
                self.height = region.height;
                // Nouvel encodeur : sa toute première sortie retombe dans le
                // même cas que le démarrage initial (voir `SUBMIT_POLL_BUDGET`).
                self.encoder_warmed_up = false;
                tracing::info!(self.width, self.height, "chaîne d'encodage reconstruite");
                Ok(())
            }
            RebuildOutcome::Recovered(new_capture, primary_error) => {
                // État exploitable restauré (anciens région/encodeur/
                // dimensions, nouvelle capture) : la session continue, comme
                // l'exige le brief pour un échec de redimensionnement. La
                // fenêtre OS, elle, a déjà changé de taille
                // (`resize_window` ci-dessus a réussi) : un décalage
                // transitoire entre la fenêtre réelle et la région capturée
                // est possible jusqu'au prochain redimensionnement réussi —
                // préférable, de loin, à un agent qui plante.
                self.capture = Some(new_capture);
                tracing::warn!(erreur = %primary_error, "reconstruction de la chaîne d'encodage échouée, capture de secours restaurée");
                Err(primary_error)
            }
            RebuildOutcome::Fatal(primary_error) => {
                // Ni la chaîne complète, ni une simple capture de secours
                // n'ont pu être obtenues : `self.capture` reste `None`.
                // `fatal` le signale pour de bon — `next_frame` s'arrête
                // avant de toucher `capture` (voir son garde), et
                // `is_exhausted()` fera clore la session proprement au tour
                // suivant, plutôt qu'un panic sur le champ vide.
                self.fatal = true;
                tracing::error!(erreur = %primary_error, "reconstruction de la chaîne d'encodage et capture de secours toutes deux échouées, source déclarée épuisée");
                Err(primary_error)
            }
        }
    }

    /// Vrai tant que la fenêtre capturée existe.
    pub fn is_alive(&self) -> bool {
        window::is_window_alive(self.hwnd)
    }

    /// Demande une image clé — notamment sur requête du navigateur, relayée
    /// depuis `Event::KeyframeRequest` par `transport.rs` via l'implémentation
    /// `VideoSource::request_keyframe` ci-dessous.
    pub fn request_keyframe(&mut self) -> Result<()> {
        self.encoder.request_keyframe()
    }

    /// Retire du pipeline tout ce qui est prêt, sans jamais attendre, et rend
    /// l'unité d'accès la plus ancienne encore en file.
    ///
    /// Trois choses dans le même tour, et c'est le point : (1) vider les
    /// sorties déjà produites, (2) rendre à l'encodeur les entrées que ce
    /// drainage vient de lui permettre d'accepter, (3) ne rendre qu'une unité
    /// à l'appelant, les autres attendant les tours suivants.
    ///
    /// Sans (1) et (2) dans le même tour, le cycle complet de l'encodeur
    /// (soumission → production → récupération → nouvelle demande d'entrée)
    /// ne franchissait qu'une étape par tour de `Session::run` : mesuré à
    /// `need_input_hz=23` pour `ticks_hz=60` et `captured_hz=46`, soit un
    /// débit divisé par ~2,6 alors que la capture et l'encodeur avaient tous
    /// deux la marge nécessaire (`desktop_updates_hz=68`, `ENCODE_TEST` à
    /// 66 i/s). Voir `H264Encoder::flush_pending_inputs`.
    ///
    /// Le drainage est borné : un encodeur qui rendrait de la sortie sans fin
    /// ne doit pas pouvoir retenir la boucle de transport, qui a aussi ICE,
    /// RTCP et les canaux de données à servir.
    fn drain_ready_output(&mut self) -> Option<AccessUnit> {
        let t_drain = std::time::Instant::now();
        let unit = self.drain_ready_output_inner();
        DRAIN_NS.fetch_add(
            t_drain.elapsed().as_nanos() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        unit
    }

    /// Le drainage proprement dit ; séparé du chronométrage ci-dessus pour que
    /// tous les chemins de sortie (dont les `break` d'erreur) soient mesurés.
    fn drain_ready_output_inner(&mut self) -> Option<AccessUnit> {
        const MAX_DRAIN: usize = 8;
        for _ in 0..MAX_DRAIN {
            match self.encoder.poll_output() {
                Ok(Some(unit)) => {
                    self.encoder_warmed_up = true;
                    PRODUCED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    self.ready.push_back(unit);
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(erreur = %e, "récupération de l'image encodée échouée");
                    break;
                }
            }
        }
        // Les emplacements d'entrée libérés par le drainage ci-dessus sont
        // réutilisables dès maintenant : ne pas attendre le tour suivant.
        if let Err(e) = self.encoder.flush_pending_inputs() {
            tracing::warn!(erreur = %e, "réalimentation de l'encodeur échouée");
        }
        self.ready.pop_front()
    }

    /// Horodatage de présentation de l'image qu'on vient de capturer, lu sur
    /// une horloge réelle.
    ///
    /// **Correction de la latence (28/07).** La version précédente comptait
    /// les images plutôt que le temps (`next_pts_90k += CLOCK_RATE_HZ / fps`
    /// à chaque soumission réussie), ce qui suppose une source à cadence
    /// parfaitement régulière. Celle-ci ne l'est pas et ne peut pas l'être :
    /// Desktop Duplication ne rend une image que lorsque le bureau change,
    /// donc les soumissions sont espacées de 16,7 ms, de 33 ms, ou de
    /// plusieurs secondes sur un écran immobile — alors que le compteur, lui,
    /// avançait invariablement de 16,7 ms.
    ///
    /// La ligne de temps RTP dérivait donc du temps réel sans jamais se
    /// recaler : à 30 i/s réels elle avançait deux fois trop lentement, et
    /// après une pause d'écran immobile elle repartait comme si cette pause
    /// n'avait pas eu lieu. Le récepteur WebRTC calcule sa gigue sur l'écart
    /// entre l'espacement d'arrivée et l'espacement annoncé par les
    /// horodatages : un décalage systématiquement positif lui fait gonfler
    /// sa cible de tampon, image après image, jusqu'à une resynchronisation
    /// brutale. C'est la signature relevée en recette
    /// (`docs/superpowers/plans/2026-07-27-jalon1-recette.md`, critère 3) :
    /// 641,8 → 877,6 → 1164,0 → 1449,9 ms de latence, puis retour net à
    /// 83,0 ms — sans qu'aucun gel ne soit détecté, ce qui excluait déjà un
    /// arrêt de la capture ou de l'encodage.
    ///
    /// Lire l'horloge à la capture donne au navigateur la ligne de temps
    /// qu'il attend, quelle que soit la régularité de la source.
    fn next_pts_90k(&mut self) -> u64 {
        let elapsed = self.clock_origin.elapsed();
        // Nanosecondes → 1/90000 s, en 128 bits : `as_nanos() * 90_000`
        // déborderait un `u64` au bout d'environ 57 heures de session.
        let pts = (elapsed.as_nanos() * CLOCK_RATE_HZ as u128 / 1_000_000_000) as u64;
        // Deux captures dans la même graduation (11 µs) ne peuvent pas
        // arriver au rythme d'un appel par tour de `Session::run`, mais un
        // horodatage qui ne progresse pas ferait rejeter l'image par le
        // décodeur : on l'exclut structurellement plutôt que de compter sur
        // la cadence de l'appelant.
        let pts = match self.last_pts_90k {
            Some(last) if pts <= last => last + 1,
            _ => pts,
        };
        self.last_pts_90k = Some(pts);
        pts
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
/// navigateur (297/10 s dans les deux cas, contenu identique). À ce stade du
/// diagnostic, le plafond réel était attribué en amont : `DesktopCapture::next_frame`
/// (donc `AcquireNextFrame`, non bloquant) ne signalait une image neuve qu'à
/// ~30 Hz, alors que la boucle l'interroge, elle, à 60 Hz exact (mesuré par
/// comptage — voir aussi `docs/superpowers/plans/fix-debit-socket-report.md`),
/// ce qui avait fait suspecter la cadence de composition/duplication du
/// bureau elle-même comme vraie limite.
///
/// **Hypothèse écartée depuis**, par la recette du jalon 1
/// (`docs/superpowers/plans/2026-07-27-jalon1-recette.md`, critère 2) :
/// mesurée isolément (`CAPTURE_TEST`), la capture soutient ~90 im/s sur cette
/// même VM — la composition/duplication du bureau n'est pas le goulot. Le
/// plafond réel se situe côté encodeur matériel : `H264Encoder::submit`, dans
/// `encode.rs`, ne reçoit de nouvelles demandes d'entrée
/// (`METransformNeedInput`) qu'à ~30 Hz, alors que le même encodeur, sollicité
/// en boucle serrée (`ENCODE_TEST`), soutient ~80 im/s — une interaction non
/// résolue entre le rythme de soumission fixe (16,7 ms) et le rythme propre
/// du MFT matériel, pas une limite de la capture ni, en tant que telle, du
/// GPU/pilote NVIDIA (détail des essais qui écartent successivement les
/// hypothèses concurrentes dans
/// `docs/superpowers/plans/diagnostic-plafond-debit.md` et
/// `docs/superpowers/plans/remesure-debit.md`).
///
/// `SUBMIT_POLL_BUDGET` n'y est pour rien — mais
/// comme il ne coûtait donc jamais rien qu'au tout premier démarrage
/// (jamais revérifié une fois l'encodeur chaud), il est désormais borné à
/// ce seul cas : sur du matériel où l'encodeur répondrait plus lentement en
/// régime établi, l'ancienne version aurait pu réellement brider `run()`
/// jusqu'à ce budget à chaque image — ce que cette restriction élimine
/// structurellement, sans rien changer au débit mesuré ici.
const SUBMIT_POLL_BUDGET: std::time::Duration = std::time::Duration::from_millis(40);
const SUBMIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);

/// Compteurs du chemin réel (`Session::run`), à l'échelle du processus.
///
/// Volontairement des statiques plutôt qu'un champ : `resize` remplace
/// l'encodeur — donc sa télémétrie — et la question à laquelle ces compteurs
/// doivent répondre (« où le flux s'arrête-t-il ? ») porte justement sur ce
/// qu'il advient *après* une reconstruction. Une poignée attachée à
/// l'encodeur cesserait d'être observée au moment précis qui intéresse.
///
/// Le coût est nul en pratique (trois incréments `Relaxed` par tour) et rien
/// ne les lit sauf le fil de surveillance de `main.rs`, activé par
/// `SOURCE_TRACE=1`. L'agent étant mono-session (un processus par session),
/// des statiques ne mélangent pas plusieurs sessions.
pub static TICKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static CAPTURED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static PRODUCED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Temps cumulé (ns) passé dans chaque étape d'un tour de `next_frame`.
///
/// Les compteurs ci-dessus disent COMBIEN d'images franchissent chaque étage ;
/// ceux-ci disent OÙ part le temps. La distinction est celle qui manquait pour
/// trancher entre « l'encodeur ne peut pas aller plus vite » et « on ne le
/// sollicite pas assez souvent » : un étage qui plafonne sans occuper le fil
/// attend quelque chose, un étage qui l'occupe est le vrai goulot.
///
/// Rapportés à la durée de la fenêtre d'observation, ils donnent un taux
/// d'occupation du fil de `Session::run` — le fil unique qui sert aussi ICE,
/// RTCP et les canaux de données.
pub static CAPTURE_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static SUBMIT_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static DRAIN_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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
        TICKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.fatal {
            // Une reconstruction de la chaîne par `resize` a pu échouer au
            // point de ne laisser aucune capture de secours valide non plus
            // (voir `RebuildOutcome::Fatal` dans `resize`) : `self.capture`
            // vaut alors `None` pour de bon. Ne JAMAIS appeler
            // `capture_mut()` dans ce cas — `is_exhausted()` (déjà vraie via
            // `self.fatal`) fera clore la session proprement au tour
            // suivant, plutôt qu'un panic sur le champ vide.
            return None;
        }

        // Alimenter l'encodeur avec l'image la plus récente, si le bureau a
        // changé depuis le dernier appel (Desktop Duplication ne rend une
        // image que sur changement — cas courant et normal, voir
        // capture.rs).
        let mut submitted = false;
        let region = self.region;
        let t_capture = std::time::Instant::now();
        let captured = self.capture_mut().next_frame(region);
        CAPTURE_NS.fetch_add(
            t_capture.elapsed().as_nanos() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        match captured {
            Ok(Some(frame)) => {
                CAPTURED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                // Horodatage lu sur l'horloge réelle AVANT la soumission :
                // c'est l'instant de la capture qui date l'image, pas celui
                // où l'encodeur voudra bien l'accepter (voir `next_pts_90k`).
                let pts = self.next_pts_90k();
                let t_submit = std::time::Instant::now();
                let fed = self.encoder.submit(&frame, pts);
                SUBMIT_NS.fetch_add(
                    t_submit.elapsed().as_nanos() as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );
                if let Err(e) = fed {
                    tracing::warn!(erreur = %e, "soumission à l'encodeur échouée");
                } else {
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
            // `SUBMIT_POLL_BUDGET`) : dans les deux cas, aucune attente —
            // mais on draine tout ce qui est DÉJÀ prêt, sans jamais dormir.
            return self.drain_ready_output();
        }

        // Encodeur pas encore chaud : sa toute première sortie peut mettre
        // un peu plus d'un tour à arriver (voir la doc de
        // `SUBMIT_POLL_BUDGET`) — on l'attend brièvement plutôt que de
        // retarder le tout premier keyframe.
        let deadline = std::time::Instant::now() + SUBMIT_POLL_BUDGET;
        loop {
            match self.drain_ready_output() {
                Some(unit) => {
                    return Some(unit);
                }
                None => {
                    if std::time::Instant::now() >= deadline {
                        // Pas encore prête : elle sortira à un appel
                        // suivant. Pas un échec, juste une latence un peu
                        // plus longue que la normale à ce tout premier tour.
                        return None;
                    }
                    std::thread::sleep(SUBMIT_POLL_INTERVAL);
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

    /// Relaie vers l'encodeur matériel (voir `WindowsSource::request_keyframe`
    /// et le commentaire de `VideoSource::request_keyframe`).
    fn request_keyframe(&mut self) -> Result<()> {
        WindowsSource::request_keyframe(self)
    }
}
