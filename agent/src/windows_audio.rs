//! Source audio Windows : capture loopback, découpage, encodage Opus.
//!
//! Tout se passe sur un fil dédié, qui dépose dans un tampon circulaire borné.
//! La boucle de transport n'y fait qu'un retrait non bloquant par tour : elle
//! ne doit jamais attendre WASAPI ni l'encodeur.

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::audio::{
    temporisation_de_reprise, AudioPacket, AudioSource, PacketRing, LECTURES_ECHOUEES_MAX,
};
use crate::frames::FrameAssembler;
use crate::opus::OpusEncoder;
use crate::wasapi::process_loopback::CaptureProcessus;
use crate::wasapi::LoopbackCapture;

// Le corps du fil de capture (tâche 3 du sous-bloc D10) : extrait côté
// production, pour rester sous le plafond de 500 lignes du projet et avant
// l'addition de la tâche 13 (`AUDIO_FAUTE_LECTURE`) qui l'aurait autrement
// fait franchir. Même schéma que `superviseur/boucle/creation_sortie.rs` et
// `capteur/serveur/instances.rs`.
mod fil;
use fil::tourner;

/// Profondeur du tampon partagé, en paquets de 10 ms. 10 paquets = 100 ms :
/// assez pour absorber un tour de boucle en retard, trop peu pour que la
/// latence s'installe.
const RING_CAPACITY: usize = 10;

/// Intervalle de sondage du fil de capture. Deux fois plus rapide que la
/// cadence des trames : la capture ne doit jamais être le facteur limitant.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Intervalle entre deux journaux de compteurs agrégés.
const REPORT_INTERVAL: Duration = Duration::from_secs(30);

/// Ce que le fil de capture lit, selon le mode.
///
/// **Deux chemins, un seul fil.** Le mode global (`Session`) est celui d'avant
/// D7 et sert le cas mono-fenêtre — un agent lancé à la main, sans
/// `FENETRE_HWND`. Le mode `Processus` est celui du multi-fenêtres.
enum Capture {
    Session(LoopbackCapture),
    Processus(CaptureProcessus),
}

impl Capture {
    fn read(&mut self) -> Result<Option<Vec<i16>>> {
        match self {
            Capture::Session(c) => c.read(),
            Capture::Processus(c) => c.read(),
        }
    }

    fn description(&self) -> String {
        match self {
            Capture::Session(c) => c.description().to_string(),
            Capture::Processus(c) => c.description().to_string(),
        }
    }

    /// Démarre ou arrête le flux. **Sans effet en mode session** sur le flux
    /// WASAPI lui-même : le loopback global n'est jamais démarré/arrêté par
    /// cette méthode — un agent mono-fenêtre porte toujours son son.
    ///
    /// ⚠️ Cette immunité ne s'étend PAS au fil appelant : c'est lui qui gate
    /// lecture/encodage/dépôt sur la valeur d'`emettait` (voir `demarrer`),
    /// pas cette méthode. Un `set_actif(false)` qui atteindrait une source en
    /// mode `Session` la rendrait donc tout aussi muette qu'une source en
    /// mode `Processus` — ~~seul `new()` (qui n'expose jamais
    /// l'`Arc<AtomicBool>` à un ordre externe) rend ce cas inatteignable
    /// aujourd'hui~~.
    ///
    /// ❌ **CE CAS EST DEVENU ATTEIGNABLE AU SOUS-BLOC D10** (revue
    /// transverse) : `Session::reconstruire_ou_signaler` applique
    /// `set_actif(self.audio_porteuse)` **sans condition** à toute source
    /// reconstruite, et le reconstructeur de `demarrage/audio.rs` passe par
    /// `WindowsAudioSource::new` — donc par `Capture::Session` — quand
    /// `config.fenetre_hwnd` est `None`. Un ordre externe atteint donc bien
    /// une source en mode session.
    ///
    /// ✅ **« Et il la fait taire » était écrit ici, et c'est FAUX depuis le
    /// sous-bloc D11** (revue transverse) : `audio_porteuse` vaut désormais
    /// `true` en mono-fenêtre, posé par `set_audio_porteuse` au branchement
    /// (`demarrage/audio.rs`), et le réarmement RÉÉMET cette source au lieu
    /// de la faire taire. **L'atteignabilité reste vraie, sa conséquence ne
    /// l'est plus.** Voir `Session::reconstruire_ou_signaler`.
    fn emettre(&mut self, actif: bool) -> Result<()> {
        match self {
            Capture::Session(_) => Ok(()),
            Capture::Processus(c) => {
                if actif {
                    c.demarrer()
                } else {
                    c.arreter()
                }
            }
        }
    }
}

