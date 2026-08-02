//! La sonde minimale : D duplications DXGI, tenues, et le moins possible
//! autour — **« rien d'autre » a une portée précise, pas absolue.**
//!
//! **Ce qui EST exclu** : encodeur (NVENC/Media Foundation), convertisseur de
//! couleur, fenêtre, WebRTC. Si le refus de la 5ᵉ duplication observé au
//! sous-bloc D2 ne se reproduit pas ici, c'est que le plafond ne porte pas sur
//! la duplication mais sur l'un de CES éléments-là (hypothèse H3 de la spec)
//! — et c'est un résultat, pas une panne de la sonde.
//!
//! **Ce qui N'EST PAS exclu, et ne peut pas l'être avec l'interface
//! imposée** : `DesktopCapture::sur_sortie` appelle `ouvrir`, qui appelle
//! `creer_peripherique` (`agent/src/capture/ouverture.rs:83-139`) — et cette
//! fonction construit INÉVITABLEMENT un `ID3D11Device` + `ID3D11DeviceContext`
//! réels par duplication (`D3D11CreateDevice` avec
//! `D3D11_CREATE_DEVICE_BGRA_SUPPORT`), puis leur pose
//! `SetMultithreadProtected(true)` — précisément la préparation documentée
//! comme nécessaire au partage Media Foundation qu'utiliserait un encodeur.
//! La sonde suit cette interface sans la modifier (la modifier était hors
//! périmètre) ; elle ne PEUT pas ouvrir une duplication sans ce périphérique.
//! **Conséquence pour la lecture de la campagne** : l'étage « ajouter un
//! périphérique D3D11 » de l'escalade H3 est déjà franchi PAR CONSTRUCTION à
//! ce rang de sonde — si H3 doit être approfondie par une sonde plus épaisse,
//! le premier étage à y ajouter est l'encodeur, pas le périphérique, déjà
//! présent ici.
//!
//! Le rang de la sonde vient de `MULTIFENETRE_PLAFOND_RANG` ; chaque ligne le
//! porte, car `agent.log` mêle le porteur et toutes ses sondes par héritage de
//! `stdout` et rien d'autre ne distinguerait l'émetteur (piège relevé en D1).
//!
//! `DesktopCapture::sur_sortie` retente une ouverture refusée pour
//! indisponibilité passagère pendant `capture_reprise::DUREE_FENETRE_OUVERTURE`
//! (3 s). Un refus rapporté ici est donc **déjà un refus durable**, pas un
//! transitoire — ce n'est pas plus fort que cela.

use anyhow::{Context, Result};

/// Fichier témoin du porteur : sa présence ordonne la sortie.
pub(super) fn chemin_arret() -> std::path::PathBuf {
    std::env::temp_dir().join("plafond-arret")
}

/// Fichier de verdict d'une sonde.
pub(super) fn chemin_verdict(rang: u8) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("plafond-sonde-{rang}.verdict"))
}

/// Verdict de secours d'un rang illisible (nom FIXE, voir `sonder` ci-dessous
/// pour pourquoi). Jamais lu par le porteur — ce n'est qu'un diagnostic pour
/// qui fouille `%TEMP%` — mais nommée ici pour que le porteur puisse la
/// nettoyer avant un tirage sans dupliquer le nom du fichier.
pub(super) fn chemin_verdict_rang_invalide() -> std::path::PathBuf {
    std::env::temp_dir().join("plafond-sonde-rang-invalide.verdict")
}

// `pub(in super::super)` et non `pub(crate)` : c'est la visibilité la plus
// étroite qui satisfait encore le réexport `pub(super) use sonde::sonder;` de
// `plafond.rs` — `super::super` désigne `multifenetre` depuis `sonde`, ce qui
// correspond exactement à la portée `pub(in multifenetre)` que ce réexport
// déclare. Un `pub(super)` ici (portée `plafond` seul) serait trop étroit et
// ferait échouer ce réexport en E0364 (« sonder is private, and cannot be
// re-exported ») — vérifié à la Task 8. `pub(crate)` compilerait aussi mais
// ouvrirait `sonder` à tout le crate sans raison : rien en dehors de
// `multifenetre` n'en a besoin.
pub(in super::super) fn sonder(sorties: &[String]) -> Result<()> {
    let rang_brute = std::env::var("MULTIFENETRE_PLAFOND_RANG").unwrap_or_else(|_| "0".to_string());
    let rang: u8 = match rang_brute.parse().context("MULTIFENETRE_PLAFOND_RANG doit être un entier")
    {
        Ok(rang) => rang,
        Err(erreur) => {
            // Sans ce bloc, le `?` d'origine sortait AVANT toute écriture de
            // verdict : le porteur (Task 10) borne son attente (voir
            // `attendre_le_verdict`, `plafond.rs`) et rend MORTE si rien
            // n'arrive, mais un rang malformé se lirait alors comme un
            // plantage de sonde (0xc0000005 et consorts) plutôt que comme ce
            // qu'il est. Un `u8` n'existe pas ici pour nommer le
            // fichier que `chemin_verdict` produirait normalement : on dépose
            // donc un verdict de secours, sous un nom FIXE plutôt que dérivé
            // de la valeur brute — l'interpoler dans un composant de chemin
            // laisserait passer des séquences `..` significatives pour la
            // résolution Windows (`Path` y traite `\` et `/` comme
            // séparateurs) et pourrait écrire hors de `%TEMP%`. La valeur
            // brute reste dans la trace ci-dessous, où l'interpoler ne pose
            // aucun risque. Best-effort (l'échec d'écriture n'aggrave rien :
            // cette trace reste le diagnostic de référence).
            tracing::error!(rang_brute = %rang_brute, %erreur, "MULTIFENETRE_PLAFOND_RANG illisible");
            let secours = chemin_verdict_rang_invalide();
            let _ = std::fs::write(&secours, format!("KO RANG_INVALIDE {rang_brute}"));
            return Err(erreur);
        }
    };

    tracing::info!(sonde = rang, sorties = ?sorties, "sonde démarrée");

    // Les duplications sont TENUES dans ce vecteur : les relâcher libérerait
    // la place et la mesure ne mesurerait plus rien.
    let mut tenues = Vec::new();
    let mut verdict = String::from("OK");
    for (rang_local, nom) in sorties.iter().enumerate() {
        match crate::capture::DesktopCapture::sur_sortie(nom) {
            Ok(duplication) => {
                tracing::info!(
                    sonde = rang,
                    duplication = rang_local + 1,
                    %nom,
                    "duplication ouverte"
                );
                tenues.push(duplication);
            }
            Err(erreur) => {
                // Le HRESULT EXACT, et le rang : c'est la seule donnée que la
                // matrice exploite.
                tracing::error!(
                    sonde = rang,
                    duplication = rang_local + 1,
                    %nom,
                    %erreur,
                    "duplication REFUSÉE"
                );
                verdict = format!("KO {erreur} {nom}");
                break;
            }
        }
    }

    // Préexistant, hors périmètre de cette correction : un échec d'écriture
    // ICI (rang valide, verdict légitime) sort par `?` sans verdict déposé.
    std::fs::write(chemin_verdict(rang), &verdict)
        .with_context(|| format!("écriture du verdict de la sonde {rang}"))?;
    tracing::info!(sonde = rang, ouvertes = tenues.len(), %verdict, "verdict déposé");

    // Tenir jusqu'au signal du porteur. Les duplications restent ouvertes tant
    // que `tenues` est vivant.
    while !chemin_arret().exists() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    tracing::info!(sonde = rang, "arrêt demandé, relâchement des duplications");
    Ok(())
}
