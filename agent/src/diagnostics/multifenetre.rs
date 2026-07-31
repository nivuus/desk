//! Sonde préalable au chantier D : quelle voie de capture rend une image
//! correcte par fenêtre quand les fenêtres se recouvrent, et à quel coût.
//!
//! Spec : `docs/superpowers/specs/2026-07-30-sonde-capture-multifenetre-design.md`.
//!
//! Chaque voie s'active par SA PROPRE variable d'environnement, et chaque
//! exécution du binaire n'en éprouve qu'une : `captureservice.dll` plantait
//! en `0xc0000005` au jalon 1, et un plantage de ce genre emporte le
//! processus entier. Les éprouver ensemble ferait perdre les autres avec la
//! première.

pub(super) mod banc;
pub(super) mod contrat;
pub(super) mod disponibilite;
pub(super) mod mires;
pub(super) mod moniteurs;
pub(super) mod montee;
pub(super) mod nvenc;
pub(super) mod peripherique;
pub(super) mod purge;
pub(super) mod replis;
pub(super) mod sudovda;
pub(super) mod voies;
pub(super) mod wgc;

use anyhow::{Context, Result};

/// Renvoie `true` si une sonde de ce chantier a tourné.
pub(super) fn aiguiller() -> Result<bool> {
    // Relevé DXGI : quelles sorties existent, laquelle porte le bureau.
    if std::env::var("MULTIFENETRE_DXGI").is_ok() {
        disponibilite::relever_dxgi()?;
        return Ok(true);
    }
    // Voie 1 : Windows.Graphics.Capture, re-test honnête (abandonnée au
    // jalon 1 — voir le commentaire de tête de `wgc.rs`).
    if std::env::var("MULTIFENETRE_WGC").is_ok() {
        wgc::eprouver()?;
        return Ok(true);
    }
    // Voie 4 : les replis par fenêtre, sondés en dernier — la moins
    // prometteuse (voir le commentaire de tête de `replis.rs`).
    if std::env::var("MULTIFENETRE_REPLIS").is_ok() {
        replis::eprouver()?;
        return Ok(true);
    }
    // Temps 2 : le banc, sur la voie et le nombre de fenêtres demandés.
    if let Ok(voie) = std::env::var("MULTIFENETRE_BANC") {
        let nombre: u8 = std::env::var("MULTIFENETRE_N")
            .unwrap_or_else(|_| "8".to_string())
            .parse()
            .context("MULTIFENETRE_N doit être un entier")?;
        banc::executer(&voie, nombre)?;
        return Ok(true);
    }
    // Mesure ① — validation du contrat du pilote d'affichage virtuel, avant
    // toute création de sortie. Le GUID d'interface et 2 des 6 codes IOCTL
    // sont confirmés par octets dans la DLL installée ; la disposition des
    // tampons, elle, est une lecture amont d'un en-tête antérieur de onze mois
    // au pilote. Sans cette sonde, la mesure suivante découvrirait un contrat
    // faux EN MÊME TEMPS qu'elle prend sa mesure, et les deux échecs seraient
    // indiscernables. Elle ne crée aucun moniteur.
    if std::env::var("MULTIFENETRE_CONTRAT").is_ok() {
        contrat::valider_contrat()?;
        return Ok(true);
    }
    // Mesure ① — l'épreuve du chien de garde, préalable à la montée en N.
    // Elle passe AVANT `MULTIFENETRE_VDD` dans cet aiguillage parce qu'elle en
    // est la condition de validité : le pilote annonce `delai = 3` d'unité non
    // documentée, et si cette unité est la seconde, une montée en N sans ping
    // mesurerait le plafond du chien de garde et non celui du pilote.
    if std::env::var("MULTIFENETRE_VDD_VEILLE").is_ok() {
        montee::eprouver_chien_de_garde()?;
        return Ok(true);
    }
    // Rattrapage : détruit les sorties virtuelles laissées par une exécution
    // tuée net, que la garde de `moniteurs_virtuels::Sorties` ne peut pas
    // couvrir. Placée avant la montée en N : si les deux variables sont
    // posées, on purge.
    if std::env::var("MULTIFENETRE_VDD_PURGE").is_ok() {
        purge::purger()?;
        return Ok(true);
    }
    // Mesure ① de la spec : combien de sorties virtuelles simultanées ce
    // pilote accepte. Sans ce chiffre, la voie « un moniteur virtuel par
    // fenêtre » n'est pas spécifiable.
    if std::env::var("MULTIFENETRE_VDD").is_ok() {
        montee::monter_en_n()?;
        return Ok(true);
    }
    // Task 10 : le plafond d'encodeurs H.264 matériels simultanés.
    if std::env::var("MULTIFENETRE_NVENC").is_ok() {
        nvenc::plafond()?;
        return Ok(true);
    }
    Ok(false)
}

/// Chaîne complète des causes d'une erreur, du contexte le plus englobant au
/// HRESULT sous-jacent — sans cela, une erreur contextualisée par
/// `H264Encoder::new` (ex. `.context("partage du périphérique D3D avec
/// l'encodeur")`) n'afficherait que ce contexte et perdrait le code d'erreur
/// natif. Voir le défaut équivalent corrigé à la tâche 6 de la sonde.
pub(super) fn causes(erreur: impl Into<anyhow::Error>) -> String {
    erreur
        .into()
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(" : ")
}
