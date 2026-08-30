//! Mise en route d'une session : capture, encodeur, source, `Session`
//! WebRTC et signalisation, assemblés dans cet ordre.

#[cfg(windows)]
use std::time::Duration;

use anyhow::Result;

use crate::signaling;
use crate::source::{FileSource, VideoSource};
use crate::transport::Session;
use crate::Config;
#[cfg(windows)]
use crate::{cursor, encode, gamepad, input};

/// Construction de la source vidéo Windows, extraite pour tenir le plafond de
/// 500 lignes de ce fichier — voir son commentaire de tête.
#[cfg(windows)]
mod source;

/// Construction et branchement de la source audio, extraite pour la même
/// raison (tâche 7 du sous-bloc D7) — voir son commentaire de tête.
#[cfg(windows)]
mod audio;

/// Le fil de trace du chemin réel (`SOURCE_TRACE=1`), extrait pour tenir le
/// plafond de 500 lignes de ce fichier — voir son commentaire de tête.
#[cfg(windows)]
mod trace;

/// Le puits de MESURE du micro (`MICRO_MESURE=1`, chantier E) — voir son
/// commentaire de tête, qui porte tout le raisonnement. **Sans
/// `#[cfg(windows)]`** : ce puits est pur, il se teste sur l'hôte.
pub(crate) mod micro;

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
        retry_apres_s,
        receiver_task: _,
        sender_task: _,
    } = signaling::run_signaling(
        &signaling::url_du_relais(&config.signaling_url),
        &config.session_id,
        config.jeton.as_deref(),
    )
    .await?;
    let mut session = Session::new(source, config.local_ip, clock_origin, bitrate)?;
    // Pour la ligne de cadence périodique de `piste_video` (voir sa doc) :
    // apparier ce relevé à celui du capteur dans un `agent.log` que
    // plusieurs fenêtres se partagent (voir `capteur/fenetre.rs`).
    session.set_session_id(&config.session_id);

    // Source audio : son absence ne compromet jamais la session vidéo — voir
    // le commentaire de tête de `demarrage::audio` pour le détail des deux
    // modes (mix de session, ou process loopback par fenêtre) et le repli
    // délibérément écarté.
    #[cfg(windows)]
    audio::brancher(&config, &mut session, clock_origin);

    // Sans `MICRO_MESURE=1` il ne pose rien : `micro_disponible()` reste faux,
    // `ready` porte `mic: false`, et le bouton du navigateur ne paraît pas —
    // ce qu'on veut tant que le vrai câble (bloc E2) n'existe pas.
    micro::brancher(&config, &mut session);

    // Le fil de trace du chemin réel (`SOURCE_TRACE=1`) vit dans
    // `demarrage/trace.rs` depuis le sous-bloc P2 du chantier presse-papier —
    // extraction préalable à l'addition, voir son commentaire de tête, qui porte
    // tout le raisonnement sur les compteurs qu'il lit et sur ceux qui sont morts.
    #[cfg(windows)]
    let _source_trace = trace::brancher();

    // 🔴 CORRECTIF DU LEGS DES FREINS MANQUANTS (round de correction 1,
    // critique ②) — même geste que `pont.rs::executer` : un refus de volume
    // (`trop-de-requetes`) ferme `offers` sans offre, ce processus va
    // mourir, et on honore le délai suggéré par le relais AVANT de rendre la
    // main — voir `signaling::honorer_retry_suggere`.
    //
    // 🔴 **DÉCLARÉ, PAS CORRIGÉ (revue, round de correction 2)** : CE
    // PROCESSUS-CI EST L'ENFANT D'UNE FENÊTRE, PAS LE PONT — et le sommeil
    // qui suit (jusqu'à 30 s, `REPLI_MAX_MS`) retarde d'AUTANT le moment où
    // `superviseur::table::orphelines::relancer_les_orphelines` voit cette
    // entrée redevenir `SansSession` et la relance. Avec `RELANCES_MAX = 3`
    // tolérées (`superviseur/table.rs`) et un refus qui se reproduit à
    // chaque relance, l'échec d'attache d'une fenêtre — le message « la
    // session n'a pas tenu après 3 tentatives » — peut donc mettre jusqu'à
    // quelques dizaines de secondes à quelques minutes à devenir visible à
    // l'utilisateur, au lieu de quelques secondes avant ce lot. Non mesuré
    // en recette ; le mécanisme, lui, est vérifiable par lecture croisée de
    // `orphelines.rs` et de ce fichier.
    let Some(offer) = offers.recv().await else {
        signaling::honorer_retry_suggere(&retry_apres_s).await;
        return Err(anyhow::anyhow!("le signaling s'est fermé avant l'offre"));
    };
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
    //
    // ⚠️ **Le second argument est le PRESSE-PAPIER, et il n'a rien à voir avec
    // la manette** : c'est `PRESSE_PAPIER`, lue par le même `actif()` que le
    // capteur. Les deux replis ci-dessous rejouent donc `actif()` et NON
    // `false` — un `capabilities(false, false)` recopié éteindrait le collage
    // parce qu'une manette manque, ce qui n'a aucun sens. La condition de
    // validité de cette annonce vit dans la doc du champ
    // (`proto/src/control.rs`, `Capabilities::clipboard`) : elle tient parce
    // que capteur et enfant lisent la MÊME variable héritée.
    #[cfg(windows)]
    let _ = control_tx.send(proto::control::AgentControl::capabilities(
        true,
        crate::presse_papier::actif(),
    ));

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
    // Clonée avant la fermeture `move` ci-dessous : c'est cette copie qui
    // donne à la trace `contrôle reçu` sa `session` (voir `on_control` plus
    // bas), faute de quoi elle est indiscernable de celle de tout autre
    // enfant partageant le même `agent.log` (D4).
    let session_id = config.session_id.clone();
    // Cloné AVANT le `move` : c'est la même valeur qui a choisi le mode de
    // capture plus haut, et c'est elle — et elle seule — qui doit choisir la
    // référence des entrées.
    let sortie_dxgi_entrees = config.sortie_dxgi.clone();
    let transport = tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        let mut injector = window_hwnd_addr.map(|addr| {
            let hwnd = windows::Win32::Foundation::HWND(addr as *mut core::ffi::c_void);
            // La référence des entrées DÉRIVE de `sortie_dxgi`, le même
            // discriminant que le mode de capture (`demarrage/source.rs`) :
            // deux descriptions indépendantes du même rectangle sont ce qui a
            // produit le défaut du lot 32M.
            input::InputInjector::new(hwnd, sortie_dxgi_entrees.as_deref(), mode_relatif.clone())
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
                            let _ = control_tx.send(
                                proto::control::AgentControl::capabilities(
                                    false,
                                    crate::presse_papier::actif(),
                                ),
                            );
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
                            let _ = control_tx.send(
                                proto::control::AgentControl::capabilities(
                                    false,
                                    crate::presse_papier::actif(),
                                ),
                            );
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
        // `session` : sans ce champ la trace n'est PAS attribuable — tous les enfants
        // héritent le même `agent.log` depuis D4. C'est exactement ce qui a rendu
        // indécidable « 2 `Resize` pour 5 sessions » (leg 10 de D8, correction I8) :
        // les deux lignes ne portaient aucune session, donc rien n'établissait
        // qu'elles vinssent de deux sessions distinctes.
        let mut on_control = |message| tracing::info!(session = %session_id, ?message, "contrôle reçu");
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
