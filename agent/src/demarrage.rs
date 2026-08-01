//! Mise en route d'une session : capture, encodeur, source, `Session`
//! WebRTC et signalisation, assemblés dans cet ordre.

#[cfg(windows)]
use std::time::Duration;

use anyhow::{Context, Result};

use crate::signaling;
use crate::source::{FileSource, VideoSource};
use crate::transport::Session;
use crate::Config;
#[cfg(windows)]
use crate::{capture, cursor, encode, gamepad, input, windows_audio, windows_source};

/// Construction de la source vidéo Windows, extraite pour tenir le plafond de
/// 500 lignes de ce fichier — voir son commentaire de tête.
#[cfg(windows)]
mod source;

pub(crate) async fn executer(config: Config) -> Result<()> {
    // Renseigné dans la branche Windows ci-dessous : la fenêtre capturée est
    // aussi celle qui reçoit les entrées injectées (tâche 12). `None` en
    // mode fichier de test (pas de fenêtre Windows à piloter) ou hors
    // Windows.
    //
    // Conservé sous forme d'adresse brute (`isize`, qui est `Send`) plutôt
    // que de `HWND` directement : `HWND` enveloppe un `*mut c_void`, non
    // `Send` en windows-rs 0.62, et ne peut donc pas traverser tel quel la
    // fermeture `move` de `spawn_blocking` ci-dessous. Un HWND n'est qu'un
    // identifiant opaque (pas un pointeur réellement déréférencé côté
    // processus), le faire transiter par son adresse et le reconstruire
    // dans le fil cible est sûr — même technique que le fil d'agitation de
    // fenêtre du mode diagnostic `CAPTURE_TEST` (voir
    // `diagnostics/capture.rs`).
    #[cfg(windows)]
    let mut window_hwnd_addr: Option<isize> = None;

    // Origine d'horloge unique de la session. Les deux médias l'utilisent :
    // c'est ce qui rend leurs lignes de temps comparables, et donc la synchro
    // A/V exacte par construction. La créer ici, une seule fois, garantit
    // qu'aucune durée d'initialisation ne les décale l'une de l'autre.
    let clock_origin = std::time::Instant::now();

    // Plafond de débit vidéo, en bits par seconde. Lu depuis `BITRATE` dans
    // la branche Windows ci-dessous (seule branche où l'environnement a un
    // sens — la source de test ne pilote pas d'encodeur matériel) ; sinon la
    // valeur par défaut. Remonté ici, hors de cette branche, pour que
    // `Session::new` reçoive le même plafond que celui appliqué à
    // l'encodeur, sans le relire une seconde fois depuis l'environnement.
    // `mut` n'est utile que dans la branche `#[cfg(windows)]` ci-dessous :
    // sur l'hôte de test (Linux, toujours `TEST_FILE`), la valeur ne varie
    // jamais, d'où l'`allow` — inutile de découper la déclaration par cfg
    // pour une valeur qui reste de toute façon lue plus bas sur les deux
    // plateformes.
    #[allow(unused_mut)]
    let mut bitrate: u32 = 12_000_000;

    let source: Box<dyn VideoSource + Send> = match &config.test_file {
        Some(path) => {
            tracing::info!(?path, "source de test");
            Box::new(FileSource::from_path(path, 1280, 720, 60)?)
        }
        None => {
            #[cfg(windows)]
            {
                let construite = source::construire(&config, clock_origin)?;
                window_hwnd_addr = Some(construite.hwnd_addr);
                bitrate = construite.bitrate;
                construite.source
            }
            #[cfg(not(windows))]
            {
                anyhow::bail!("TEST_FILE est requis hors Windows")
            }
        }
    };

    // `receiver_task`/`sender_task` : conservés par `SignalingHandle` pour ne
    // pas être abandonnés silencieusement (I6), mais cette tâche mono-session
    // n'a rien de plus à en faire une fois `closed` observé ci-dessous — on
    // les laisse donc détachés explicitement plutôt que de les ignorer par
    // accident.
    let signaling::SignalingHandle {
        mut offers,
        answers,
        mut closed,
        ice_config,
        receiver_task: _,
        sender_task: _,
    } = signaling::run_signaling(&config.signaling_url, &config.session_id).await?;
    let mut session = Session::new(source, config.local_ip, clock_origin, bitrate)?;

    // Source audio : son absence ne compromet jamais la session vidéo. Sur une
    // source de test (TEST_FILE), il n'y a rien à capter. Hors Windows, il n'y
    // a pas de WASAPI. Et si le loopback refuse de s'ouvrir — pas de
    // périphérique de rendu par défaut, format de mixage non supporté — on
    // journalise et la session continue, muette.
    //
    // `config.audio` en plus : en multi-fenêtres, une seule fenêtre porte le
    // son (la table le réserve à la première détectée). Sans cette garde, huit
    // enfants ouvriraient huit captures loopback du MÊME périphérique et le
    // navigateur recevrait le son en huit exemplaires.
    #[cfg(windows)]
    if config.test_file.is_none() && config.audio {
        match windows_audio::WindowsAudioSource::new(clock_origin) {
            Ok(source_audio) => {
                tracing::info!(format = source_audio.description(), "audio activé");
                session.set_audio_source(Box::new(source_audio));
            }
            Err(e) => {
                tracing::warn!(erreur = %e, "audio indisponible, la session continue sans son");
            }
        }
    }
    #[cfg(windows)]
    if !config.audio {
        tracing::info!("son désactivé sur cet enfant : une seule fenêtre le porte");
    }

    // Surveillance du chemin réel (`SOURCE_TRACE=1`) : cadence d'appel de
    // `next_frame`, captures neuves, unités d'accès produites. Contrairement
    // à `watch_encoder`, ces compteurs survivent à un redimensionnement (qui
    // remplace l'encodeur, donc sa télémétrie) — c'est justement le cas qu'il
    // faut pouvoir observer.
    #[cfg(windows)]
    let _source_trace = std::env::var("SOURCE_TRACE").is_ok().then(|| {
        std::thread::spawn(|| {
            use std::sync::atomic::Ordering::Relaxed;
            let (mut t0, mut c0, mut p0, mut a0, mut h0, mut ac0) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
            let (mut ni0, mut ei0, mut ds0) = (0u64, 0u64, 0u64);
            let (mut cn0, mut sn0, mut dn0) = (0u64, 0u64, 0u64);
            let (mut cv0, mut in0, mut out0) = (0u64, 0u64, 0u64);
            let (mut ci0, mut co0, mut cs0) = (0u64, 0u64, 0u64);
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let (t, c, p) = (
                    windows_source::TICKS.load(Relaxed),
                    windows_source::CAPTURED.load(Relaxed),
                    windows_source::PRODUCED.load(Relaxed),
                );
                let (a, h, ac) = (
                    capture::ATTEMPTS.load(Relaxed),
                    capture::HITS.load(Relaxed),
                    capture::ACCUMULATED.load(Relaxed),
                );
                let (ni, ei, ds) = (
                    encode::NEED_INPUT_EVENTS.load(Relaxed),
                    encode::ENCODER_INPUTS.load(Relaxed),
                    encode::DROPPED_STALE.load(Relaxed),
                );
                let (cap_ns, sub_ns, dr_ns) = (
                    windows_source::CAPTURE_NS.load(Relaxed),
                    windows_source::SUBMIT_NS.load(Relaxed),
                    windows_source::DRAIN_NS.load(Relaxed),
                );
                let (ci, co, cs) = (
                    encode::CONVERTER_INPUTS.load(Relaxed),
                    encode::CONVERTER_OUTPUTS.load(Relaxed),
                    encode::CONVERTER_SKIPPED.load(Relaxed),
                );
                let (cv_ns, in_ns, out_ns) = (
                    encode::CONVERT_NS.load(Relaxed),
                    encode::ENC_IN_NS.load(Relaxed),
                    encode::ENC_OUT_NS.load(Relaxed),
                );
                // Part de la fenêtre d'observation (2 s = 2e9 ns) réellement
                // passée dans chaque appel : c'est ce qui distingue un étage
                // qui sature d'un étage qui attend.
                let pct = |now: u64, prev: u64| (now - prev) as f64 / 2e9 * 100.0;
                tracing::info!(
                    ticks_hz = (t - t0) as f64 / 2.0,
                    captured_hz = (c - c0) as f64 / 2.0,
                    produced_hz = (p - p0) as f64 / 2.0,
                    acquire_hz = (a - a0) as f64 / 2.0,
                    hits_hz = (h - h0) as f64 / 2.0,
                    // Mises à jour du bureau réellement survenues, y compris
                    // celles que DXGI a fusionnées : c'est ce chiffre qui dit
                    // si la fenêtre produit plus que ce qu'on en récupère.
                    desktop_updates_hz = (ac - ac0) as f64 / 2.0,
                    need_input_hz = (ni - ni0) as f64 / 2.0,
                    encoder_inputs_hz = (ei - ei0) as f64 / 2.0,
                    dropped_stale_hz = (ds - ds0) as f64 / 2.0,
                    conv_in_hz = (ci - ci0) as f64 / 2.0,
                    conv_out_hz = (co - co0) as f64 / 2.0,
                    conv_skipped_hz = (cs - cs0) as f64 / 2.0,
                    capture_pct = pct(cap_ns, cn0),
                    submit_pct = pct(sub_ns, sn0),
                    drain_pct = pct(dr_ns, dn0),
                    convert_pct = pct(cv_ns, cv0),
                    enc_in_pct = pct(in_ns, in0),
                    enc_out_pct = pct(out_ns, out0),
                    "cadence de la source (chemin réel)"
                );
                (t0, c0, p0, a0, h0, ac0) = (t, c, p, a, h, ac);
                (ni0, ei0, ds0) = (ni, ei, ds);
                (cn0, sn0, dn0) = (cap_ns, sub_ns, dr_ns);
                (cv0, in0, out0) = (cv_ns, in_ns, out_ns);
                (ci0, co0, cs0) = (ci, co, cs);
            }
        })
    });

    let offer = offers
        .recv()
        .await
        .context("le signaling s'est fermé avant l'offre")?;
    tracing::info!("offre reçue");

    // Le client web n'a pas de trickle ICE : il envoie UNE offre après
    // collecte complète et attend UNE réponse. Le candidat relayé doit donc
    // exister AVANT que la réponse ne soit produite — après, il n'y a plus
    // aucun moyen de le transmettre.
    //
    // Borné à 2 s : très en deçà des 15 s au bout desquelles le client
    // abandonne (`ANSWER_TIMEOUT_MS` de `client/src/webrtc.ts`), et suffisant
    // pour les deux aller-retours d'une allocation authentifiée (Allocate nu →
    // 401 → Allocate signé).
    // Pleinement qualifié : l'import de `Duration` en tête de fichier est
    // conditionné à Windows, et cette séquence-ci est commune aux deux cibles.
    const DELAI_ALLOCATION: std::time::Duration = std::time::Duration::from_secs(2);

    if let Some(config) = ice_config.borrow().clone() {
        match session.allouer_relais(config, DELAI_ALLOCATION) {
            Ok(()) => tracing::info!("relais TURN alloué avant la réponse SDP"),
            Err(e) => tracing::warn!(
                erreur = %e,
                "allocation TURN impossible : la session continue sans relais"
            ),
        }
    }

    let answer = session.accept_offer(&offer)?;
    answers.send(answer).await?;
    tracing::info!("réponse envoyée");
    // Note : `AgentControl::ready` n'est plus envoyé ici. À cet instant SCTP
    // n'est pas encore ouvert (le canal de contrôle vaut encore `None`), donc
    // l'envoyer maintenant serait silencieusement perdu (I3 de la revue).
    // `Session` le met en file elle-même dès `Event::ChannelOpen("control")`.

    // I6 : une perte du signaling après l'échange initial doit être visible
    // plutôt que silencieuse. Le transport ne dépend plus du signaling une
    // fois l'offre/réponse échangées (pas de renégociation dans cette
    // tâche), donc on ne fait rien de plus qu'observer et journaliser — mais
    // on l'observe.
    tokio::spawn(async move {
        if closed.changed().await.is_ok() && *closed.borrow() {
            tracing::warn!(
                "connexion de signaling perdue (aucune renégociation possible pour cette session)"
            );
        }
    });

    // Fil de sondage du curseur : décide du mode absolu/relatif et de la
    // forme à afficher. Le drapeau est partagé avec l'injecteur d'entrées,
    // les messages passent par la session (canal de contrôle).
    #[cfg_attr(not(windows), allow(unused_variables))]
    let mode_relatif = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let arret_sondes = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    // `control_tx` n'a de lecteur (`cursor::spawn_probe`) que sous Windows :
    // même raison que `mode_relatif` ci-dessus, même traitement.
    #[cfg_attr(not(windows), allow(unused_variables))]
    let (control_tx, control_rx) = std::sync::mpsc::channel();
    session.set_control_source(control_rx);

    #[cfg(windows)]
    let sonde_curseur = cursor::spawn_probe(
        control_tx.clone(),
        mode_relatif.clone(),
        arret_sondes.clone(),
    );

    // Vrai à ce stade : ViGEmBus n'est sondé qu'au premier état de manette
    // reçu, et l'échec éventuel enverra un second `Capabilities` à false.
    // Annoncer l'optimisme évite d'afficher « manette indisponible » à un
    // utilisateur qui n'en a simplement pas branché.
    #[cfg(windows)]
    let _ = control_tx.send(proto::control::AgentControl::capabilities(true));

    // Clone dédiée au fil de transport ci-dessous (`spawn_blocking` est
    // `move` : il faut lui donner sa propre copie de l'`Arc`, faute de quoi
    // il capturerait `arret_sondes` en entier et la rendrait indisponible
    // pour `arret_sondes.store(...)` après `transport.await`, plus bas).
    #[cfg_attr(not(windows), allow(unused_variables))]
    let arret_sondes_manette = arret_sondes.clone();

    // I6 : `Session::run` bloque volontairement (lecture UDP synchrone bornée
    // par la cadence vidéo et les échéances str0m). L'exécuter sur un ouvrier
    // async de tokio gèlerait les autres tâches de ce processus — ici, la
    // boucle d'émission du signaling — jusqu'à une seconde par tour, voire
    // beaucoup plus dès que la session n'est plus vivante. On la déplace donc
    // sur le pool de threads bloquants de tokio, dédié à cet usage.
    let transport = tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        let mut injector = window_hwnd_addr.map(|addr| {
            let hwnd = windows::Win32::Foundation::HWND(addr as *mut core::ffi::c_void);
            input::InputInjector::new(hwnd, mode_relatif.clone())
        });

        // Branchement paresseux : à la PREMIÈRE réception d'un état de
        // manette, pas au démarrage — voir le commentaire de
        // `gamepad::win::VirtualPad`. `pad_indisponible` garantit qu'on ne
        // retente qu'une fois : au premier échec, on renonce pour le reste
        // de la session plutôt que de retenter à chaque état reçu.
        #[cfg(windows)]
        let mut pad: Option<gamepad::VirtualPad> = None;
        #[cfg(windows)]
        let mut pad_indisponible = false;
        // `gamepad::VirtualPad::connect()` peut dormir jusqu'à 5 s (attente
        // de l'énumération PnP côté Windows, voir sa documentation) : on ne
        // l'appelle donc JAMAIS directement ici, cette fermeture tournant
        // dans la boucle de `Session::run` qui porte aussi vidéo et RTCP
        // (voir le commentaire sur `spawn_blocking` plus haut). `spawn_connect`
        // le fait sur un fil séparé ; ce récepteur est sondé sans bloquer.
        #[cfg(windows)]
        let mut connexion_manette: Option<std::sync::mpsc::Receiver<anyhow::Result<gamepad::VirtualPad>>> =
            None;

        let mut on_input = |message: proto::input::InputMessage| {
            #[cfg(windows)]
            if let proto::input::InputMessage::Gamepad(state) = message {
                if pad.is_none() && !pad_indisponible {
                    let rx = connexion_manette.get_or_insert_with(gamepad::spawn_connect);
                    match rx.try_recv() {
                        Ok(Ok(mut nouveau)) => {
                            match gamepad::spawn_rumble(
                                &mut nouveau,
                                control_tx.clone(),
                                arret_sondes_manette.clone(),
                            ) {
                                Ok(_) => {}
                                Err(e) => tracing::warn!(erreur = %e, "vibrations indisponibles"),
                            }
                            pad = Some(nouveau);
                            connexion_manette = None;
                        }
                        Ok(Err(e)) => {
                            // Une seule fois : `pad_indisponible` empêche tout
                            // nouvel essai, et donc tout second envoi de
                            // `Capabilities` pour cette session.
                            pad_indisponible = true;
                            connexion_manette = None;
                            tracing::warn!(erreur = %e, "manette virtuelle indisponible");
                            let _ = control_tx
                                .send(proto::control::AgentControl::capabilities(false));
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            // Connexion encore en cours (jusqu'à 5 s
                            // observées) : cet état de manette est perdu,
                            // sans conséquence — pas grâce à la fréquence de
                            // sondage du client (cadence sous charge du
                            // `setInterval(4 ms)` jamais mesurée), mais parce
                            // qu'il réémet un état complet toutes les 100 ms
                            // même sans changement jusqu'à ce que la cible
                            // soit prête (voir `client/src/gamepad.ts`,
                            // `RAFRAICHISSEMENT_MS`).
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            pad_indisponible = true;
                            connexion_manette = None;
                            tracing::warn!(
                                "fil de connexion à la manette virtuelle interrompu de façon inattendue"
                            );
                            let _ = control_tx
                                .send(proto::control::AgentControl::capabilities(false));
                        }
                    }
                }
                if let Some(pad) = pad.as_mut() {
                    if let Err(e) = pad.apply(&state) {
                        tracing::warn!(erreur = %e, "application de l'état de manette échouée");
                    }
                }
                return;
            }

            // Seul journal d'entrée disponible : il était auparavant gardé
            // par `#[cfg(not(windows))]`, donc mort sur la cible réelle — la
            // recette du chantier B (mesure 4) a dû s'en passer et
            // reconstituer la preuve autrement (instrumentation du canal
            // côté client). Le rendre disponible sous Windows aussi permet
            // au prochain diagnostic de lire directement `agent.log`.
            tracing::debug!(?message, "entrée reçue");

            #[cfg(windows)]
            if let Some(injector) = injector.as_mut() {
                if let Err(e) = injector.inject(message) {
                    tracing::warn!(erreur = %e, "injection d'entrée échouée");
                }
            }
        };
        let mut on_control = |message| tracing::info!(?message, "contrôle reçu");
        session.run(&mut on_input, &mut on_control)
    });

    match transport.await {
        Ok(Ok(())) => tracing::info!("session terminée"),
        Ok(Err(e)) => {
            // `Session::run` ne remonte une erreur que pour un problème jugé
            // irrécupérable au niveau de la session (voir
            // `Session::begin_ending` pour ce qui est au contraire traité
            // comme une fin de session propre, via `Ok(())`).
            tracing::error!(erreur = %e, "erreur fatale dans la boucle de transport");
            return Err(e);
        }
        Err(join_err) => {
            tracing::error!(erreur = %join_err, "la boucle de transport a paniqué");
            return Err(join_err.into());
        }
    }

    arret_sondes.store(true, std::sync::atomic::Ordering::Relaxed);
    #[cfg(windows)]
    let _ = sonde_curseur.join();

    Ok(())
}

