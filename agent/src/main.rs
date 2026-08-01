mod audio;
mod clock;
mod congestion;
mod cursor;
mod demarrage;
mod diagnostics;
mod disposition;
mod frames;
// Pas de `#[cfg(windows)]` ici : les logiques pures de `gamepad` (tâche 9,
// ordonnancement des états et limitation des vibrations) n'ont rien de
// spécifique à Windows et doivent compiler et se tester sur Linux. La sonde
// `probe`, elle, reste gated à l'intérieur même du fichier
// (`agent/src/gamepad/probe.rs`), avec ses dépendances `vigem-client` /
// `anyhow::Context` propres à la sonde.
mod gamepad;
mod geometry;
mod mire;
mod moniteurs_virtuels;
mod h264;
mod input;
mod opus;
#[cfg(windows)]
mod pointer_settings;
mod rebuild;
mod signaling;
// Pas de `#[cfg(windows)]` ici : c'est la part portable de `capture.rs`
// (lui-même `#![cfg(windows)]` dans son ensemble) — voir le commentaire de
// module de `sortie_dxgi.rs`. `superviseur::placement` (tâche 7) en a besoin
// pour se compiler et se tester sur Linux.
mod sortie_dxgi;
// Pas de `#[cfg(windows)]` ici : la logique pure de `superviseur` (tâche 2)
// décide quelles fenêtres méritent d'exister côté navigateur, et doit se
// compiler et se tester sur Linux sans dépendance à l'API Windows.
mod superviseur;
mod source;
mod transport;
mod turn;
// Le calcul de région est pur et doit être testable sur l'hôte : il est donc
// déclaré indépendamment du reste de `windows_source`, qui ne compile que sur
// Windows.
#[path = "windows_source/sortie.rs"]
mod windows_source_sortie;

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

use anyhow::{Context, Result};

/// Configuration de l'agent, lue depuis l'environnement.
struct Config {
    signaling_url: String,
    session_id: String,
    local_ip: IpAddr,
    test_file: Option<PathBuf>,
    /// Vrai en mode superviseur : ce processus ne capture rien, il détecte les
    /// fenêtres et lance un enfant par fenêtre.
    superviseur: bool,
    /// `HWND` de la fenêtre à capturer, en décimal ou hexadécimal préfixé
    /// `0x`. Posé par le superviseur sur ses enfants ; absent, l'agent
    /// retombe sur la recherche par titre (`WINDOW_TITLE`), c'est-à-dire sur
    /// le comportement mono-fenêtre d'avant ce sous-bloc.
    ///
    /// Les trois champs qui suivent ne sont lus que par la branche Windows de
    /// `demarrage` : sur l'hôte Linux ils sont morts par construction, et
    /// l'`allow` le dit plutôt que de laisser un avertissement s'installer.
    #[cfg_attr(not(windows), allow(dead_code))]
    fenetre_hwnd: Option<u64>,
    /// Sortie DXGI à capturer, sous la forme `adaptateur:sortie`. Absente,
    /// l'agent capture le bureau et recadre la fenêtre.
    #[cfg_attr(not(windows), allow(dead_code))]
    sortie_dxgi: Option<(u32, u32)>,
    /// Faux sur les enfants qui ne portent pas le son.
    #[cfg_attr(not(windows), allow(dead_code))]
    audio: bool,
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
        superviseur: std::env::var("SUPERVISEUR").is_ok(),
        fenetre_hwnd: std::env::var("FENETRE_HWND").ok().and_then(|v| {
            let v = v.trim();
            match v.strip_prefix("0x") {
                Some(hexa) => u64::from_str_radix(hexa, 16).ok(),
                None => v.parse().ok(),
            }
        }),
        sortie_dxgi: std::env::var("SORTIE_DXGI").ok().and_then(|v| {
            let (a, s) = v.split_once(':')?;
            Some((a.trim().parse().ok()?, s.trim().parse().ok()?))
        }),
        // Le son est actif par défaut : c'est le comportement mono-fenêtre
        // d'avant ce sous-bloc, qu'un agent lancé à la main doit retrouver.
        // Seul le superviseur le coupe, sur les enfants non porteurs.
        audio: std::env::var("AUDIO").as_deref() != Ok("0"),
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

    // Diagnostic (chantier duplications-parallèles, tâche 2bis) : posé AVANT
    // tout le reste, pour qu'aucune faute ne puisse survenir avant lui.
    // Inerte sans `AGENT_TRACE_EXCEPTIONS`, et le gestionnaire ne s'exécute
    // qu'au moment d'une violation d'accès — jamais sur le chemin nominal.
    #[cfg(windows)]
    diagnostics::exceptions::installer();

    // Au tout début, avant toute possibilité d'injection d'entrée (les modes
    // diagnostic de `diagnostics::aiguiller` n'en injectent pas, mais la
    // session normale plus bas le fait) : neutraliser l'accélération et la
    // sensibilité pointeur de la session Windows. Ne fait jamais échouer le
    // démarrage — une visée dégradée vaut mieux que pas de session.
    //
    // `INPUT_LINEARITY_NEUTRALISER=0` saute AUSSI cet appel-ci, et pas
    // seulement celui, propre à la sonde de linéarité, que porte
    // `diagnostics::entree`.
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

    // Après la neutralisation ci-dessus, et avant tout assemblage de session :
    // c'est cet ordre que la sonde de linéarité suppose (voir `diagnostics`).
    if diagnostics::aiguiller()? {
        return Ok(());
    }

    // Le mode superviseur ne capture rien : il détecte les fenêtres et lance
    // un enfant par fenêtre. Ses enfants n'héritent JAMAIS de `SUPERVISEUR`
    // (voir `superviseur::lanceur`), sans quoi chacun se prendrait pour un
    // superviseur et lancerait les siens, indéfiniment.
    if config.superviseur {
        return superviseur::executer(config).await;
    }

    demarrage::executer(config).await
}
