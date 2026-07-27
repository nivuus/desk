mod geometry;
mod h264;
mod signaling;
mod source;
mod transport;

#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod encode;
#[cfg(windows)]
mod window;

use std::net::IpAddr;
use std::path::PathBuf;
#[cfg(windows)]
use std::time::Duration;

use anyhow::{Context, Result};

use crate::source::{FileSource, VideoSource};
use crate::transport::Session;

/// Lit un pixel BGRA d'une texture GPU en la copiant vers une texture
/// « staging » accessible au CPU (`D3D11_USAGE_STAGING`).
///
/// Sert uniquement au mode diagnostic `CAPTURE_TEST` : prouver que le
/// recadrage capture bien le contenu de la fenêtre, et pas juste des
/// dimensions qui auraient l'air correctes sans l'être (voir l'appelant).
/// Renvoie `(r, g, b, a)`.
#[cfg(windows)]
fn read_pixel(
    device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
    texture: &windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> Result<(u8, u8, u8, u8)> {
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
        D3D11_USAGE_STAGING,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut staging = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) }
        .context("allocation de la texture de lecture")?;
    let staging = staging.context("texture de lecture absente")?;

    let context = unsafe { device.GetImmediateContext() }.context("contexte immédiat")?;

    unsafe { context.CopyResource(&staging, texture) };

    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }
        .context("projection de la texture de lecture en mémoire CPU")?;

    let base = mapped.pData as *const u8;
    let offset = (y * mapped.RowPitch + x * 4) as isize;
    // Format BGRA : l'ordre des octets en mémoire est bleu, vert, rouge, alpha.
    let (b, g, r, a) = unsafe {
        (
            *base.offset(offset),
            *base.offset(offset + 1),
            *base.offset(offset + 2),
            *base.offset(offset + 3),
        )
    };

    unsafe { context.Unmap(&staging, 0) };

    Ok((r, g, b, a))
}

/// Acquiert une image pour `region` (en retentant jusqu'à `timeout`) et lit
/// le pixel en son centre.
#[cfg(windows)]
fn capture_center_pixel(
    capture: &mut capture::DesktopCapture,
    region: geometry::Rect,
    timeout: Duration,
) -> Result<(u32, u32, u8, u8, u8, u8)> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if let Some(frame) = capture.next_frame(region)? {
            let (r, g, b, a) = read_pixel(
                capture.device(),
                &frame.texture,
                frame.width,
                frame.height,
                frame.width / 2,
                frame.height / 2,
            )?;
            return Ok((frame.width, frame.height, r, g, b, a));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    anyhow::bail!("aucune image obtenue pour {region:?} en {timeout:?}")
}

/// Configuration de l'agent, lue depuis l'environnement.
struct Config {
    signaling_url: String,
    session_id: String,
    local_ip: IpAddr,
    test_file: Option<PathBuf>,
}

