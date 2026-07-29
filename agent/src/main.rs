mod audio;
mod clock;
mod congestion;
mod cursor;
mod frames;
// Pas de `#[cfg(windows)]` ici : les logiques pures de `gamepad` (tâche 9,
// ordonnancement des états et limitation des vibrations) n'ont rien de
// spécifique à Windows et doivent compiler et se tester sur Linux. La sonde
// `probe`, elle, reste gated à l'intérieur même du fichier
// (`agent/src/gamepad.rs`), avec ses dépendances `vigem-client` /
// `anyhow::Context` propres à la sonde.
mod gamepad;
mod geometry;
mod h264;
mod input;
mod opus;
#[cfg(windows)]
mod pointer_settings;
mod rebuild;
mod signaling;
mod source;
mod transport;

#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod encode;
#[cfg(windows)]
mod window;
#[cfg(windows)]
mod wasapi;
#[cfg(windows)]
mod windows_audio;
#[cfg(windows)]
mod windows_source;

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
#[cfg(windows)]
fn watch_encoder(telemetry: std::sync::Arc<encode::EncoderTelemetry>) -> impl FnOnce() {
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

/// Point de départ et amplitude de la sonde de linéarité (tâche 8, chantier
/// B), extraits en fonction pure — sans le moindre appel Windows — pour
/// être testables sur Linux, comme `geometry.rs` ou `cursor::Hysteresis`.
///
/// `dx` et `dy` partagent toujours le signe de `pas` (la sonde ne déplace
/// le curseur que dans un seul quadrant), donc il ne faut de marge que DANS
/// LE SENS du déplacement, pas des deux côtés : partir à `marge` px du bord
/// de départ (haut-gauche si `pas` est positif ou nul, bas-droite sinon)
/// suffit, quelle que soit la résolution, tant que sa plus petite dimension
/// excède `2 * marge + amplitude.abs()`. En dessous, le clampage aux bords
/// de l'écran fausserait la mesure : refusé explicitement plutôt que de
/// laisser un écart silencieusement faux (constaté en pratique : un essai
/// parti du CENTRE d'un écran 2400×1080 avec une amplitude de 1000 px a
/// donné `obtenu_y = 539`, signature d'un curseur buté en bas d'écran).
///
/// Renvoie `(centre_x, centre_y, amplitude)`.
fn point_depart_lineaire(
    largeur: i32,
    hauteur: i32,
    pas: i16,
    repetitions: i32,
    marge: i32,
) -> Result<(i32, i32, i32)> {
    let amplitude = pas as i32 * repetitions;
    anyhow::ensure!(
        largeur.min(hauteur) >= 2 * marge + amplitude.abs(),
        "écran {largeur}x{hauteur} trop petit pour une amplitude de {amplitude} px \
         (pas={pas}, répétitions={repetitions}) : le clampage fausserait la mesure"
    );
    let coord = |dimension: i32| if amplitude >= 0 { marge } else { (dimension - 1 - marge).max(0) };
    Ok((coord(largeur), coord(hauteur), amplitude))
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

    // Au tout début, avant toute possibilité d'injection d'entrée (les modes
    // diagnostic ci-dessous n'en injectent pas, mais la session normale plus
    // bas le fait) : neutraliser l'accélération et la sensibilité pointeur de
    // la session Windows. Ne fait jamais échouer le démarrage — une visée
    // dégradée vaut mieux que pas de session.
    //
    // `INPUT_LINEARITY_NEUTRALISER=0` saute AUSSI cet appel-ci, et pas
    // seulement celui, plus bas, propre à la sonde de linéarité.
    // Correction de la ronde de revue 1 : le réglage SPI que pose
    // `neutraliser()` vaut pour la SESSION Windows entière, pas pour le
    // processus qui l'a posé (voir `pointer_settings.rs`) — sauter
    // uniquement l'appel de la sonde ne suffisait donc pas, puisque cet
    // appel-ci, plus haut et inconditionnel, avait déjà neutralisé
    // l'accélération avant même que la sonde ne lise sa propre variable.
    // La mesure de référence obtenait un écart nul quel que soit l'état
    // réel de la VM : un artefact garanti par construction, pas une mesure.
    #[cfg(windows)]
    if std::env::var("INPUT_LINEARITY_NEUTRALISER").as_deref() != Ok("0") {
        match pointer_settings::neutraliser() {
            Ok(rapport) => tracing::info!(rapport, "accélération pointeur neutralisée"),
            Err(e) => tracing::warn!(erreur = %e, "neutralisation de l'accélération pointeur échouée"),
        }
    } else {
        tracing::warn!(
            "neutralisation SAUTÉE au démarrage (INPUT_LINEARITY_NEUTRALISER=0, mesure de référence)"
        );
    }

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
            use windows::Win32::Foundation::RECT;
            use windows::Win32::UI::WindowsAndMessaging::{
                GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
            };
            let jitter_hwnd = HWND(jitter_hwnd_addr as *mut core::ffi::c_void);
            // Repère d'oscillation : l'origine de la FENÊTRE, pas celle de sa
            // zone client. `window_rect` est un rectangle client converti en
            // coordonnées écran (`client_rect_on_screen`) : le repasser tel
            // quel à `SetWindowPos`, qui attend des coordonnées de fenêtre,
            // décalait la fenêtre vers la droite de l'épaisseur de sa bordure
            // (~9 px) à chaque exécution. Le décalage s'accumulait d'un essai
            // à l'autre jusqu'à faire chevaucher la fenêtre et la région de
            // contrôle, ce qui faisait échouer la preuve de contenu — panne
            // du banc d'essai, pas de la capture.
            let mut origin = RECT::default();
            let anchor = match unsafe { GetWindowRect(jitter_hwnd, &mut origin) } {
                Ok(()) => (origin.left, origin.top),
                // Repli sur l'ancien comportement : mieux vaut agiter la
                // fenêtre à quelques pixels près que ne pas l'agiter du tout.
                Err(_) => (window_rect.x, window_rect.y),
            };
            let mut toggle = false;
            while !jitter_flag.load(std::sync::atomic::Ordering::Relaxed) {
                let dx = if toggle { 0 } else { 1 };
                let _ = unsafe {
                    SetWindowPos(
                        jitter_hwnd,
                        None,
                        anchor.0 + dx,
                        anchor.1,
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
                    encode::H264Encoder::new(
                        capture.device(),
                        (region.width, region.height),
                        (region.width, region.height),
                        60,
                        8_000_000,
                    )?;
                encoder.request_keyframe()?;
                // Même surveillance que la mesure de débit : elle sert ici à
                // vérifier que des images RÉELLEMENT DISTINCTES traversent le
                // convertisseur (`converter_inputs` doit progresser), et pas
                // seulement que des unités d'accès sortent de l'encodeur.
                let stop_watchdog = watch_encoder(encoder.telemetry());

                let mut encoded = 0usize;
                let mut keyframes = 0usize;
                let mut submitted = 0usize;
                let mut first_unit_has_params = None;
                let mut pts = 0u64;
                // Bornes réglables : le contrat du brief (120 images, 5 s)
                // reste la valeur par défaut, mais une mesure du pipeline
                // RÉEL sur plusieurs centaines d'images demande une fenêtre
                // plus longue.
                //
                // Chiffre périmé retiré (28/07) : ce commentaire citait un
                // plafond de ~48 im/s pour la capture Desktop Duplication.
                // La recette du jalon 1
                // (`docs/superpowers/plans/2026-07-27-jalon1-recette.md`,
                // « Ce qui a été appris ») établit que ce chiffre était
                // obsolète — remesurée, la capture isolée (`CAPTURE_TEST`)
                // soutient ~90 im/s, et cette boucle capture+encodage
                // elle-même (mesurée ici, `ENCODE_TEST`) soutient ~80 im/s.
                // 120 images durent donc en pratique ~1,5 s, pas 2,5 s.
                let encode_target: usize = std::env::var("ENCODE_TEST_TARGET")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(120);
                let encode_secs: u64 = std::env::var("ENCODE_TEST_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5);
                let start = std::time::Instant::now();
                let deadline = start + std::time::Duration::from_secs(encode_secs);
                let phase = encoder.telemetry();
                // Marquer l'acquisition : un blocage dans la capture et un
                // blocage dans l'encodeur produisent la même signature vue du
                // fil de surveillance (compteurs figés, étape au repos). La
                // capture affine elle-même en sous-étapes (acquisition, copie
                // GPU, libération) une fois le marqueur branché.
                capture.set_phase_marker(phase.phase.clone());
                while std::time::Instant::now() < deadline && encoded < encode_target {
                    phase
                        .phase
                        .store(encode::PHASE_CAPTURE, std::sync::atomic::Ordering::Relaxed);
                    let acquired = capture.next_frame(region)?;
                    phase
                        .phase
                        .store(encode::PHASE_IDLE, std::sync::atomic::Ordering::Relaxed);
                    if let Some(frame) = acquired {
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
                stop_watchdog();
                let elapsed = start.elapsed();
                // `converter_inputs` prouve que ce sont bien des images
                // NEUVES qui ont traversé le convertisseur, et pas la même
                // réencodée : c'est la différence entre un pipeline qui
                // fonctionne et un compteur qui monte.
                let converter_inputs = encoder
                    .telemetry()
                    .converter_inputs
                    .load(std::sync::atomic::Ordering::Relaxed);
                tracing::info!(
                    encoded,
                    keyframes,
                    submitted,
                    converter_inputs,
                    elapsed_ms = elapsed.as_millis() as u64,
                    fps = encoded as f64 / elapsed.as_secs_f64(),
                    "images encodées"
                );
                anyhow::ensure!(encoded > 0, "aucune image encodée");
                anyhow::ensure!(keyframes > 0, "aucune image clé produite");
                anyhow::ensure!(
                    first_unit_has_params == Some(true),
                    "la première unité d'accès ne commence pas par SPS puis PPS puis IDR : {:?}",
                    first_unit_has_params
                );
            }

            // Mesure de débit isolée du pipeline conversion+encodage :
            // ENCODER_THROUGHPUT_TEST=1 réinjecte une SEULE texture déjà
            // capturée, en boucle serrée, sans jamais repasser par
            // `capture.next_frame` — contrairement à `ENCODE_TEST` ci-dessus,
            // qui mélange le débit de la source (Desktop Duplication, limité
            // par les changements d'écran réels) avec celui de l'encodeur
            // lui-même. C'est cette mesure isolée, faite par le relecteur
            // pendant la ronde de correction 1/5, qui a permis d'établir que
            // le plafond à ~1 image/s observé avec `ENCODE_TEST` venait d'un
            // bogue de pilotage du convertisseur (voir `encode.rs`), pas du
            // GPU : rejouer la même texture prouve/dément la théorie
            // matérielle sans dépendre de la disponibilité d'images fraîches.
            // Conservée telle quelle pour la tâche 14, qui en aura besoin
            // pour ses propres mesures de débit.
            if std::env::var("ENCODER_THROUGHPUT_TEST").is_ok() {
                let mut encoder =
                    encode::H264Encoder::new(
                        capture.device(),
                        (region.width, region.height),
                        (region.width, region.height),
                        60,
                        8_000_000,
                    )?;
                encoder.request_keyframe()?;

                // Une seule image réelle, capturée une fois puis réinjectée
                // telle quelle à chaque itération : aucun autre appel à
                // `capture.next_frame` dans cette boucle.
                // Bornée : sans échéance, une capture qui ne rend plus d'image
                // ferait attendre indéfiniment sans la moindre trace.
                let frame_deadline = std::time::Instant::now() + Duration::from_secs(10);
                let frame = loop {
                    if let Some(frame) = capture.next_frame(region)? {
                        break frame;
                    }
                    anyhow::ensure!(
                        std::time::Instant::now() < frame_deadline,
                        "aucune image capturée en 10 s pour amorcer la mesure de débit"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(2));
                };

                let target: usize = std::env::var("ENCODER_THROUGHPUT_TARGET")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(600);
                // Échéance réglable, et courte par défaut : une échéance de
                // dix minutes transforme le moindre blocage du pipeline en
                // dix minutes de silence complet, ce qui a réellement coûté
                // plusieurs cycles d'investigation sur cette tâche.
                let deadline_secs: u64 = std::env::var("ENCODER_THROUGHPUT_DEADLINE_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30);
                // Cadence de soumission, en images par seconde. 0 = aucune
                // limite (on soumet aussi vite que la boucle tourne).
                //
                // Cadencer change ce que la mesure signifie, et c'est
                // volontaire. Sans limite, `submit` est appelé bien plus
                // souvent qu'aucune source réelle ne le ferait : le
                // convertisseur, sollicité en permanence, refuse la quasi-
                // totalité des images neuves et le débit mesuré ne reflète
                // plus que l'encodeur rejouant la dernière image convertie.
                // Avec une cadence, on mesure ce que le jalon exige vraiment :
                // combien d'images NEUVES par seconde traversent conversion
                // PUIS encodage (`converter_inputs` dans la ligne de
                // résultat).
                let submit_hz: u64 = std::env::var("ENCODER_THROUGHPUT_SUBMIT_HZ")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let submit_interval =
                    (submit_hz > 0).then(|| Duration::from_nanos(1_000_000_000 / submit_hz));
                tracing::info!(
                    target,
                    deadline_secs,
                    submit_hz,
                    "début de la mesure de débit isolée"
                );
                let stop_watchdog = watch_encoder(encoder.telemetry());

                let mut encoded = 0usize;
                let mut keyframes = 0usize;
                let mut submitted = 0usize;
                let mut pts = 0u64;
                let start = std::time::Instant::now();
                let deadline = start + std::time::Duration::from_secs(deadline_secs);
                let loop_result = (|| -> Result<()> {
                    let mut next_submit = std::time::Instant::now();
                    while encoded < target && std::time::Instant::now() < deadline {
                        let before = encoded;
                        if let Some(interval) = submit_interval {
                            let now = std::time::Instant::now();
                            if now < next_submit {
                                std::thread::sleep(next_submit - now);
                            }
                            next_submit += interval;
                        }
                        encoder.submit(&frame, pts)?;
                        submitted += 1;
                        pts += 1500; // 90000 / 60
                        while let Some(unit) = encoder.poll_output()? {
                            encoded += 1;
                            if unit.is_keyframe {
                                keyframes += 1;
                            }
                        }
                        if encoded == before && submit_interval.is_none() {
                            // Rien n'a avancé : rendre la main brièvement.
                            // Marteler `submit` sans répit (mesuré : 90
                            // millions d'appels en 30 s) ne mesure pas un
                            // débit, ça le détruit — chaque appel interroge le
                            // convertisseur et le maintient sous une pression
                            // qu'aucune source réelle ne produirait. La pause
                            // n'a lieu QUE sur un tour improductif, donc elle
                            // ne peut pas plafonner le débit mesuré.
                            std::thread::sleep(Duration::from_micros(100));
                        }
                    }
                    Ok(())
                })();
                stop_watchdog();
                let elapsed = start.elapsed();
                let fps = encoded as f64 / elapsed.as_secs_f64();
                // Une mesure de débit doit dire ce qu'elle a réellement fait :
                // `converter_inputs` est le nombre de conversions DISTINCTES,
                // `conv_refus` le nombre d'images sautées faute de
                // disponibilité du convertisseur. Sans ces deux chiffres, un
                // débit élevé pourrait n'être que la même image réencodée.
                let telemetry = encoder.telemetry();
                let converter_inputs = telemetry
                    .converter_inputs
                    .load(std::sync::atomic::Ordering::Relaxed);
                let conv_refus = telemetry
                    .converter_not_accepting
                    .load(std::sync::atomic::Ordering::Relaxed);
                // Journalisé AVANT de propager une éventuelle erreur de la
                // boucle : une mesure partielle reste une donnée, alors qu'une
                // erreur remontée sans chiffres ne dit rien de l'endroit où le
                // pipeline s'est arrêté.
                tracing::info!(
                    encoded,
                    keyframes,
                    submitted,
                    elapsed_ms = elapsed.as_millis() as u64,
                    fps,
                    converter_inputs,
                    conv_refus,
                    "débit isolé du pipeline conversion+encodage (source constante, sans capture)"
                );
                loop_result?;
                anyhow::ensure!(encoded > 0, "aucune image encodée en mesure isolée");
            }

            Ok(())
        })();

        stop_jitter_on_exit.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = jitter_thread.join();
        result?;
        return Ok(());
    }

    // Sonde audio (`AUDIO_PROBE=1`) : répond aux questions n°1 et n°2 de la
    // spécification du chantier A — quel est le périphérique de rendu par
    // défaut de CETTE session, quel est son format de mixage, et un loopback
    // y capte-t-il bien ce que jouent les applications.
    #[cfg(windows)]
    if std::env::var("AUDIO_PROBE").is_ok() {
        let secondes: u64 = std::env::var("AUDIO_PROBE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        let mut capture = wasapi::LoopbackCapture::open()?;
        tracing::info!(format = %capture.description(), "loopback ouvert");

        let debut = std::time::Instant::now();
        let mut echantillons = 0u64;
        let mut crete = 0i16;
        let mut lectures_vides = 0u64;
        while debut.elapsed() < std::time::Duration::from_secs(secondes) {
            match capture.read()? {
                Some(bloc) => {
                    echantillons += bloc.len() as u64;
                    for v in bloc {
                        crete = crete.max(v.saturating_abs());
                    }
                }
                None => lectures_vides += 1,
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        tracing::info!(
            echantillons,
            lectures_vides,
            crete,
            silencieux = crete == 0,
            "sonde audio terminée"
        );
        return Ok(());
    }

    // Sonde n°4 de la spec du chantier A : le *process loopback*
    // (Windows 10 build 19041+) isole l'audio d'un seul processus, ce
    // qu'exige le modèle multi-fenêtres du chantier D. La VM est en build
    // 20348, donc éligible sur le papier. RIEN N'EST CONSTRUIT DESSUS ici :
    // on observe seulement si l'activation réussit, et le résultat est
    // consigné pour le chantier D.
    #[cfg(windows)]
    if let Ok(pid_texte) = std::env::var("PROCESS_LOOPBACK_PROBE") {
        let pid: u32 = pid_texte
            .parse()
            .context("PROCESS_LOOPBACK_PROBE doit être un identifiant de processus")?;
        match wasapi::probe_process_loopback(pid) {
            Ok(rapport) => tracing::info!(pid, rapport, "sonde process loopback"),
            Err(e) => tracing::warn!(pid, erreur = %e, "sonde process loopback échouée"),
        }
        return Ok(());
    }

    // Sonde du chantier B (§11, inconnues n°1 et n°2) : ViGEmBus accepte-t-il
    // de brancher une manette Xbox 360 virtuelle, et son rappel de
    // notification restitue-t-il bien les magnitudes de vibration qu'un jeu
    // demande ? RIEN N'EST CONSTRUIT DESSUS ici : on observe, et le résultat
    // fige l'API réellement disponible pour la tâche 10.
    #[cfg(windows)]
    if std::env::var("VIGEM_PROBE").is_ok() {
        let secondes: u64 = std::env::var("VIGEM_PROBE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);
        match crate::gamepad::probe(secondes) {
            Ok(rapport) => tracing::info!(rapport, "sonde ViGEmBus"),
            Err(e) => tracing::warn!(erreur = %e, "sonde ViGEmBus échouée"),
        }
        return Ok(());
    }

    // Sonde n°1 de la recette du chantier B : la visée est-elle linéaire
    // 1:1 ? On injecte une somme connue de déplacements relatifs et on
    // compare au déplacement réel du curseur.
    //
    // `INPUT_LINEARITY_NEUTRALISER=0` saute la neutralisation SPI — ici ET
    // au tout début de `main()` (voir le commentaire là-bas) : c'est ce qui
    // rend la mesure démonstrative plutôt que rassurante — l'écart observé
    // sans neutralisation chiffre ce que la neutralisation apporte. Sauter
    // seulement l'appel ci-dessous, sans toucher à celui du démarrage,
    // aurait laissé ce dernier neutraliser la session avant même que la
    // sonde ne s'exécute (bogue réel de la première version de cette
    // tâche, corrigé en ronde de revue 1).
    #[cfg(windows)]
    if std::env::var("INPUT_LINEARITY_PROBE").is_ok() {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetCursorPos, GetSystemMetrics, SetCursorPos, SM_CXSCREEN, SM_CYSCREEN,
        };

        let pas: i16 = std::env::var("INPUT_LINEARITY_PAS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        let repetitions: i32 = std::env::var("INPUT_LINEARITY_REPETITIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);

        if std::env::var("INPUT_LINEARITY_NEUTRALISER").as_deref() != Ok("0") {
            match pointer_settings::neutraliser() {
                Ok(rapport) => tracing::info!(rapport, "neutralisation appliquée"),
                Err(e) => tracing::warn!(erreur = %e, "neutralisation échouée"),
            }
        } else {
            tracing::warn!("neutralisation SAUTÉE (mesure de référence)");
        }

        // Écart au brief : le point fixe (960, 540) qu'il propose suppose un
        // écran 1920×1080, jamais vérifié, et son CENTRE s'est révélé
        // insuffisant à l'essai (voir `point_depart_lineaire`, testée sans
        // Windows). Remplacé par une lecture réelle de la résolution et un
        // point de départ biaisé dans le sens du déplacement.
        const MARGE: i32 = 20;
        let largeur = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let hauteur = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        let (centre_x, centre_y, amplitude) =
            point_depart_lineaire(largeur, hauteur, pas, repetitions, MARGE)?;
        tracing::info!(largeur, hauteur, centre_x, centre_y, amplitude, "point de départ de la sonde");
        unsafe { SetCursorPos(centre_x, centre_y) }?;
        std::thread::sleep(std::time::Duration::from_millis(200));

        let mut avant = POINT::default();
        unsafe { GetCursorPos(&mut avant) }?;

        let injecteur_hwnd = windows::Win32::Foundation::HWND(std::ptr::null_mut());
        let mode = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let mut injecteur = input::InputInjector::new(injecteur_hwnd, mode);
        for _ in 0..repetitions {
            injecteur.inject(proto::input::InputMessage::MouseMoveRelative { dx: pas, dy: pas })?;
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        std::thread::sleep(std::time::Duration::from_millis(200));

        let mut apres = POINT::default();
        unsafe { GetCursorPos(&mut apres) }?;

        // Même formule que `point_depart_lineaire` : réutilisée telle
        // quelle plutôt que recalculée, pour ne pas risquer de la faire
        // diverger de celle qui a dimensionné le point de départ (revue 1).
        let attendu = amplitude;
        let obtenu_x = apres.x - avant.x;
        let obtenu_y = apres.y - avant.y;
        tracing::info!(
            attendu,
            obtenu_x,
            obtenu_y,
            ecart_x = obtenu_x - attendu,
            ecart_y = obtenu_y - attendu,
            "sonde de linéarité terminée"
        );
        return Ok(());
    }

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
    // fenêtre du mode diagnostic `CAPTURE_TEST` plus haut dans ce fichier.
    #[cfg(windows)]
    let mut window_hwnd_addr: Option<isize> = None;

    // Origine d'horloge unique de la session. Les deux médias l'utilisent :
    // c'est ce qui rend leurs lignes de temps comparables, et donc la synchro
    // A/V exacte par construction. La créer ici, une seule fois, garantit
    // qu'aucune durée d'initialisation ne les décale l'une de l'autre.
    let clock_origin = std::time::Instant::now();

    let source: Box<dyn VideoSource + Send> = match &config.test_file {
        Some(path) => {
            tracing::info!(?path, "source de test");
            Box::new(FileSource::from_path(path, 1280, 720, 60)?)
        }
        None => {
            #[cfg(windows)]
            {
                let title = std::env::var("WINDOW_TITLE").unwrap_or_else(|_| "firefox".into());
                let hwnd = window::find_window_by_title(&title)?;
                window_hwnd_addr = Some(hwnd.0 as isize);
                let bitrate: u32 = std::env::var("BITRATE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(12_000_000);
                // 90 et non 60 : cette valeur n'est pas une cadence cible, c'est
                // le `MF_MT_FRAME_RATE` annoncé aux deux MFT — et le Video
                // Processor s'en sert comme cadence de SORTIE, qu'il tient en
                // rejouant la dernière image convertie quand rien de neuf ne
                // lui est arrivé. Annoncer 60 plafonnait donc tout le pipeline
                // à 60 sorties/s pour un bureau qui en produit 68,5, d'où
                // 47,5 images/s encodées et ~44 i/s au navigateur.
                //
                // Mesuré (bureau à 68,5 Hz) : 60 → 44,3 i/s · 75 → 53,1 ·
                // 90 → 58,5 · 120 → 63,0. Au-delà de 90, le gain est du
                // rejeu : à 120, les images NEUVES converties retombent de 62
                // à 54/s parce que le convertisseur, occupé à tenir sa cadence
                // déclarée, refuse davantage d'entrées.
                let fps: u32 = std::env::var("ENCODER_FPS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(90);
                tracing::info!(title, bitrate, fps, "capture de la fenêtre Windows");
                Box::new(windows_source::WindowsSource::new(hwnd, fps, bitrate, clock_origin)?)
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
        receiver_task: _,
        sender_task: _,
    } = signaling::run_signaling(&config.signaling_url, &config.session_id).await?;
    let mut session = Session::new(source, config.local_ip, clock_origin)?;

    // Source audio : son absence ne compromet jamais la session vidéo. Sur une
    // source de test (TEST_FILE), il n'y a rien à capter. Hors Windows, il n'y
    // a pas de WASAPI. Et si le loopback refuse de s'ouvrir — pas de
    // périphérique de rendu par défaut, format de mixage non supporté — on
    // journalise et la session continue, muette.
    #[cfg(windows)]
    if config.test_file.is_none() {
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

#[cfg(test)]
mod tests {
    use super::point_depart_lineaire;

    /// Résolution de la VM au moment des mesures (2400×1080) : le centre du
    /// bureau (1200, 540) ne laisse que 540 px de marge verticale, en dessous
    /// de l'amplitude de 1000 px que la tâche demande de mesurer — c'est
    /// exactement le clampage constaté en pratique avant la correction.
    const LARGEUR_VM: i32 = 2400;
    const HAUTEUR_VM: i32 = 1080;

    #[test]
    fn pas_positif_part_pres_du_bord_haut_gauche() {
        // pas=10, répétitions=100 : amplitude 1000, comme la mesure 2 du
        // brief.
        let (x, y, amplitude) =
            point_depart_lineaire(LARGEUR_VM, HAUTEUR_VM, 10, 100, 20).unwrap();
        assert_eq!(amplitude, 1000);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }

    #[test]
    fn pas_negatif_part_pres_du_bord_bas_droit() {
        // Jamais exercé par la sonde telle qu'appelée aujourd'hui (le brief
        // ne teste que des `pas` positifs), mais la formule doit rester
        // correcte pour ce cas : c'est justement ce que ce test vérifie.
        let (x, y, amplitude) =
            point_depart_lineaire(LARGEUR_VM, HAUTEUR_VM, -10, 100, 20).unwrap();
        assert_eq!(amplitude, -1000);
        assert_eq!(x, LARGEUR_VM - 1 - 20);
        assert_eq!(y, HAUTEUR_VM - 1 - 20);
    }

    #[test]
    fn grand_pas_faible_repetition_donne_la_meme_amplitude() {
        // pas=200, répétitions=5 : amplitude 1000, comme la mesure 3 du
        // brief — c'est là que l'accélération se verrait si la
        // neutralisation n'avait pas pris.
        let (x, y, amplitude) =
            point_depart_lineaire(LARGEUR_VM, HAUTEUR_VM, 200, 5, 20).unwrap();
        assert_eq!(amplitude, 1000);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }

    #[test]
    fn refuse_un_ecran_trop_petit_pour_l_amplitude_demandee() {
        // 100x100 ne laisse aucune place pour une amplitude de 1000 px :
        // la garde doit refuser plutôt que de laisser le clampage fausser
        // silencieusement la mesure.
        let erreur = point_depart_lineaire(100, 100, 200, 5, 20).unwrap_err();
        assert!(erreur.to_string().contains("trop petit"));
    }

    #[test]
    fn accepte_pile_a_la_limite_de_la_marge() {
        // largeur.min(hauteur) == 2*marge + amplitude exactement : la garde
        // compare avec `>=`, ce cas limite doit donc passer.
        let (x, y, amplitude) = point_depart_lineaire(1040, 2000, 10, 100, 20).unwrap();
        assert_eq!(amplitude, 1000);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }

    #[test]
    fn refuse_juste_sous_la_limite_de_la_marge() {
        let erreur = point_depart_lineaire(1039, 2000, 10, 100, 20).unwrap_err();
        assert!(erreur.to_string().contains("trop petit"));
    }

    #[test]
    fn amplitude_nulle_ne_demande_aucune_marge_particuliere() {
        let (x, y, amplitude) = point_depart_lineaire(50, 50, 0, 0, 20).unwrap();
        assert_eq!(amplitude, 0);
        assert_eq!(x, 20);
        assert_eq!(y, 20);
    }
}
