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
pub(super) mod capture_virtuelle;
pub(super) mod compteurs;
pub(super) mod contrat;
pub(super) mod disponibilite;
pub(super) mod mires;
// `pub(crate)` et non `pub(super)` : deux consommateurs de PRODUCTION lisent
// encore ici. `moniteurs_virtuels::purge` (promue hors de cet arbre à la
// tâche 4) y prend `relever_topologie` et `DELAI_TOPOLOGIE` ;
// `superviseur::boucle` y prend `relever_topologie` et `noms_attaches`,
// c'est-à-dire l'APPARIEMENT d'une sortie fraîchement créée à sa place DXGI —
// la pièce centrale du montage. `PLAFOND_RECHERCHE` a en revanche cessé d'être
// emprunté : il vit désormais dans `moniteurs_virtuels::numeros` (correctif I1).
pub(crate) mod montee;
pub(super) mod nvenc;
pub(super) mod paralleles;
pub(super) mod pointeur_virtuel;
pub(super) mod replis;
pub(super) mod voies;
pub(super) mod wgc;

use anyhow::{Context, Result};

/// Vrai si la variable est posée **à autre chose que `0`**.
///
/// **Ne jamais activer une sonde sur la seule PRÉSENCE de sa variable.** Trois
/// d'entre elles créent des moniteurs virtuels qui **survivent au processus** :
/// lues par présence, `MULTIFENETRE_VDD=0` et `MULTIFENETRE_VDD_VEILLE=0`
/// lançaient leur mesure et laissaient jusqu'à dix sorties derrière elles, là
/// où quiconque écrit `=0` demande le contraire. Le projet compare déjà à `"0"`
/// ailleurs (`main.rs`, `INPUT_LINEARITY_NEUTRALISER`).
///
/// Le patron s'applique à **toutes** les sondes lues par présence, y compris
/// celles qui ne créent aucun état : une seule exception, et c'est celle qu'on
/// oublie. Corollaire assumé sur `MULTIFENETRE_VDD_VEILLE`, dont la valeur sert
/// aussi de durée (`montee.rs`) : `=0` éteint désormais la sonde au lieu de
/// demander une veille nulle.
fn sonde_demandee(variable: &str) -> bool {
    std::env::var(variable).is_ok_and(|valeur| valeur != "0")
}

