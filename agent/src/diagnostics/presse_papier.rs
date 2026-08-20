//! Sonde P0 du sous-bloc P1 (presse-papier) : le compteur de séquence du
//! presse-papier Windows se comporte-t-il comme la documentation l'annonce ?
//!
//! La spécification (§8) déclare ce point **non mesuré** : elle s'appuie sur
//! la documentation de `GetClipboardSequenceNumber`, la VM ayant été occupée
//! quand elle a été écrite. Cette sonde prend la mesure, et elle est une
//! **porte éliminatoire** : trois de ses cinq verdicts rendent le mécanisme
//! de détection retenu (D2) non livrable en l'état.
//!
//! ⚠️ **ELLE ÉCRIT LE PRESSE-PAPIER DE LA VM, et le détruit donc.** Les
//! phases C et D posent `SetClipboardData(CF_UNICODETEXT)` pour répondre aux
//! questions Q3 et Q2. C'est un geste de SONDE : **le produit, lui, n'écrit
//! jamais le presse-papier en P1** — il ne fait que le lire. Ne pas
//! confondre les deux (décision D-P1-7 du plan).
//!
//! ⚠️ **Elle ne journalise JAMAIS le texte du presse-papier** — une empreinte
//! tronquée et une longueur, jamais le contenu. C'est une ressource privée de
//! l'utilisateur de la VM, et un journal versé dans git est public au dépôt.
//!
//! ⚠️ **Elle se lance SEULE**, sans `SUPERVISEUR` : voir le commentaire du
//! bras d'aiguillage dans `diagnostics.rs` (divergence E10 du plan).
//!
//! Aucun verdict ne se juge sur un code de retour. La phase C juge sur le
//! **mouvement du compteur relu**, jamais sur le succès de
//! `SetClipboardData` — c'est la doctrine que ce dépôt s'est donnée après
//! qu'un `ChangeDisplaySettingsExW` a rendu `0` sur une sortie qui n'avait
//! pas bougé d'un pixel (sous-bloc D8).

use anyhow::Result;

/// Nombre de relevés de la phase A, et leur espacement.
const RELEVES_REPOS: usize = 3;
const PAS_REPOS: std::time::Duration = std::time::Duration::from_millis(250);
/// Cadence de la phase B : 4 Hz, la même que `PERIODE_PRESSE_PAPIER`.
const PAS_OBSERVATION: std::time::Duration = std::time::Duration::from_millis(250);

/// Empreinte d'un texte : SHA-1 tronqué à 12 caractères hexadécimaux.
///
/// ⚠️ **SHA-1 et non SHA-256, contrairement au plan (tâche 1)**, et c'est
/// délibéré : `sha1` est déjà une dépendance de cette caisse (bail TURN),
/// `sha2` ne l'est pas, et le plan pose en tête que **P1 n'ajoute aucune
/// dépendance**. La propriété recherchée est « distinguer deux textes sans
/// jamais écrire l'un des deux », pas une résistance cryptographique : les
/// deux fonctions la rendent également.
fn empreinte(texte: &str) -> String {
    use sha1::{Digest, Sha1};
    let condense = Sha1::digest(texte.as_bytes());
    condense.iter().take(6).map(|octet| format!("{octet:02x}")).collect()
}

/// Ce que la sonde a lu du presse-papier à un instant donné.
struct Lecture {
    octets_utf16: usize,
    octets_utf8: usize,
    empreinte: String,
}

