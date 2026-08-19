mod audio;
// Pas de `#[cfg(windows)]` ici : le protocole du canal média et la
// `SourceDistante` sont de la logique pure, et doivent se compiler et se
// tester sur Linux. Les sous-modules qui touchent DXGI et les tubes sont
// gatés à l'intérieur de `capteur.rs`.
mod capteur;
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
mod micro;
mod opus;
#[cfg(windows)]
mod pointer_settings;
// Le client du canal `/agent` de la plateforme. Pas de `#[cfg(windows)]` :
// le calcul du délai de reprise (`plateforme::repli`) est pur et doit se
// compiler et se tester sur l'hôte Linux, et le socket lui-même n'a rien de
// spécifique à Windows.
mod plateforme;
mod rebuild;
mod signaling;
// Pas de `#[cfg(windows)]` ici : c'est la part portable de `capture.rs`
// (lui-même `#![cfg(windows)]` dans son ensemble) — voir le commentaire de
// module de `sortie_dxgi.rs`. `superviseur::placement` (tâche 7) en a besoin
// pour se compiler et se tester sur Linux.
mod sortie_dxgi;
// Fréquence dominante d'un bloc d'échantillons (correction « A-bis »). Pur,
// nom autonome : racine nue, comme `geometry` et `sortie_dxgi` — voir la
// convention de module enfant de `CLAUDE.md`.
mod spectre;
// Pas de `#[cfg(windows)]` ici : le prédicat de PERSISTANCE (tâche 2bis, D9,
// correction n°14 de la revue) est pur -- `Option<(u32, u32)> ×
// Option<(u32, u32)> -> &str` -- et doit se compiler et se tester sur Linux,
// même s'il n'est appelé que depuis `diagnostics::multifenetre`, gated
// derrière `#[cfg(windows)]` dans `diagnostics.rs`.
mod survie_verdict;
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

// Même montage, et pour la même raison : la classification des échecs
// d'acquisition et le budget de reprises sont purs et doivent se tester sur
// l'hôte, alors que `capture.rs` est `#![cfg(windows)]` dans son ensemble.
#[path = "capture/reprise.rs"]
mod capture_reprise;

// Même montage encore (D9, tâche 11) : `Telemetrie` est pure — trois
// compteurs atomiques par SESSION, remplaçant les statiques de PROCESSUS
// `TICKS`/`CAPTURED`/`PRODUCED` de `windows_source.rs`, mortes des deux côtés
// depuis D4 (consignation n°1 de D6). `mod telemetrie;` DANS `windows_source`
// ne suffirait pas : ce fichier est lui-même `#![cfg(windows)]`, et
// `mod windows_source;` ci-dessous l'est aussi — sur Linux, tout son
// sous-arbre serait absent de la compilation, y compris ce module, qui ne
// serait donc plus « éprouvable sur l'hôte ».
#[path = "windows_source/telemetrie.rs"]
mod windows_source_telemetrie;

