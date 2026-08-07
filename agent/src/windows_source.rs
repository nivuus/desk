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
use crate::windows_source_sortie::ModeCapture;
// `Telemetrie` (D9, tâche 11) : compteurs de capture PAR SESSION, même
// montage FRÈRE que `windows_source_sortie` ci-dessus — voir
// `windows_source/telemetrie.rs`.
use crate::windows_source_telemetrie::Telemetrie;
// `WindowsSource::sur_sortie` vit dans `windows_source/sortie.rs` (module
// FRÈRE, déclaré `#[path]` dans `main.rs` sous le nom `windows_source_sortie`
// pour que son calcul pur de région et son `ModeCapture` restent testables sur
// l'hôte Linux) ; `WindowsSource::resize` vit dans le module ENFANT
// `windows_source/redimensionnement.rs`, déclaré ci-dessous. La différence
// n'est pas cosmétique : un module enfant voit les champs privés de ce
// module-ci, un module frère non — voir le commentaire des champs.

/// Redimensionnement de la fenêtre et reconstruction de la chaîne d'encodage.
///
/// Module **enfant** de `windows_source` (et non frère) précisément pour que
/// `resize` continue de lire et d'écrire les champs privés de `WindowsSource`
/// sans qu'aucun n'ait à être ouvert. Extrait d'ici parce que ce fichier est
/// en dette de taille (voir `CLAUDE.md`) et que le correctif C1 y ajoutait
/// `depuis_pieces` et le champ `mode`.
mod redimensionnement;

/// Changement de la seule taille d'encodage, capture et fenêtre inchangées.
///
/// Module **enfant** pour la même raison que `redimensionnement` ci-dessus :
/// il lit et écrit les champs privés de `WindowsSource`. Extrait d'ici au
/// sous-bloc D5, le remède du défaut C2 (détruire l'encodeur avant d'en
/// construire un neuf) y ajoutant une quinzaine de lignes que ce fichier, en
/// dette de taille gelée, ne pouvait pas absorber.
mod encodage;

pub struct WindowsSource {
    // Champs PRIVÉS, et c'est un invariant de conception, pas un détail :
    // plusieurs d'entre eux (`capture`, `fatal`, `width`/`height` face à
    // `encoder.encode_size()`) ne sont corrects que pris ensemble, et leurs
    // commentaires disent en quoi. Les seuls écrivains sont donc ce module et
    // son enfant `redimensionnement` ; le module frère
    // `windows_source_sortie`, lui, passe par `depuis_pieces` (`pub(crate)`)
    // et n'en touche aucun.
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
    /// `None` seulement de façon transitoire, à l'intérieur de
    /// `set_encode_size` (module enfant `encodage`), qui doit détruire
    /// l'encodeur courant AVANT d'en construire un neuf — voir son
    /// commentaire pour le pourquoi et pour le prix. Comme pour `capture`
    /// ci-dessus, le seul état où ce champ reste vide après retour est celui
    /// où `fatal` vaut vrai : les lecteurs passent donc par `encoder_mut`,
    /// qui rend une ERREUR et jamais une panique — deux d'entre eux
    /// (`request_keyframe`, `set_bitrate`) sont appelés depuis la boucle de
    /// transport sans garde `fatal`, et une panique y traverserait
    /// `spawn_blocking`.
    encoder: Option<H264Encoder>,
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
    /// Origine d'horloge **de la session**, imposée par `demarrage.rs` et partagée
    /// avec la source audio. C'est cette origine commune qui rend les deux
    /// lignes de temps comparables, donc la synchro A/V exacte. La créer ici
    /// la décalerait de la durée d'initialisation de l'autre source.
    ///
    /// Jamais réinitialisée, y compris lorsque `resize` reconstruit la chaîne
    /// d'encodage : le décodeur du navigateur rejetterait un horodatage qui
    /// recule.
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
    /// Ce que cette source capture — bureau recadré sur la fenêtre, ou sortie
    /// DXGI entière. **Seul `resize` le consulte**, et c'est sa raison d'être :
    /// voir `ModeCapture` (`windows_source/sortie.rs`) pour ce que son absence
    /// produisait.
    mode: ModeCapture,
    /// Compteurs de capture par session (D9, tâche 11), remplaçant les
    /// statiques `TICKS`/`CAPTURED`/`PRODUCED`. `pub(crate)` : lu depuis
    /// `capteur/fenetre.rs`, module frère et non enfant de celui-ci.
    pub(crate) telemetrie: Telemetrie,
}