pub struct WindowsAudioSource {
    ring: PacketRing,
    arret: Arc<AtomicBool>,
    description: String,
    /// Taux de perte voulu par le contrôleur de congestion, en pourcentage.
    /// Lu par le fil de capture avant chaque encodage — voir son commentaire
    /// dans `new` pour la raison d'être de cet indirection : l'`OpusEncoder`
    /// lui-même est déplacé dans ce fil et n'est donc pas accessible ici.
    perte_desiree: Arc<AtomicI32>,
    /// Ordre d'émission voulu par l'arbitrage du capteur, lu par le fil de
    /// capture avant chaque tour. Même patron d'indirection que
    /// `perte_desiree` : la capture vit sur le fil, pas ici.
    ///
    /// **Faux à la naissance.** L'enfant naît MUET et n'émet que sur ordre du
    /// capteur — même doctrine que `SourceDistante::endormie`, qui naît à
    /// `true`. C'est ce qui évite que deux fenêtres d'un même processus soient
    /// toutes deux audibles pendant les millisecondes qui précèdent le premier
    /// arbitrage.
    ///
    /// ⚠️ **« À la naissance » veut dire à CHAQUE appel de `demarrer` — donc
    /// aussi à chaque RECONSTRUCTION**, pas seulement à l'ouverture initiale
    /// (défaut trouvé en recette VM, sous-bloc D10 : `capture audio
    /// reconstruite` = 2, `compteurs_audio_actif_true` = 0 aux deux
    /// exécutions). `pour_processus` ne s'auto-émet jamais, à la
    /// différence de `new()` (mode session), qui s'émet lui-même
    /// juste après construction.
    ///
    /// ❌ **« Le seul chemin qu'emprunte un reconstructeur » était écrit ici,
    /// et c'est faux : `demarrage/audio.rs` en pose un dans les DEUX modes.**
    /// La conséquence — en mono-fenêtre le réarmement RETIRAIT le son que
    /// `new()` venait de donner, faute d'ordre du capteur pour poser
    /// `audio_porteuse` — est documentée auprès de
    /// `Session::reconstruire_ou_signaler` (`transport/piste_audio.rs`).
    /// ✅ **« Et léguée » : plus depuis le sous-bloc D11** (leg 4, mesuré en
    /// recette VM — 441 Hz reçus contre la sentinelle au bras rouge).
    /// `demarrage/audio.rs::brancher` pose `audio_porteuse = true` dans sa
    /// seule branche mono-fenêtre. C'est
    /// `Session::reconstruire_ou_signaler` (`transport/piste_audio.rs`) qui
    /// réarme désormais une source reconstruite, sur `audio_porteuse` — sans
    /// quoi une capture reconstruite pour une fenêtre porteuse restait
    /// muette pour toujours (aucun paquet → aucune preuve → aucune
    /// réélection → muette, un état ABSORBANT).
    emet: Arc<AtomicBool>,
    /// PID capté. `None` en mode session. Exposé par `pid()`, au journal
    /// d'ouverture (`demarrage/audio.rs`) — c'est `pid_fil`, une copie locale
    /// prise avant le déplacement de ce paramètre dans le fil de capture, qui
    /// alimente la trace périodique « compteurs audio ».
    pid: Option<u32>,
    /// Posé par le fil de capture juste avant qu'il ne se termine pour de bon,
    /// et jamais effacé — sa mort est sans retour. Lu par `appliquer_audio`,
    /// qui annoncerait sinon `actif=true` pour une fenêtre qui ne produira plus
    /// jamais un paquet (F3, revue finale de branche du sous-bloc D7 ; voir
    /// `AudioSource::capture_morte`).
    capture_morte: Arc<AtomicBool>,
}

impl WindowsAudioSource {
    /// Ouvre le loopback GLOBAL de la session et démarre le fil de production.
    ///
    /// Mode mono-fenêtre : un agent lancé à la main, sans `FENETRE_HWND`. Le
    /// son est porté sans arbitrage — il n'y a personne avec qui le partager.
    ///
    /// `origin` est l'origine d'horloge **de la session**, partagée avec la
    /// source vidéo : c'est elle qui rend les deux lignes de temps
    /// comparables, donc la synchro A/V exacte.
    pub fn new(origin: Instant) -> Result<Self> {
        let capture = LoopbackCapture::open().context("ouverture du loopback audio")?;
        let source = Self::demarrer(Capture::Session(capture), origin, None)?;
        // Aucun capteur n'enverra jamais d'ordre à cet agent : il émet d'emblée.
        source.emettre(true);
        Ok(source)
    }