// Même montage encore (correction « A-bis », 19 août 2026) : la règle qui
// élit le point de terminaison audio de rendu à capter est pure — elle prend
// une liste de noms et d'identifiants et rend un élu ou une raison de refus —
// alors que `wasapi.rs` est `#![cfg(windows)]` dans son ensemble. Un
// `mod peripherique;` DANS `wasapi` la rendrait absente de la compilation
// hôte, donc inéprouvable, exactement comme pour `telemetrie` ci-dessus.
#[path = "wasapi/peripherique.rs"]
mod wasapi_peripherique;

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
    /// Les quatre champs qui suivent ne sont lus que par la branche Windows de
    /// `demarrage` : sur l'hôte Linux ils sont morts par construction, et
    /// l'`allow` le dit plutôt que de laisser un avertissement s'installer.
    #[cfg_attr(not(windows), allow(dead_code))]
    fenetre_hwnd: Option<u64>,
    /// Sortie DXGI à capturer, désignée par son nom (`\\.\DISPLAYn`). Absente,
    /// l'agent capture le bureau et recadre la fenêtre.
    #[cfg_attr(not(windows), allow(dead_code))]
    sortie_dxgi: Option<String>,
    /// La taille RETENUE (`superviseur::placement::taille_retenue`) à
    /// laquelle le superviseur a posé cette fenêtre — posée par lui seul
    /// (`lanceur.rs`), jamais par un opérateur. Absente — le chemin
    /// mono-fenêtre, où `sortie_dxgi` l'est aussi —, la taille demandée au
    /// capteur à l'attache reste `(u32::MAX, u32::MAX)`
    /// (`capteur::tube::connecter`), et `taille_retenue` la ramène telle
    /// quelle à la taille de la sortie : le comportement d'avant ce
    /// sous-bloc, inchangé.
    #[cfg_attr(not(windows), allow(dead_code))]
    taille_fenetre: Option<(u32, u32)>,
    /// Faux quand `AUDIO=0` coupe le son de cet agent.
    ///
    /// **Interrupteur GLOBAL, plus une consigne par fenêtre.** Jusqu'au
    /// sous-bloc D7, le superviseur posait `AUDIO=0` sur tous les enfants sauf
    /// un, parce qu'un unique loopback de session aurait été capté huit fois.
    /// Chaque enfant capte désormais le son de son PROPRE processus, et c'est
    /// le capteur qui arbitre entre les fenêtres d'un même processus : il n'y a
    /// plus rien à réserver.
    ///
    /// **Un agent lancé à la main coupe son son via `AUDIO=0
    /// scripts/run-agent.sh`**, qui transmet la variable en `$env:AUDIO` dans
    /// le script d'amorçage PowerShell (`schtasks` ne transmet pas
    /// l'environnement directement). Ce transport a manqué un temps : retirer
    /// le poseur du superviseur a d'abord laissé `AUDIO` sans aucun chemin
    /// vers l'enfant, `run-agent.sh` ne la portant pas — corrigé dans la
    /// même tâche 9, avant que ce commentaire ne soit lu par personne.
    #[cfg_attr(not(windows), allow(dead_code))]
    audio: bool,
    /// Vrai quand `MICRO_MESURE=1` arme le puits de mesure du micro
    /// (chantier E, `demarrage/micro.rs`).
    ///
    /// ⚠️ **CONVENTION INVERSE de `AUDIO`, `SUPERVISEUR`, `PLEIN_ECRAN` et
    /// `CAPTEUR`, et à dessein** : on désarme sur `=0` ce qui est LIVRÉ, on
    /// **arme sur `=1`** ce qui ne l'est pas. Ce puits est un instrument de
    /// banc — il consomme le flux montant pour le journaliser, il ne le joue
    /// nulle part —, et une simple présence de la variable ne suffit donc pas :
    /// il faut la valeur `1`. C'est la convention qu'avait
    /// `PLEIN_ECRAN_MODE_SORTIE`, variable retirée par le sous-bloc D9 et qu'il
    /// est donc inutile de chercher dans le code.
    micro_mesure: bool,
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
        // `SUPERVISEUR=0` DÉSACTIVE le mode, comme `AUDIO=0` désactive le son.
        // Une simple présence (`is_ok()`) ferait qu'écrire `SUPERVISEUR=0`
        // pour le couper l'activerait — piège d'exploitation d'autant plus
        // sûr que la variable voisine, elle, se lit bien ainsi.
        superviseur: matches!(std::env::var("SUPERVISEUR").as_deref(), Ok(v) if v != "0"),
        // ABSENTE : mode mono-fenêtre légitime, aucun bruit. PRÉSENTE MAIS MAL
        // FORMÉE : échec du démarrage, jamais un repli muet — voir
        // `analyser_hwnd`.
        fenetre_hwnd: match std::env::var("FENETRE_HWND") {
            Ok(brut) => Some(analyser_hwnd(&brut)?),
            Err(_) => None,
        },
        // ABSENTE : mode mono-fenêtre légitime. PRÉSENTE MAIS VIDE : échec du
        // démarrage, jamais un repli muet — même règle que `FENETRE_HWND`.
        // Plus d'analyse `adaptateur:sortie` : c'est un NOM de sortie DXGI
        // (`\\.\DISPLAYn`), stable là où les index sont positionnels.
        sortie_dxgi: match std::env::var("SORTIE_DXGI") {
            Ok(brut) => {
                let nom = brut.trim().to_string();
                anyhow::ensure!(!nom.is_empty(), "SORTIE_DXGI est vide");
                Some(nom)
            }
            Err(_) => None,
        },
        // ABSENTE : mode mono-fenêtre légitime, aucun bruit — même cas que
        // `SORTIE_DXGI`. PRÉSENTE MAIS MAL FORMÉE : échec du démarrage, même
        // règle que `FENETRE_HWND` et `SORTIE_DXGI` — cette variable n'est
        // posée QUE par le superviseur (`lanceur.rs`, forme `LxH`), donc une
        // valeur illisible signale un bug du superviseur, pas une entrée
        // d'opérateur à tolérer en silence.
        taille_fenetre: match std::env::var("TAILLE_FENETRE") {
            Ok(brut) => {
                let (l, h) = brut
                    .split_once('x')
                    .with_context(|| format!("TAILLE_FENETRE « {brut} » : format attendu LxH"))?;
                let largeur: u32 = l
                    .parse()
                    .with_context(|| format!("TAILLE_FENETRE « {brut} » : largeur illisible"))?;
                let hauteur: u32 = h
                    .parse()
                    .with_context(|| format!("TAILLE_FENETRE « {brut} » : hauteur illisible"))?;
                Some((largeur, hauteur))
            }
            Err(_) => None,
        },
        // Le son est actif par défaut : un agent lancé à la main doit
        // retrouver ce comportement. `AUDIO=0` DÉSACTIVE, comme
        // `SUPERVISEUR=0` : une simple présence (`is_ok()`) activerait le son
        // en écrivant `AUDIO=0` pour le couper — même piège que documenté
        // au-dessus pour `SUPERVISEUR`. Depuis la tâche 9 du sous-bloc D7, le
        // superviseur ne pose plus cette variable sur ses enfants : chacun
        // capte le son de SON PROPRE processus (tâche 7), et c'est le capteur
        // qui arbitre entre les fenêtres qui en partagent un.
        audio: std::env::var("AUDIO").as_deref() != Ok("0"),
        // La décision vit dans `demarrage::micro::arme`, où un test la garde :
        // une variable posée à `0`, à vide, ou à quoi que ce soit d'autre
        // laisse le puits DÉSARMÉ, comme son absence.
        micro_mesure: demarrage::micro::arme(std::env::var("MICRO_MESURE").ok().as_deref()),
    })
}