// SÉCURITÉ : les types COM enveloppés ici (`HWND`, `ID3D11Device`,
// `IMFTransform`...) ne sont pas `Send` par défaut dans windows-rs, mais
// `Session` (voir `transport.rs`) exige `Box<dyn VideoSource + Send>` pour
// finir sur le fil dédié de `Session::run` (`tokio::task::spawn_blocking`,
// voir `demarrage.rs`). Ce transfert n'est PAS un unique déplacement littéral :
// entre sa construction et cette remise à `spawn_blocking`, l'objet est
// porté par une tâche `#[tokio::main]` (ordonnanceur multi-fils par défaut)
// qui franchit plusieurs `.await` (réception de l'offre, envoi de la
// réponse...), et peut donc être repris sur un fil de travail différent à
// chacun d'eux avant d'atteindre le fil bloquant dédié. Ce qui rend `Send`
// sûr n'est pas un compte de déplacements, mais l'absence d'accès
// CONCURRENT : à tout instant, un seul fil à la fois possède l'objet, quel
// qu'il soit, et plus aucun ne le touche une fois remis à `spawn_blocking`.
// Le périphérique D3D11 est explicitement protégé pour un accès multi-fils
// (`SetMultithreadProtected(TRUE)`, posé dans `DesktopCapture::new`, voir son
// commentaire) précisément parce qu'il est aussi sollicité par les fils
// internes de Media Foundation ; les objets MF eux-mêmes sont documentés
// agiles (utilisables depuis n'importe quel fil).
unsafe impl Send for WindowsSource {}

impl WindowsSource {
    pub fn new(hwnd: HWND, fps: u32, bitrate: u32, clock_origin: std::time::Instant) -> Result<Self> {
        let window_rect = window::client_rect_on_screen(hwnd)?;

        let capture = DesktopCapture::new()?;
        let (dw, dh) = capture.desktop_size();
        // `crop_region` aligne déjà les dimensions sur des valeurs paires,
        // exigées par l'encodeur H.264.
        let region = crop_region(window_rect, dw, dh)
            .ok_or_else(|| anyhow::anyhow!("la fenêtre est hors de l'écran"))?;
        let (width, height) = (region.width, region.height);

        let mut encoder =
            H264Encoder::new(capture.device(), (width, height), (width, height), fps, bitrate)?;
        encoder.request_keyframe()?;

        Ok(Self::depuis_pieces(
            hwnd,
            capture,
            encoder,
            region,
            width,
            height,
            fps,
            bitrate,
            clock_origin,
            ModeCapture::FenetreRecadree,
        ))
    }

    // `sur_sortie` (mode du sous-bloc D1 : sortie DXGI entière, plus rien à
    // recadrer) est défini dans un second `impl WindowsSource`, situé dans
    // `windows_source/sortie.rs` sous `#[cfg(windows)]`. Déplacé hors d'ici en
    // revue pour que ce fichier — déjà en dette de taille (voir `CLAUDE.md`) —
    // ne porte que le câblage ; il se termine, comme `new` ci-dessus, par un
    // appel à `Self::depuis_pieces`, résolu par recherche de méthode inhérente
    // à travers tout le crate, indépendamment du fichier qui la définit.

