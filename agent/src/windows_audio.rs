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

use crate::audio::{AudioPacket, AudioSource, PacketRing};
use crate::frames::FrameAssembler;
use crate::opus::OpusEncoder;
use crate::wasapi::process_loopback::CaptureProcessus;
use crate::wasapi::LoopbackCapture;

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

    /// Démarre ou arrête le flux. **Sans effet en mode session** : le loopback
    /// global n'est jamais arbitré — un agent mono-fenêtre porte toujours son
    /// son.
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
    emet: Arc<AtomicBool>,
    /// PID capté, pour la trace périodique. `None` en mode session.
    ///
    /// Non relu après construction : c'est `pid_fil`, une copie locale prise
    /// avant le déplacement de ce paramètre dans le fil de capture, qui
    /// alimente la trace. Conservé ici pour que l'origine du PID reste
    /// visible sur la valeur construite, malgré l'avertissement `dead_code`
    /// que cela vaut sous `cargo check --target x86_64-pc-windows-gnu`.
    #[allow(dead_code)]
    pid: Option<u32>,
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
    fn demarrer(mut capture: Capture, origin: Instant, pid: Option<u32>) -> Result<Self> {
        let description = capture.description();
        let mut encodeur = OpusEncoder::new().context("création de l'encodeur Opus")?;

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

        let ring_fil = ring.clone();
        let arret_fil = Arc::clone(&arret);
        let perte_desiree_fil = Arc::clone(&perte_desiree);
        let emet_fil = Arc::clone(&emet);
        // `Option<u32>` est `Copy` : cette copie locale est celle que le fil
        // emporte, indépendamment du champ `pid` de `Self` construit plus bas.
        let pid_fil = pid;
        std::thread::Builder::new()
            .name("audio-capture".into())
            .spawn(move || {
                // Ce fil appelle lui-même des méthodes COM — `read()` à chaque
                // tour, et `Stop()` via le `Drop` de `LoopbackCapture` en
                // sortant — alors que `open()` a initialisé COM sur le fil
                // APPELANT, pas sur celui-ci. Microsoft exige que tout fil
                // invoquant des méthodes COM ait d'abord rejoint un
                // appartement.
                //
                // Résultat volontairement ignoré ICI — contrairement à
                // `wasapi::open`, qui lui **vérifie** son `HRESULT` et refuse
                // `RPC_E_CHANGED_MODE` (voir son commentaire, dont dépend
                // `unsafe impl Send for LoopbackCapture`) : ce fil-ci vient
                // d'être créé par `thread::Builder::spawn` juste au-dessus,
                // il n'a donc encore rejoint aucun appartement COM, et
                // `CoInitializeEx` y rend nécessairement `S_OK`. `open()`,
                // lui, s'exécute sur un fil quelconque — potentiellement
                // recyclé, potentiellement déjà lié à une STA — d'où la
                // vérification qui n'a pas lieu d'être répétée ici.
                //
                // Symétriquement, PAS de `CoUninitialize` : voir le motif
                // détaillé dans `wasapi.rs`.
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }

                let mut assembleur = FrameAssembler::new(origin);
                let mut dernier_rapport = Instant::now();
                // Dernière valeur effectivement posée sur l'encodeur. Un
                // appel CTL par trame de 10 ms serait du gaspillage sur ce
                // chemin chaud : on ne réécrit que lorsque la cible a changé.
                let mut derniere_perte: i32 = 0;
                // Dernière valeur effectivement appliquée à `capture` : on ne
                // rappelle `Start()`/`Stop()` que lorsque l'ordre change,
                // jamais à chaque tour à ~200 Hz.
                let mut emettait = false;

                while !arret_fil.load(Ordering::Relaxed) {
                    let veut_emettre = emet_fil.load(Ordering::Relaxed);
                    if veut_emettre != emettait {
                        match capture.emettre(veut_emettre) {
                            Ok(()) => emettait = veut_emettre,
                            Err(e) => {
                                // Un refus ne tue pas la session : on
                                // journalise et on retentera au prochain
                                // changement d'ordre plutôt qu'à chaque tour.
                                emettait = veut_emettre;
                                tracing::warn!(
                                    erreur = %e,
                                    actif = veut_emettre,
                                    "bascule d'emission audio refusee"
                                );
                            }
                        }
                    }
                    if !emettait {
                        // Muette : ne rien lire, ne rien encoder, ne rien
                        // déposer. Une trame de silence encodée coûterait
                        // quelques octets grâce au DTX, mais elle arriverait
                        // au navigateur — et deux fenêtres d'un même processus
                        // s'entendraient toutes les deux.
                        std::thread::sleep(POLL_INTERVAL);
                        continue;
                    }

                    match capture.read() {
                        Ok(Some(bloc)) => assembleur.push(&bloc),
                        Ok(None) => {}
                        Err(e) => {
                            // Une erreur de lecture ne doit pas tuer la
                            // session : on journalise et on arrête l'audio.
                            // La vidéo continue. Les compteurs sont inclus
                            // ici parce que c'est la dernière ligne de log de
                            // ce fil : sans eux, une capture morte en cours
                            // de session serait indiscernable d'un simple
                            // silence — `next_packet` continuerait à rendre
                            // `None` comme dans le cas nominal.
                            tracing::warn!(
                                erreur = %e,
                                rejetes = ring_fil.rejetes(),
                                complements = assembleur.complements(),
                                echantillons_jetes = assembleur.echantillons_jetes(),
                                "lecture audio échouée, capture arrêtée définitivement"
                            );
                            return;
                        }
                    }

                    for trame in assembleur.drain_due(Instant::now()) {
                        let voulue = perte_desiree_fil.load(Ordering::Relaxed);
                        if voulue != derniere_perte {
                            match encodeur.set_packet_loss_perc(voulue) {
                                Ok(()) => derniere_perte = voulue,
                                Err(e) => {
                                    // Refus de l'encodeur : on retentera au
                                    // prochain changement de cible plutôt que
                                    // de rejouer cet appel à chaque trame.
                                    derniere_perte = voulue;
                                    tracing::warn!(
                                        erreur = %e,
                                        valeur = voulue,
                                        "réglage du taux de perte Opus refusé"
                                    );
                                }
                            }
                        }
                        match encodeur.encode(&trame.pcm) {
                            Ok(data) => ring_fil.push(AudioPacket {
                                data,
                                pts_48k: trame.pts_48k,
                                captured_at: trame.captured_at,
                            }),
                            Err(e) => {
                                // Même raisonnement que pour l'erreur de
                                // lecture ci-dessus : dernière ligne de log
                                // de ce fil, donc dernière chance de rendre
                                // les compteurs accumulés exploitables.
                                tracing::warn!(
                                    erreur = %e,
                                    rejetes = ring_fil.rejetes(),
                                    complements = assembleur.complements(),
                                    echantillons_jetes = assembleur.echantillons_jetes(),
                                    "encodage Opus échoué, capture arrêtée définitivement"
                                );
                                return;
                            }
                        }
                    }

                    if dernier_rapport.elapsed() >= REPORT_INTERVAL {
                        dernier_rapport = Instant::now();
                        // `info!`, pas `debug!` : le filtre par défaut
                        // (`agent/src/main.rs`, `EnvFilter` replié sur
                        // `"info"` en l'absence de `RUST_LOG`) n'émet jamais
                        // les journaux `debug!` en exploitation normale. La
                        // spec (§5, §9) promet des compteurs « journalisés
                        // périodiquement et jamais silencieux » — un
                        // enregistrement toutes les `REPORT_INTERVAL` (30 s)
                        // n'est pas du bruit, et un compteur de rejets muet
                        // est exactement ce qui rendrait une dégradation
                        // audio invisible en recette.
                        //
                        // `pid` et `actif` sont le seul moyen d'observer un
                        // arbitrage figé : si aucune fenêtre ne portait plus
                        // jamais le son, le symptôme serait le silence total
                        // sans un `WARN`, sans une erreur. C'est le `grep`
                        // d'entrée du sous-bloc suivant (spec §6).
                        tracing::info!(
                            pid = pid_fil,
                            actif = emettait,
                            rejetes = ring_fil.rejetes(),
                            complements = assembleur.complements(),
                            echantillons_jetes = assembleur.echantillons_jetes(),
                            "compteurs audio"
                        );
                    }

                    std::thread::sleep(POLL_INTERVAL);
                }
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
        })
    }

    /// Format de mixage obtenu, pour le journal.
    pub fn description(&self) -> &str {
        &self.description
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
}

impl Drop for WindowsAudioSource {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::Relaxed);
    }
}