/// Fil de surveillance du pipeline d'encodage : journalise chaque seconde
/// l'étape Media Foundation en cours et les compteurs du chemin chaud.
///
/// Indispensable pour distinguer un appel qui ne rend JAMAIS la main (l'étape
/// reste figée sur le même nom) d'une boucle qui tourne sans progresser
/// (l'étape varie, les compteurs non). Aucune trace posée *autour* des appels
/// ne peut faire cette distinction, puisqu'un appel bloqué n'atteint jamais sa
/// trace de sortie — c'est exactement ce qui a rendu le blocage du 28/07
/// invisible pendant plusieurs cycles d'investigation.
///
/// Renvoie de quoi l'arrêter : appeler la closure rendue rejoint le fil.
///
/// `pub(crate)` et non `pub(super)` : seul `diagnostics::capture` l'appelle
/// aujourd'hui, depuis l'autre branche de l'arborescence de modules.
#[cfg(windows)]
pub(crate) fn watch_encoder(telemetry: std::sync::Arc<encode::EncoderTelemetry>) -> impl FnOnce() {
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let handle = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        while !flag.load(Relaxed) {
            std::thread::sleep(Duration::from_secs(1));
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                etape = encode::phase_name(telemetry.phase.load(Relaxed)),
                submit_calls = telemetry.submit_calls.load(Relaxed),
                need_input_events = telemetry.need_input_events.load(Relaxed),
                have_output_events = telemetry.have_output_events.load(Relaxed),
                converter_inputs = telemetry.converter_inputs.load(Relaxed),
                converter_outputs = telemetry.converter_outputs.load(Relaxed),
                encoder_inputs = telemetry.encoder_inputs.load(Relaxed),
                encoder_outputs = telemetry.encoder_outputs.load(Relaxed),
                queued_nv12 = telemetry.queued_nv12.load(Relaxed),
                pending_input_requests = telemetry.pending_input_requests.load(Relaxed),
                skipped_busy = telemetry.skipped_busy.load(Relaxed),
                awaiting_drain = telemetry.awaiting_drain.load(Relaxed),
                conv_in_status = telemetry.converter_input_status.load(Relaxed),
                conv_out_status = telemetry.converter_output_status.load(Relaxed),
                conv_refus = telemetry.converter_not_accepting.load(Relaxed),
                "surveillance du pipeline d'encodage"
            );
        }
    });
    move || {
        stop.store(true, Relaxed);
        let _ = handle.join();
    }
}