fn config() -> Result<Config> {
    Ok(Config {
        signaling_url: std::env::var("SIGNALING_URL")
            .unwrap_or_else(|_| "ws://127.0.0.1:8080".into()),
        session_id: std::env::var("SESSION_ID").unwrap_or_else(|_| "demo".into()),
        local_ip: std::env::var("LOCAL_IP")
            .unwrap_or_else(|_| "127.0.0.1".into())
            .parse()
            .context("LOCAL_IP n'est pas une adresse IP valide")?,
        test_file: std::env::var("TEST_FILE").ok().map(PathBuf::from),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = config()?;

    // Mode diagnostic : CAPTURE_TEST=firefox vérifie le repérage et la capture.
    #[cfg(windows)]
    if let Ok(fragment) = std::env::var("CAPTURE_TEST") {
        let hwnd = window::find_window_by_title(&fragment)?;
        let window_rect = window::client_rect_on_screen(hwnd)?;
        tracing::info!(?window_rect, "fenêtre trouvée");

        let mut capture = capture::DesktopCapture::new()?;
        let (dw, dh) = capture.desktop_size();
        let region = geometry::crop_region(window_rect, dw, dh)
            .ok_or_else(|| anyhow::anyhow!("la fenêtre est hors de l'écran"))?;
        tracing::info!(?region, bureau = ?(dw, dh), "région de recadrage");

        // Desktop Duplication ne rend une image que lorsque le bureau change
        // (voir la note du brief). Une animation CSS dans la page de test
        // suffit en général, mais elle peut être throttlée par le navigateur
        // dès que sa fenêtre perd le focus (constaté en pratique : le tout
        // premier next_frame réussit, les suivants expirent tous — y compris
        // 5 s durant sur cette VM). Un premier essai a tenté de pallier ça en
        // faisant osciller le curseur via `SetCursorPos` en tâche de fond,
        // sans effet : le curseur matériel semble composé hors du pipeline
        // que surveille Desktop Duplication sur cette configuration (double
        // adaptateur virtuel/RTX 4070). On déplace donc plutôt la fenêtre
        // elle-même d'un pixel, en boucle : un déplacement de fenêtre force
        // toujours une recomposition DWM réelle du bureau, quel que soit le
        // pipeline d'affichage, et débloque `AcquireNextFrame` pour N'IMPORTE
        // QUELLE région échantillonnée (l'API renvoie l'image du bureau
        // entier dès qu'UNE zone change, pas seulement celle qui a changé).
        let stop_jitter = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let jitter_flag = stop_jitter.clone();
        // écart d'API windows-rs 0.62 : `HWND` enveloppe un `*mut c_void`, qui
        // n'est pas `Send` — on ne peut pas déplacer `hwnd` tel quel dans le
        // fil d'agitation. Un HWND n'est qu'un identifiant opaque (pas un
        // pointeur réellement déréférencé côté processus), donc le faire
        // transiter par son adresse brute (`isize`, qui est `Send`) et le
        // reconstruire dans le fil cible est sûr.
        let jitter_hwnd_addr = hwnd.0 as isize;
        let jitter_thread = std::thread::spawn(move || {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
            };
            let jitter_hwnd = HWND(jitter_hwnd_addr as *mut core::ffi::c_void);
            let mut toggle = false;
            while !jitter_flag.load(std::sync::atomic::Ordering::Relaxed) {
                let dx = if toggle { 0 } else { 1 };
                let _ = unsafe {
                    SetWindowPos(
                        jitter_hwnd,
                        None,
                        window_rect.x + dx,
                        window_rect.y,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
                toggle = !toggle;
                std::thread::sleep(Duration::from_millis(30));
            }
        });
        let stop_jitter_on_exit = stop_jitter.clone();
        // `result` porte le corps du diagnostic : on le fait passer par une
        // closure pour garantir l'arrêt du fil d'agitation de la fenêtre sur
        // TOUS les chemins de sortie (succès comme erreur via `?`), sans
        // dupliquer le `store` avant chaque `return`/`?`.
        let result = (|| -> Result<()> {
            // Preuve fondée sur le CONTENU, pas seulement les dimensions
            // (revue 1/5) : `CapturedFrame.width/height` sont recopiés
            // depuis `region` par construction, donc les voir correspondre à
            // la taille de la fenêtre ne prouve rien sur ce que
            // `CopySubresourceRegion` a réellement copié — un box figé sur
            // l'origine du bureau donnerait exactement le même journal. On
            // lit donc un vrai pixel :
            //   - une fois recadré sur `region` (la fenêtre, attendue verte
            //     — voir la page de test utilisée pour l'essai),
            //   - une fois recadré sur un rectangle de contrôle de même
            //     taille, placé dans le coin du bureau le plus éloigné de la
            //     fenêtre (et non à l'origine (0, 0) : pour une fenêtre
            //     proche du coin haut-gauche, un rectangle de contrôle à
            //     l'origine et de même taille peut chevaucher la fenêtre
            //     elle-même, ce qui invaliderait la comparaison sans qu'on
            //     s'en aperçoive).
            let (rw, rh, rr, rg, rb, ra) =
                capture_center_pixel(&mut capture, region, Duration::from_secs(5))?;
            tracing::info!(
                width = rw, height = rh, r = rr, g = rg, b = rb, a = ra,
                "pixel lu au centre de la région réelle (recadrage sur la fenêtre)"
            );

            let control_region = geometry::Rect {
                x: dw.saturating_sub(region.width) as i32,
                y: dh.saturating_sub(region.height) as i32,
                width: region.width,
                height: region.height,
            };
            anyhow::ensure!(
                !geometry::rects_overlap(window_rect, control_region),
                "région de contrôle {control_region:?} chevauche la fenêtre {window_rect:?} : \
                 la fenêtre est trop grande pour cet essai, la preuve de contenu serait invalide"
            );
            let (cw, ch, cr, cg, cb, ca) =
                capture_center_pixel(&mut capture, control_region, Duration::from_secs(5))?;
            tracing::info!(
                ?control_region, width = cw, height = ch, r = cr, g = cg, b = cb, a = ca,
                "pixel lu au centre de la région de contrôle (coin opposé du bureau, même taille)"
            );
            anyhow::ensure!(
                (rr, rg, rb) != (cr, cg, cb),
                "le pixel de la région réelle ({rr},{rg},{rb}) est identique à celui du \
                 contrôle ({cr},{cg},{cb}) : le recadrage ne distingue pas les deux zones"
            );
            tracing::info!(
                "preuve de contenu : le recadrage distingue bien la fenêtre du reste du bureau (pixels différents)"
            );

            let mut captured = 0;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while std::time::Instant::now() < deadline {
                if let Some(frame) = capture.next_frame(region)? {
                    captured += 1;
                    if captured == 1 {
                        tracing::info!(frame.width, frame.height, "première image capturée");
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            tracing::info!(captured, "images capturées en 3 s");
            anyhow::ensure!(captured > 0, "aucune image capturée");

            // Encodage de vérification : ENCODE_TEST=1 encode 120 images capturées.
            //
            // Écart au brief : les dimensions de l'encodeur viennent de `region`
            // (la zone effectivement recadrée par `crop_region`, toujours paire)
            // plutôt que de `window::client_size(hwnd)` — ce sont exactement les
            // dimensions des `CapturedFrame` produites par `capture.next_frame`,
            // qui peuvent différer de la zone client brute si la fenêtre déborde
            // de l'écran. Utiliser une dimension différente de celle des textures
            // réellement soumises aurait pu faire échouer `SetInputType`/
            // `ProcessInput` de façon confuse.
            if std::env::var("ENCODE_TEST").is_ok() {
                let mut encoder =
                    encode::H264Encoder::new(capture.device(), region.width, region.height, 60, 8_000_000)?;
                encoder.request_keyframe()?;

                let mut encoded = 0usize;
                let mut keyframes = 0usize;
                let mut submitted = 0usize;
                let mut first_unit_has_params = None;
                let mut pts = 0u64;
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                let mut last_progress_log = std::time::Instant::now();
                while std::time::Instant::now() < deadline && encoded < 120 {
                    if last_progress_log.elapsed() >= std::time::Duration::from_millis(500) {
                        tracing::debug!(submitted, encoded, "progression de l'essai d'encodage");
                        last_progress_log = std::time::Instant::now();
                    }
                    if let Some(frame) = capture.next_frame(region)? {
                        encoder.submit(&frame, pts)?;
                        submitted += 1;
                        pts += 1500; // 90000 / 60
                    }
                    while let Some(unit) = encoder.poll_output()? {
                        if encoded == 0 {
                            // Une unité d'accès H.264 valide doit ouvrir sur des
                            // NAL de paramètres (SPS puis PPS) avant la première
                            // tranche IDR : sans elles le décodeur du navigateur
                            // ne peut pas s'initialiser (tâche 11). On le vérifie
                            // ici plutôt que de supposer que `group_access_units`
                            // les a bien rattachées.
                            let nals = h264::split_annex_b(&unit.data);
                            let types: Vec<u8> =
                                nals.iter().map(|n| n.first().map_or(0, |b| b & 0x1F)).collect();
                            const NAL_SPS: u8 = 7;
                            const NAL_PPS: u8 = 8;
                            const NAL_IDR: u8 = 5;
                            let sps_idx = types.iter().position(|&t| t == NAL_SPS);
                            let pps_idx = types.iter().position(|&t| t == NAL_PPS);
                            let idr_idx = types.iter().position(|&t| t == NAL_IDR);
                            let ordered = matches!((sps_idx, pps_idx, idr_idx),
                                (Some(s), Some(p), Some(i)) if s < p && p < i);
                            tracing::info!(?types, ordered, "NAL de la première unité d'accès");
                            first_unit_has_params = Some(ordered);
                        }
                        encoded += 1;
                        if unit.is_keyframe {
                            keyframes += 1;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                tracing::info!(encoded, keyframes, submitted, "images encodées");
                anyhow::ensure!(encoded > 0, "aucune image encodée");
                anyhow::ensure!(keyframes > 0, "aucune image clé produite");
                anyhow::ensure!(
                    first_unit_has_params == Some(true),
                    "la première unité d'accès ne commence pas par SPS puis PPS puis IDR : {:?}",
                    first_unit_has_params
                );
            }

            Ok(())
        })();

        stop_jitter_on_exit.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = jitter_thread.join();
        result?;
        return Ok(());
    }

    let source: Box<dyn VideoSource + Send> = match &config.test_file {
        Some(path) => {
            tracing::info!(?path, "source de test");
            Box::new(FileSource::from_path(path, 1280, 720, 60)?)
        }
        None => anyhow::bail!("TEST_FILE non défini ; la capture Windows arrive à la tâche 9"),
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
        receiver_task: _,
        sender_task: _,
    } = signaling::run_signaling(&config.signaling_url, &config.session_id).await?;
    let mut session = Session::new(source, config.local_ip)?;

    let offer = offers
        .recv()
        .await
        .context("le signaling s'est fermé avant l'offre")?;
    tracing::info!("offre reçue");
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

    // I6 : `Session::run` bloque volontairement (lecture UDP synchrone bornée
    // par la cadence vidéo et les échéances str0m). L'exécuter sur un ouvrier
    // async de tokio gèlerait les autres tâches de ce processus — ici, la
    // boucle d'émission du signaling — jusqu'à une seconde par tour, voire
    // beaucoup plus dès que la session n'est plus vivante. On la déplace donc
    // sur le pool de threads bloquants de tokio, dédié à cet usage.
    let transport = tokio::task::spawn_blocking(move || {
        let mut on_input = |message| tracing::debug!(?message, "entrée reçue");
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

    Ok(())
}