/// Analyse un `HWND` tel que le superviseur le pose sur ses enfants :
/// hexadécimal préfixé `0x` (la forme que produit `lanceur.rs`), ou décimal.
///
/// **Échoue bruyamment plutôt que de rendre `None`**, et c'est le correctif I5
/// de la revue finale. La version précédente enchaînait `.ok().and_then(…)` :
/// toute valeur mal formée — un `0x` oublié, un espace, un débordement —
/// devenait indiscernable d'une variable absente, et `demarrage::source`
/// basculait alors sur la capture du bureau entier avec recadrage, sans un mot.
/// Un enfant du superviseur diffuserait ainsi le bureau de la VM en croyant
/// montrer sa fenêtre. Une variable ABSENTE garde son sens (mode mono-fenêtre,
/// recherche par titre) ; une variable PRÉSENTE doit être honorée ou refusée.
fn analyser_hwnd(brut: &str) -> Result<u64> {
    let texte = brut.trim();
    let valeur = match texte.strip_prefix("0x").or_else(|| texte.strip_prefix("0X")) {
        Some(hexa) => u64::from_str_radix(hexa, 16)
            .with_context(|| format!("FENETRE_HWND « {texte} » : hexadécimal illisible"))?,
        None => texte
            .parse()
            .with_context(|| format!("FENETRE_HWND « {texte} » : décimal illisible"))?,
    };
    anyhow::ensure!(valeur != 0, "FENETRE_HWND vaut 0 : aucune fenêtre ne porte ce handle");
    Ok(valeur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_hwnd_hexadecimal_ou_decimal_est_accepte() {
        assert_eq!(analyser_hwnd("0x1a2b").unwrap(), 0x1a2b);
        assert_eq!(analyser_hwnd(" 0x1A2B ").unwrap(), 0x1a2b);
        assert_eq!(analyser_hwnd("6699").unwrap(), 6699);
    }

    /// Le cœur de I5 : chacune de ces valeurs rendait `None` — donc « pas de
    /// fenêtre imposée », donc le repli silencieux sur la capture du bureau.
    #[test]
    fn un_hwnd_mal_forme_fait_echouer_le_demarrage() {
        assert!(analyser_hwnd("").is_err(), "vide");
        assert!(analyser_hwnd("0x").is_err(), "préfixe seul");
        assert!(analyser_hwnd("0xzz").is_err(), "pas de l'hexadécimal");
        assert!(analyser_hwnd("1a2b").is_err(), "hexadécimal sans préfixe");
        assert!(analyser_hwnd("-1").is_err(), "négatif");
        assert!(analyser_hwnd("0").is_err(), "handle nul");
    }
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

    // Le mode capteur ne détecte rien et ne lance personne : il tient les N
    // duplications DXGI et les N encodeurs, et sert le média aux enfants du
    // superviseur par tube nommé. `CAPTEUR=0` DÉSACTIVE le mode, comme
    // `SUPERVISEUR=0` et `AUDIO=0` — même piège d'exploitation, même parade.
    if matches!(std::env::var("CAPTEUR").as_deref(), Ok(v) if v != "0") {
        return capteur::executer();
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
