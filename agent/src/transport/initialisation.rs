//! Construction du socket UDP et du `Rtc` str0m dans leur état initial,
//! avant toute négociation SDP.
//!
//! **Extrait de `Session::new` (`transport.rs`)** : ce code est entièrement
//! auto-contenu — il ne lit ni n'écrit aucun champ de `Session`, seulement
//! `local_ip` et `plafond_bps` — et son extraction est ce qui a rendu à
//! `transport.rs` la marge que la revue de la tâche 12 (sous-bloc D10, le
//! remède au budget de reconstruction) lui avait prise : le fichier était
//! passé à 501 lignes, un de plus que le plafond de 500 du projet.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use str0m::bwe::Bitrate;
use str0m::{Candidate, Rtc};

use super::adaptation::ESTIMATION_INITIALE_BPS;

/// Ouvre le socket UDP de l'agent et construit le `Rtc` correspondant,
/// codecs H.264/Opus activés, estimation de bande passante initiale posée,
/// et le seul candidat local (hôte) ajouté.
///
/// `local_ip` est l'adresse par laquelle le navigateur joindra l'agent ;
/// `plafond_bps` est la cible que le sondage de bande passante cherche à
/// atteindre.
pub(super) fn construire_rtc(local_ip: IpAddr, plafond_bps: u32) -> Result<(UdpSocket, Rtc)> {
    let socket =
        UdpSocket::bind(SocketAddr::new(local_ip, 0)).context("ouverture du socket UDP")?;
    // Non bloquant une fois pour toutes : `act_on_timeout` ne dépend plus de
    // `set_read_timeout`, dont le délai déborde massivement sous Windows
    // (mesuré : dépassement moyen +12,7 ms, jusqu'à +37 ms sur un délai
    // demandé de 617 µs — voir `poll_recv_or_timeout`). Le rythme d'attente
    // est désormais entièrement piloté par notre propre boucle de sondage,
    // indépendante de la précision du minuteur du socket.
    socket.set_nonblocking(true).context("passage du socket UDP en non bloquant")?;
    let addr = socket.local_addr()?;
    tracing::info!(%addr, "socket UDP de l'agent");

    // str0m 0.21 exige un fournisseur cryptographique installé pour le
    // processus (vérifié dans les sources de la crate : la feature Cargo par
    // défaut `aws-lc-rs` fournit `from_feature_flags()`, et
    // `install_process_default(self)` est une méthode consommante sur
    // `CryptoProvider`). Idempotent : `OnceLock::set` ignore silencieusement
    // un second appel, donc appeler `Session::new` plusieurs fois par
    // processus ne panique pas.
    str0m::crypto::from_feature_flags().install_process_default();

    // `enable_opus(true)` : sans cette ligne, aucun type de charge utile
    // Opus n'est jamais proposé dans la réponse SDP, quoi que le pair
    // négocie de son côté — `select_negotiated_opus_pt` ne trouverait alors
    // jamais rien, et l'audio resterait muet même avec une source ouverte
    // avec succès. Absente du brief original, ajoutée ici : sans elle, la
    // piste audio ne se négocie tout simplement jamais (voir le rapport de
    // tâche).
    let mut rtc = Rtc::builder()
        .clear_codecs()
        .enable_h264(true)
        .enable_opus(true)
        // Sans cet appel, `Event::EgressBitrateEstimate` n'est JAMAIS émis et
        // tout l'asservissement reste muet. L'estimation initiale est
        // volontairement modeste : le sous-système sonde à la hausse vers
        // `set_desired_bitrate` (posé plus bas), et partir trop haut ferait
        // saturer le lien avant la première correction.
        .enable_bwe(Some(Bitrate::bps(ESTIMATION_INITIALE_BPS as u64)))
        .set_stats_interval(Some(Duration::from_secs(1)))
        .build(Instant::now());

    // Cible que le sondage cherche à atteindre : le plafond configuré.
    rtc.bwe().set_desired_bitrate(Bitrate::bps(plafond_bps as u64));

    // `add_local_candidate` ne renvoie pas de `Result` : elle retourne
    // `Option<&Candidate>` (le candidat précédent s'il était déjà connu).
    // Seule la construction du `Candidate` lui-même peut échouer.
    rtc.add_local_candidate(
        Candidate::host(addr, "udp").map_err(|e| anyhow!("candidat hôte invalide : {e}"))?,
    );

    Ok((socket, rtc))
}