/// Exécute la sonde pendant `secondes` de phase B.
pub fn executer(secondes_texte: &str) -> Result<()> {
    let secondes: u64 = secondes_texte.trim().parse().unwrap_or(40);

    tracing::info!(
        secondes,
        "P0 DEBUT sonde presse-papier — elle ECRIT le presse-papier de la VM (phases C et D)"
    );

    // ---- Phase A : le compteur existe-t-il, et est-il STABLE au repos ? ----
    let mut repos = Vec::with_capacity(RELEVES_REPOS);
    for i in 0..RELEVES_REPOS {
        if i > 0 {
            std::thread::sleep(PAS_REPOS);
        }
        let seq = win::numero_de_sequence();
        tracing::info!(releve = i, seq, "P0 A repos");
        repos.push(seq);
    }
    // `GetClipboardSequenceNumber` rend 0 quand le processus n'a pas l'accès
    // `WINSTA_ACCESSCLIPBOARD` sur la station de fenêtres. Trois zéros ne
    // sont donc pas « un compteur stable » : c'est un compteur ABSENT, et le
    // verdict doit pouvoir le dire — un verdict positif exige que la chose
    // mesurée existe (piège de D9, `survit=true` rendu par une sortie
    // disparue).
    let compteur_absent = repos.iter().all(|&s| s == 0);
    let stable_au_repos = repos.windows(2).all(|paire| paire[0] == paire[1]);
    let q1 = if compteur_absent {
        "NON-MESURABLE-compteur-nul"
    } else if stable_au_repos {
        "stable"
    } else {
        "INSTABLE-au-repos"
    };
    tracing::info!(q1, seq_min = repos.iter().min(), seq_max = repos.iter().max(), "P0 A bilan");

    if compteur_absent {
        tracing::warn!(
            "P0 NON MESURABLE : GetClipboardSequenceNumber rend 0 aux trois releves — \
             le processus n'a probablement pas WINSTA_ACCESSCLIPBOARD. Les phases B a D \
             ne peuvent rien mesurer, elles sont sautees."
        );
        tracing::info!(
            q1,
            q1bis = "NON-MESUREE",
            q2 = "NON-MESUREE",
            q3 = "NON-MESUREE",
            echecs_open = 0,
            tentatives_open = 0,
            seq_debut = repos[0],
            seq_fin = repos[RELEVES_REPOS - 1],
            "P0 BILAN"
        );
        return Ok(());
    }

    // ---- Phase B : une copie faite à la main fait-elle bouger le compteur ? ----
    let seq_debut = *repos.last().expect("RELEVES_REPOS > 0");
    let mut reference = seq_debut;
    let mut mouvements = 0usize;
    let mut lectures = 0usize;
    let mut echecs_open = 0usize;
    let mut tentatives_open = 0usize;
    tracing::info!(
        secondes,
        seq_debut,
        "P0 B debut — COPIER MAINTENANT trois textes distincts a la main dans la VM, \
         puis RECOPIER le troisieme a l'identique"
    );
    let fin = std::time::Instant::now() + std::time::Duration::from_secs(secondes);
    while std::time::Instant::now() < fin {
        std::thread::sleep(PAS_OBSERVATION);
        let seq = win::numero_de_sequence();
        if seq == reference {
            continue;
        }
        mouvements += 1;
        tentatives_open += 1;
        match win::lire_texte() {
            Ok(Some(texte)) => {
                lectures += 1;
                let lecture = mesurer(&texte);
                tracing::info!(
                    seq,
                    precedent = reference,
                    octets_utf16 = lecture.octets_utf16,
                    octets_utf8 = lecture.octets_utf8,
                    empreinte = lecture.empreinte,
                    "P0 B mouvement, texte lu"
                );
            }
            Ok(None) => {
                tracing::info!(seq, precedent = reference, "P0 B mouvement, aucun CF_UNICODETEXT");
            }
            Err(erreur) => {
                echecs_open += 1;
                tracing::warn!(seq, precedent = reference, %erreur, "P0 B mouvement, ouverture refusee");
            }
        }
        reference = seq;
    }
    let q1bis = if mouvements == 0 { "AUCUN-MOUVEMENT" } else { "bouge" };
    tracing::info!(q1bis, mouvements, lectures, echecs_open, "P0 B bilan");

    // ---- Phase C : notre PROPRE écriture fait-elle bouger le compteur ? ----
    let nonce = format!("{:x}", std::process::id());
    let notre_texte = format!("sonde-presse-papier-{nonce}");
    let avant_c = win::numero_de_sequence();
    let ecrit_c = win::ecrire_texte(&notre_texte);
    std::thread::sleep(PAS_REPOS);
    let apres_c = win::numero_de_sequence();
    // On juge sur le MOUVEMENT relu, jamais sur `ecrit_c`.
    let q3 = if apres_c != avant_c { "bouge" } else { "PAS-DE-MOUVEMENT" };
    tracing::info!(
        q3,
        avant = avant_c,
        apres = apres_c,
        api_annonce_succes = ecrit_c.is_ok(),
        empreinte = empreinte(&notre_texte),
        octets = notre_texte.len(),
        "P0 C notre ecriture"
    );

    // ---- Phase D : une réécriture IDENTIQUE fait-elle bouger le compteur ? ----
    let avant_d = win::numero_de_sequence();
    let ecrit_d = win::ecrire_texte(&notre_texte);
    std::thread::sleep(PAS_REPOS);
    let apres_d = win::numero_de_sequence();
    let q2 = if apres_d != avant_d { "bouge" } else { "PAS-DE-MOUVEMENT" };
    tracing::info!(
        q2,
        avant = avant_d,
        apres = apres_d,
        api_annonce_succes = ecrit_d.is_ok(),
        "P0 D reecriture identique"
    );

    // ---- Phase E : le bilan ----
    tracing::info!(
        q1,
        q1bis,
        q2,
        q3,
        mouvements,
        lectures,
        echecs_open,
        tentatives_open,
        seq_debut,
        seq_fin = apres_d,
        "P0 BILAN"
    );
    Ok(())
}