/// Renvoie `true` si une sonde de ce chantier a tourné.
pub(super) fn aiguiller() -> Result<bool> {
    // Relevé DXGI : quelles sorties existent, laquelle porte le bureau.
    if sonde_demandee("MULTIFENETRE_DXGI") {
        disponibilite::relever_dxgi()?;
        return Ok(true);
    }
    // Voie 1 : Windows.Graphics.Capture, re-test honnête (abandonnée au
    // jalon 1 — voir le commentaire de tête de `wgc.rs`).
    if sonde_demandee("MULTIFENETRE_WGC") {
        wgc::eprouver()?;
        return Ok(true);
    }
    // Voie 4 : les replis par fenêtre, sondés en dernier — la moins
    // prometteuse (voir le commentaire de tête de `replis.rs`).
    if sonde_demandee("MULTIFENETRE_REPLIS") {
        replis::eprouver()?;
        return Ok(true);
    }
    // Temps 2 : le banc, sur la voie et le nombre de fenêtres demandés.
    //
    // `MULTIFENETRE_SORTIE` (« adaptateur:sortie ») désigne une sortie DXGI
    // autre que celle du bureau. Absente, le banc mesure le bureau — son
    // comportement d'origine. Elle sert à rejouer À LA MAIN le banc sur une
    // sortie que l'on sait vivante ; la mesure ③, elle, passe par
    // `MULTIFENETRE_VDD_CAPTURE`, la sortie virtuelle ne survivant pas au
    // processus qui la crée.
    if let Ok(voie) = std::env::var("MULTIFENETRE_BANC") {
        let nombre: u8 = std::env::var("MULTIFENETRE_N")
            .unwrap_or_else(|_| "8".to_string())
            .parse()
            .context("MULTIFENETRE_N doit être un entier")?;
        let sortie = match std::env::var("MULTIFENETRE_SORTIE") {
            Ok(designation) => Some(crate::moniteurs_virtuels::analyser_designation(&designation)?),
            Err(_) => None,
        };
        banc::executer(&voie, nombre, sortie)?;
        return Ok(true);
    }
    // Mesure ① — validation du contrat du pilote d'affichage virtuel, avant
    // toute création de sortie. Le GUID d'interface et 2 des 6 codes IOCTL
    // sont confirmés par octets dans la DLL installée ; la disposition des
    // tampons, elle, est une lecture amont d'un en-tête antérieur de onze mois
    // au pilote. Sans cette sonde, la mesure suivante découvrirait un contrat
    // faux EN MÊME TEMPS qu'elle prend sa mesure, et les deux échecs seraient
    // indiscernables. Elle ne crée aucun moniteur.
    if sonde_demandee("MULTIFENETRE_CONTRAT") {
        contrat::valider_contrat()?;
        return Ok(true);
    }
    // Rattrapage : détruit les sorties virtuelles laissées par une exécution
    // tuée net, que la garde de `moniteurs_virtuels::Sorties` ne peut pas
    // couvrir. Placée avant TOUTE sonde qui crée des sorties — pas seulement
    // `MULTIFENETRE_VDD`, mais aussi `MULTIFENETRE_VDD_VEILLE` juste en
    // dessous, qui en crée une elle aussi : si les deux variables sont
    // posées, une mesure ne doit jamais l'emporter sur une purge demandée.
    if sonde_demandee("MULTIFENETRE_VDD_PURGE") {
        crate::moniteurs_virtuels::purge::purger()?;
        return Ok(true);
    }
    // Tâche 1 du chantier D1 : le bureau virtuel s'étend-il jusqu'à une
    // sortie virtuelle, et le pointeur y arrive-t-il ? Crée une sortie, donc
    // passe après `MULTIFENETRE_VDD_PURGE`.
    if std::env::var("MULTIFENETRE_POINTEUR").is_ok() {
        pointeur_virtuel::sonder()?;
        return Ok(true);
    }
    // Mesure ① — l'épreuve du chien de garde, préalable à la montée en N.
    // Elle passe AVANT `MULTIFENETRE_VDD` dans cet aiguillage parce qu'elle en
    // est la condition de validité : le pilote annonce `delai = 3` d'unité non
    // documentée, et si cette unité est la seconde, une montée en N sans ping
    // mesurerait le plafond du chien de garde et non celui du pilote.
    if sonde_demandee("MULTIFENETRE_VDD_VEILLE") {
        montee::eprouver_chien_de_garde()?;
        return Ok(true);
    }
    // Mesure ① de la spec : combien de sorties virtuelles simultanées ce
    // pilote accepte. Sans ce chiffre, la voie « un moniteur virtuel par
    // fenêtre » n'est pas spécifiable.
    if sonde_demandee("MULTIFENETRE_VDD") {
        montee::monter_en_n()?;
        return Ok(true);
    }
    // Mesure ③ : la correction d'image sur la sortie virtuelle, jamais mesurée
    // par la sonde — la voie 2 n'y était garantie que « par construction ».
    // Elle crée une sortie, donc elle passe après `MULTIFENETRE_VDD_PURGE`.
    if let Ok(texte) = std::env::var("MULTIFENETRE_VDD_CAPTURE") {
        let nombre: u8 =
            texte.parse().context("MULTIFENETRE_VDD_CAPTURE doit être un entier (nombre de mires)")?;
        capture_virtuelle::capturer_sur_virtuelle(nombre)?;
        return Ok(true);
    }
    // La mesure de ce chantier : N sorties virtuelles, une fenêtre et une
    // duplication DXGI chacune — l'arrangement que la voie recommandée
    // propose réellement. Elle crée des sorties, donc elle passe après
    // `MULTIFENETRE_VDD_PURGE`.
    if let Ok(texte) = std::env::var("MULTIFENETRE_VDD_PARALLELE") {
        let nombre: u8 = texte
            .parse()
            .context("MULTIFENETRE_VDD_PARALLELE doit être un entier (nombre de sorties)")?;
        paralleles::mesurer(nombre)?;
        return Ok(true);
    }
    // Mesure ② : le plafond d'encodeurs, sur périphérique partagé (la mesure
    // de la sonde) ou sur périphériques séparés (la question qu'elle laisse).
    if let Ok(mode) = std::env::var("MULTIFENETRE_NVENC") {
        nvenc::plafond(&mode)?;
        return Ok(true);
    }
    // Tâche 6 du chantier D1 : le hook de détection des fenêtres
    // (`SetWinEventHook`), éprouvé en dehors des descriptions factices de
    // `fenetres` — la seule façon de savoir si le filtre tient sur de vraies
    // fenêtres Windows (menus, boîtes de dialogue) et si la pompe de
    // messages fait bien vivre le rappel `WINEVENT_OUTOFCONTEXT`. `sonde_demandee`
    // et non la présence seule : cette sonde ne survit pas au processus (la
    // garde `Hook` retire le hook au retour de cette branche), mais le
    // patron du fichier n'admet aucune exception.
    if sonde_demandee("SUPERVISEUR_HOOK") {
        // Pas de `#[cfg(windows)]` ici : ce module entier est déjà posé
        // derrière `#[cfg(windows)]` dans `diagnostics.rs`.
        let (tx, rx) = std::sync::mpsc::channel();
        for (fenetre, titre) in crate::superviseur::hook::enumerer_existantes() {
            tracing::info!(id = fenetre.0, titre, "fenêtre déjà ouverte");
        }
        let _garde = crate::superviseur::hook::poser(tx)?;
        tracing::info!("hook posé — ouvrez et fermez des fenêtres pendant 60 s");
        let fin = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while std::time::Instant::now() < fin {
            match rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(evenement) => tracing::info!(?evenement, "événement de fenêtre"),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(e) => {
                    tracing::warn!(erreur = %e, "canal du hook rompu");
                    break;
                }
            }
        }
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