    /// Ouvre le loopback du PROCESSUS `pid` et de son arbre, et démarre le fil
    /// de production.
    ///
    /// **La source naît MUETTE** : c'est le capteur qui décide qui porte le
    /// son, et son premier ordre arrive dès l'attache. Voir le champ `emet`.
    pub fn pour_processus(pid: u32, origin: Instant) -> Result<Self> {
        let capture = CaptureProcessus::ouvrir(pid)
            .with_context(|| format!("ouverture du process loopback du PID {pid}"))?;
        Self::demarrer(Capture::Processus(capture), origin, Some(pid))
    }

    /// Corps commun aux deux constructeurs : démarre le fil de production à
    /// partir d'une capture déjà ouverte, quel que soit son mode.
    fn demarrer(capture: Capture, origin: Instant, pid: Option<u32>) -> Result<Self> {
        let description = capture.description();
        let encodeur = OpusEncoder::new().context("création de l'encodeur Opus")?;

        let ring = PacketRing::new(RING_CAPACITY);
        let arret = Arc::new(AtomicBool::new(false));
        // Pont entre `AudioSource::set_packet_loss_perc` (appelé depuis la
        // boucle de transport) et l'encodeur Opus, qui vit sur le fil de
        // capture et n'est donc accessible que depuis lui. Un `Mutex` autour
        // de l'encodeur serait pris à chaque trame de 10 ms sur ce chemin
        // chaud ; un entier atomique lu une fois par trame ne coûte rien.
        let perte_desiree = Arc::new(AtomicI32::new(0));
        // Même patron : voir la doc du champ `emet`.
        let emet = Arc::new(AtomicBool::new(false));
        // Même patron encore : voir la doc du champ `capture_morte`.
        let capture_morte = Arc::new(AtomicBool::new(false));

        let ring_fil = ring.clone();
        let arret_fil = Arc::clone(&arret);
        let perte_desiree_fil = Arc::clone(&perte_desiree);
        let emet_fil = Arc::clone(&emet);
        let capture_morte_fil = Arc::clone(&capture_morte);
        // `Option<u32>` est `Copy` : cette copie locale est celle que le fil
        // emporte, indépendamment du champ `pid` de `Self` construit plus bas.
        let pid_fil = pid;
        std::thread::Builder::new()
            .name("audio-capture".into())
            .spawn(move || {
                tourner(
                    capture,
                    encodeur,
                    origin,
                    ring_fil,
                    arret_fil,
                    perte_desiree_fil,
                    emet_fil,
                    capture_morte_fil,
                    pid_fil,
                )
            })
            .context("démarrage du fil de capture audio")?;

        tracing::info!(format = %description, "source audio démarrée");
        Ok(Self {
            ring,
            arret,
            description,
            perte_desiree,
            emet,
            pid,
            capture_morte,
        })
    }

    /// Format de mixage obtenu, pour le journal.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// PID capté, pour le journal d'ouverture. `None` en mode session.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Porte le son, ou se tait. Appelée depuis la boucle de transport, qui
    /// consomme l'ordre du capteur.
    pub fn emettre(&self, actif: bool) {
        self.emet.store(actif, Ordering::Relaxed);
    }
}

impl AudioSource for WindowsAudioSource {
    fn next_packet(&mut self) -> Option<AudioPacket> {
        self.ring.pop()
    }

    fn set_packet_loss_perc(&mut self, perc: i32) -> Result<()> {
        // Ne fait qu'écrire : c'est le fil de capture qui lit cette valeur et
        // relaie vers `OpusEncoder::set_packet_loss_perc`, seul détenteur de
        // l'encodeur (voir le commentaire du champ `perte_desiree`).
        self.perte_desiree.store(perc, Ordering::Relaxed);
        Ok(())
    }

    fn set_actif(&mut self, actif: bool) {
        self.emettre(actif);
    }

    fn capture_morte(&self) -> bool {
        self.capture_morte.load(Ordering::Relaxed)
    }
}

impl Drop for WindowsAudioSource {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::Relaxed);
    }
}