    /// Assemblage final, partagé par les deux constructeurs (`new` ci-dessus et
    /// `sur_sortie`, dans `windows_source/sortie.rs`).
    ///
    /// Extrait pour que « capture du bureau + recadrage de la fenêtre » et
    /// « capture d'une sortie entière » ne divergent pas sur l'initialisation
    /// des champs — ils ne diffèrent que par la façon d'obtenir la capture,
    /// l'encodeur, la région, et par `mode`.
    ///
    /// **Reste ICI, dans le module qui déclare `WindowsSource`**, et c'est la
    /// correction d'une revue : l'avoir migrée dans le module frère
    /// `windows_source_sortie` obligeait à ouvrir les treize champs en
    /// `pub(crate)` pour que le littéral `Self { … }` y compile. `pub(crate)`
    /// sur cette fonction unique suffit à l'appel inter-module et n'expose
    /// aucun champ.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn depuis_pieces(
        hwnd: HWND,
        capture: DesktopCapture,
        encoder: H264Encoder,
        region: Rect,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
        clock_origin: std::time::Instant,
        mode: ModeCapture,
    ) -> Self {
        Self {
            hwnd,
            capture: Some(capture),
            region,
            encoder: Some(encoder),
            width,
            height,
            fps,
            bitrate,
            clock_origin,
            last_pts_90k: None,
            fatal: false,
            encoder_warmed_up: false,
            ready: std::collections::VecDeque::new(),
            mode,
            telemetrie: Telemetrie::default(),
        }
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

    /// Encodeur courant, mutable — ou une **erreur**, jamais une panique.
    ///
    /// La différence avec `capture_mut` ci-dessus est délibérée : `capture`
    /// n'est lue que par `next_frame`, derrière son garde `if self.fatal`, ce
    /// qui rend son `expect` inatteignable. L'encodeur, lui, est aussi lu par
    /// `request_keyframe` et `set_bitrate`, que la boucle de transport appelle
    /// sur un événement du navigateur **sans consulter `is_exhausted`** : entre
    /// l'échec de `set_encode_size` et la clôture de la session, un tel appel
    /// est possible, et une panique y emporterait tout le processus — donc
    /// toutes les autres fenêtres. Les deux savent quoi faire d'un `Err`.
    fn encoder_mut(&mut self) -> Result<&mut H264Encoder> {
        self.encoder
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("source épuisée : encodeur détruit"))
    }

    /// Vrai tant que la fenêtre capturée existe.
    pub fn is_alive(&self) -> bool {
        window::is_window_alive(self.hwnd)
    }

    /// Demande une image clé — notamment sur requête du navigateur, relayée
    /// depuis `Event::KeyframeRequest` par `transport/evenements.rs` via l'implémentation
    /// `VideoSource::request_keyframe` ci-dessous.
    pub fn request_keyframe(&mut self) -> Result<()> {
        self.encoder_mut()?.request_keyframe()
    }

    // `set_encode_size` (changement de la seule taille d'encodage) vit dans le
    // module enfant `encodage.rs` — voir sa déclaration en tête de fichier.

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
            // `None` : encodeur détruit par un `set_encode_size` en échec (voir
            // `encoder_mut`). Plus rien à drainer, mais ce qui est déjà en
            // file reste bon à rendre.
            match self.encoder.as_mut().map(H264Encoder::poll_output) {
                Some(Ok(Some(unit))) => {
                    self.encoder_warmed_up = true;
                    self.telemetrie.produite();
                    self.ready.push_back(unit);
                }
                Some(Ok(None)) | None => break,
                Some(Err(e)) => {
                    tracing::warn!(erreur = %e, "récupération de l'image encodée échouée");
                    break;
                }
            }
        }
        // Les emplacements d'entrée libérés par le drainage ci-dessus sont
        // réutilisables dès maintenant : ne pas attendre le tour suivant.
        if let Err(e) = self.encoder_mut().and_then(H264Encoder::flush_pending_inputs) {
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

// Les compteurs `TICKS`/`CAPTURED`/`PRODUCED` vivaient ici, en statiques de
// processus — mortes des deux côtés depuis D4. Remplacées par le champ
// `telemetrie` de `WindowsSource`, par session (voir `windows_source/telemetrie.rs`).

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
        self.telemetrie.tick();
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
                self.telemetrie.capturee();
                // Horodatage lu sur l'horloge réelle AVANT la soumission :
                // c'est l'instant de la capture qui date l'image, pas celui
                // où l'encodeur voudra bien l'accepter (voir `next_pts_90k`).
                let pts = self.next_pts_90k();
                let t_submit = std::time::Instant::now();
                let fed = self.encoder_mut().and_then(|encoder| encoder.submit(&frame, pts));
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
                // Une perte d'accès est déjà passée par les reprises de
                // `next_frame` : la recevoir ici signifie qu'elles n'ont pas
                // suffi. Fin légitime dans les deux cas.
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

    fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        // Mémorisé même en cas d'échec : c'est ce débit-là qu'une
        // reconstruction ultérieure de l'encodeur devra reprendre.
        self.bitrate = bitrate;
        self.encoder_mut()?.set_bitrate(bitrate)
    }

    fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        WindowsSource::set_encode_size(self, width, height)
    }
}