/// Ce qu'on mesure d'un texte lu — jamais le texte lui-même.
fn mesurer(texte: &str) -> Lecture {
    Lecture {
        octets_utf16: texte.encode_utf16().count() * 2,
        octets_utf8: texte.len(),
        empreinte: empreinte(texte),
    }
}

mod win {
    use anyhow::{Context, Result};
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
        OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    pub fn numero_de_sequence() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }

    /// Ouvre le presse-papier, lit `CF_UNICODETEXT`, referme.
    ///
    /// `Ok(None)` = le presse-papier ne porte pas de texte Unicode (une image,
    /// par exemple) ; `Err` = l'ouverture a été refusée, ce qui est NORMAL
    /// sous Windows (une autre application le tient) et non une panne.
    pub fn lire_texte() -> Result<Option<String>> {
        unsafe { OpenClipboard(None) }.context("OpenClipboard")?;
        let resultat = (|| unsafe {
            let poignee = match GetClipboardData(CF_UNICODETEXT.0 as u32) {
                Ok(poignee) if !poignee.is_invalid() => poignee,
                _ => return Ok(None),
            };
            let global = HGLOBAL(poignee.0);
            let pointeur = GlobalLock(global) as *const u16;
            if pointeur.is_null() {
                anyhow::bail!("GlobalLock a rendu un pointeur nul");
            }
            let mut longueur = 0usize;
            while *pointeur.add(longueur) != 0 {
                longueur += 1;
            }
            let unites = std::slice::from_raw_parts(pointeur, longueur);
            let texte = String::from_utf16_lossy(unites);
            let _ = GlobalUnlock(global);
            Ok(Some(texte))
        })();
        let _ = unsafe { CloseClipboard() };
        resultat
    }

    /// Écrit `texte` dans le presse-papier — **geste de sonde uniquement**.
    pub fn ecrire_texte(texte: &str) -> Result<()> {
        let mut unites: Vec<u16> = texte.encode_utf16().collect();
        unites.push(0);
        unsafe { OpenClipboard(None) }.context("OpenClipboard")?;
        let resultat = (|| unsafe {
            EmptyClipboard().context("EmptyClipboard")?;
            let octets = unites.len() * std::mem::size_of::<u16>();
            let global = GlobalAlloc(GMEM_MOVEABLE, octets).context("GlobalAlloc")?;
            let pointeur = GlobalLock(global) as *mut u16;
            if pointeur.is_null() {
                anyhow::bail!("GlobalLock a rendu un pointeur nul");
            }
            std::ptr::copy_nonoverlapping(unites.as_ptr(), pointeur, unites.len());
            let _ = GlobalUnlock(global);
            // Le presse-papier prend possession du bloc : ne pas le libérer.
            SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(global.0)))
                .context("SetClipboardData")?;
            Ok(())
        })();
        let _ = unsafe { CloseClipboard() };
        resultat
    }
}
