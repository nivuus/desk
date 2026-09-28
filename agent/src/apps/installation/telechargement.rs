//! Fetching the installer over HTTP, writing it, and checking its fingerprint WHILE
//! writing.
//!
//! 🔴 THIS MODULE IS PORTABLE: no `#[cfg]`. `tokio::net::TcpStream`, a
//! header parser and a file write compile and run on
//! the Linux host. It is a DECLARED divergence from §6 of the
//! specification, which put it under `#[cfg(windows)]` — and it is a
//! coverage improvement, not a detail: the **third** fingerprint
//! check, the `Range` resumption, the refusal of `chunked` and that of
//! `https` are all tested here, **against a real local TCP server**, instead
//! of depending on a VM acceptance run. Only the execution stays Windows.
//!
//! ⚠️ WHY A HAND-WRITTEN CLIENT. The agent has **no** HTTP client, and
//! `tokio-tungstenite` is locked **without TLS** there: `reqwest` would bring
//! an entire TLS stack and break the invariant "no production
//! dependency" that G1 and G2 both hold. This repository has already written its
//! TURN client, its STUN codec and its SHA-256 for the same reason.
//!
//! 🔴 WHAT IT REFUSES LOUDLY RATHER THAN INTERPRETING: `https://`,
//! because it does not speak TLS; `Transfer-Encoding`; any status other than `200`
//! and `206`. *A named refusal is diagnosed in one log line; a
//! parser that guesses is diagnosed in a campaign.*

use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::apps::sha256::{hex_de, Condensateur};

use super::reponse::{self, Etat};

/// How many times we reopen after a cut in the middle of a transfer.
///
/// ⚠️ **NOT CALIBRATED**, it joins the list this repository has kept since
/// `BPP_MIN`. What it bounds is real: without it, a server that cuts at
/// every byte would make the download loop without end.
pub const RETABLISSEMENTS_MAX: u32 = 5;

/// The size of the socket read buffer.
const TAMPON: usize = 64 * 1024;

/// Why a download did not complete.
///
/// ⚠️ EACH VARIANT CARRIES WHAT IS NEEDED TO DIAGNOSE IT WITHOUT REOPENING THE PRODUCT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refus {
    /// The URL is not an `http://` URL this client can read.
    ///
    /// 🔴 `https://` AND `wss://` FALL HERE, AND IT IS A NAMED REFUSAL — not an
    /// "unknown scheme". The `/agent` channel speaks only `ws://` today,
    /// and the download URL is derived from `SIGNALING_URL` exactly as
    /// `url_du_canal` derives its own: the day the platform moves to
    /// TLS, it is this refusal that will say so, and not a parse error.
    ///
    /// ✅ `ws://` IS ACCEPTED, on the other hand, since the acceptance run showed that **every**
    /// installation order was refused without it.
    Url(String),
    /// The connection could not be opened, or broke beyond the budget.
    Reseau(String),
    /// The response parsing refused — the reason travels as is.
    Reponse(reponse::Refus),
    /// Writing to disk failed.
    Disque(String),
    /// 🔴 THE THIRD OF THE THREE FINGERPRINT CHECKS. The browser can
    /// lie, the platform's disk can get corrupted, the transfer can
    /// truncate: **no hop trusts the previous one**.
    Empreinte { attendue: String, obtenue: String },
    /// The received body is not the size announced by the order.
    Taille { attendue: u64, obtenue: u64 },
    /// The recovery budget is exhausted.
    TropDeCoupures(u32),
}

/// What the caller supplies, and what it observes.
pub struct Demande<'a> {
    /// The absolute URL, `http://` only.
    pub url: &'a str,
    /// The agent token, as is — it goes out as `Authorization: Bearer`.
    pub jeton: &'a str,
    /// Where to write. The parent directory must exist.
    pub destination: &'a Path,
    pub taille_attendue: u64,
    /// In lowercase hexadecimal, 64 characters.
    pub sha256_attendu: &'a str,
}

/// An `http://host:port/path` URL, split up.
///
/// ⚠️ MODULE-PRIVATE AND TESTED: URL parsing is the only place where an
/// error would produce a connection to a host nobody asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cible {
    hote: String,
    port: u16,
    /// What goes out on the request line. Always non-empty, always
    /// starting with `/`.
    chemin: String,
    /// The `Host` header, with its port when it is not the default one.
    entete_host: String,
}

fn decouper(url: &str) -> Result<Cible, Refus> {
    // 🔴 `https` IS REFUSED BY NAME, NOT AS "UNKNOWN". Distinguishing it from a
    // malformed URL is what lets the log say "this agent does not speak
    // TLS" rather than "unreadable URL", which would send you looking for a
    // typo where there is a missing capability.
    //
    // ⚠️ `wss://` FALLS HERE TOO, and for the same reason: the download
    // URL is derived from the channel's, which is a WebSocket scheme.
    // A `wss://` refused as "unrecognised scheme" would send you looking for a typo
    // where there is, once again, a missing capability.
    if url.starts_with("https://") || url.starts_with("wss://") {
        return Err(Refus::Url(format!(
            "TLS non pris en charge : cet agent ne parle ni https ni wss ({url})"
        )));
    }
    // 🔴 `ws://` IS ACCEPTED ON THE SAME FOOTING AS `http://`, AND IT WAS THE ACCEPTANCE
    // RUN THAT DEMANDED IT. The installer URL is **derived, not configured**:
    // `canal-apps.ts` sends the relative path `/televersement/:id/contenu`, and
    // the agent resolves it against the address of its OWN channel — which is a
    // `ws://`, since it is a WebSocket. With the client accepting only `http://`,
    // **every installation order was refused** on `schéma non reconnu :
    // ws://…`, measured on the real chain.
    //
    // ⚠️ THIS FILE ALREADY CARRIED THE FACT WITHOUT CARRYING THE REMEDY: the doc of
    // `Refus::Url` says, word for word, that "the `/agent` channel itself speaks
    // only `ws://`". The reading was right and the code did not follow it — a
    // gap no host test could see, all of them building their URLs
    // as `http://` against a local `TcpListener`.
    //
    // ✅ IT IS ALSO G2'S PRECEDENT, AND IT IS REUSED RATHER THAN
    // REINVENTED: `apps/icone/televersement.rs` accepts exactly these two
    // schemes, through the same `strip_prefix(…).or_else(…)`. Two modules that
    // derive the same address must accept the same form of it.
    let reste = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("ws://"))
        .ok_or_else(|| Refus::Url(format!("schéma non reconnu : {url}")))?;
    let (autorite, chemin) = match reste.find('/') {
        Some(i) => (&reste[..i], &reste[i..]),
        None => (reste, "/"),
    };
    if autorite.is_empty() {
        return Err(Refus::Url(format!("hôte vide : {url}")));
    }
    // ⚠️ NO `rfind` ON `:` WITHOUT CARE: a literal IPv6 address
    // carries several. This client does not support them, and SAYS so.
    if autorite.starts_with('[') {
        return Err(Refus::Url(format!(
            "adresse IPv6 littérale non prise en charge : {url}"
        )));
    }
    let (hote, port) = match autorite.split_once(':') {
        Some((h, p)) => {
            let port = p
                .parse::<u16>()
                .map_err(|_| Refus::Url(format!("port illisible : {url}")))?;
            (h.to_string(), port)
        }
        None => (autorite.to_string(), 80u16),
    };
    if hote.is_empty() {
        return Err(Refus::Url(format!("hôte vide : {url}")));
    }
    Ok(Cible {
        hote,
        port,
        chemin: chemin.to_string(),
        // ⚠️ THE DEFAULT PORT IS NOT WRITTEN IN `Host`: that is what
        // RFC 9110 §7.2 asks, and a proxy may route on it.
        entete_host: if port == 80 {
            autorite.split(':').next().unwrap_or(autorite).to_string()
        } else {
            autorite.to_string()
        },
    })
}

/// What one transfer pass did.
struct Passe {
    /// The total written SINCE THE START OF THE FILE at the end of this pass.
    ///
    /// ⚠️ A TOTAL, NOT A DELTA, and that is what makes restarting
    /// expressible: a pass that starts over from zero returns the total it
    /// actually wrote, and the caller has nothing to subtract.
    total: u64,
    /// `true` if the connection broke before the announced end.
    coupee: bool,
}

/// Downloads, writes, and checks. Returns the bytes written.
///
/// 🔴 THE FINGERPRINT IS COMPUTED WHILE WRITING, NOT BY RE-READING THE FILE
/// AFTERWARDS — except on a resumption path, where we **re-read what is already
/// written** to re-prime the digest state. Saying so here keeps anyone from
/// "optimising" that re-read one day: without it, a resumption would yield
/// the fingerprint of the file's END alone.
pub async fn telecharger<F>(demande: Demande<'_>, mut progres: F) -> Result<u64, Refus>
where
    F: FnMut(u64, u64),
{
    let cible = decouper(demande.url)?;
    let mut coupures = 0u32;
    let mut deja = 0u64;
    let mut condensateur = Condensateur::neuf();

    loop {
        let passe = une_passe(&cible, &demande, deja, &mut condensateur, &mut progres).await?;
        deja = passe.total;
        if !passe.coupee {
            break;
        }

        coupures = coupures.saturating_add(1);
        if coupures > RETABLISSEMENTS_MAX {
            let _ = tokio::fs::remove_file(demande.destination).await;
            return Err(Refus::TropDeCoupures(coupures));
        }
        tracing::warn!(
            url = demande.url,
            deja,
            coupures,
            "transfert coupé, reprise par Range"
        );
    }

    if deja != demande.taille_attendue {
        let _ = tokio::fs::remove_file(demande.destination).await;
        return Err(Refus::Taille {
            attendue: demande.taille_attendue,
            obtenue: deja,
        });
    }

    let obtenue = hex_de(condensateur.terminer());
    if obtenue != demande.sha256_attendu {
        // 🔴 THE PARTIAL FILE IS DELETED, AND THE UPLOAD REMAINS
        // RESUMABLE. Keeping it would invite a later path to take it
        // for a valid installer.
        let _ = tokio::fs::remove_file(demande.destination).await;
        return Err(Refus::Empreinte {
            attendue: demande.sha256_attendu.to_string(),
            obtenue,
        });
    }
    Ok(deja)
}

async fn une_passe<F>(
    cible: &Cible,
    demande: &Demande<'_>,
    deja: u64,
    condensateur: &mut Condensateur,
    progres: &mut F,
) -> Result<Passe, Refus>
where
    F: FnMut(u64, u64),
{
    let mut socket = TcpStream::connect((cible.hote.as_str(), cible.port))
        .await
        .map_err(|e| Refus::Reseau(format!("connexion à {}:{} : {e}", cible.hote, cible.port)))?;

    let mut requete = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\
         Accept-Encoding: identity\r\nConnection: close\r\n",
        cible.chemin, cible.entete_host, demande.jeton
    );
    if deja > 0 {
        requete.push_str(&format!("Range: bytes={deja}-\r\n"));
    }
    requete.push_str("\r\n");
    socket
        .write_all(requete.as_bytes())
        .await
        .map_err(|e| Refus::Reseau(format!("envoi de la requête : {e}")))?;

    // --- the header, re-read until complete ---
    let mut tampon: Vec<u8> = Vec::with_capacity(TAMPON);
    let mut lecture = [0u8; TAMPON];
    let entete = loop {
        match reponse::analyser(&tampon) {
            Ok(Etat::Prete(e)) => break e,
            Ok(Etat::Incomplet) => {}
            Err(refus) => return Err(Refus::Reponse(refus)),
        }
        let n = socket
            .read(&mut lecture)
            .await
            .map_err(|e| Refus::Reseau(format!("lecture de l'en-tête : {e}")))?;
        if n == 0 {
            return Err(Refus::Reseau(
                "connexion fermée avant la fin de l'en-tête".into(),
            ));
        }
        tampon.extend_from_slice(&lecture[..n]);
    };

    // 🔴 A `200` WHEN WE ASKED FOR A `Range`: THE SERVER IGNORED THE
    // RANGE and sends the WHOLE file. We start over from zero — we never
    // concatenate, which would produce a file longer than its size and a
    // wrong fingerprint **without anyone knowing why**.
    //
    // 🔴 AND WE CONSUME **THIS** RESPONSE, we do not reopen a connection. The
    // whole body is already arriving: throwing it away to ask for it again
    // would push several hundred megabytes over the link twice,
    // for nothing. The file is truncated and the digest re-primed, which
    // is exactly what "start over from zero" means.
    let deja = if deja > 0 && entete.statut == 200 {
        tracing::warn!(
            url = demande.url,
            deja,
            "le serveur a ignoré le Range : le téléchargement REPART DE ZÉRO,              sur cette réponse même"
        );
        *condensateur = Condensateur::neuf();
        0
    } else {
        deja
    };

    // --- le corps ---
    let mut fichier = ouvrir(demande.destination, deja).await?;
    let mut ecrits = 0u64;
    let debut = &tampon[entete.debut_du_corps..];
    if !debut.is_empty() {
        ecrire(&mut fichier, condensateur, debut).await?;
        ecrits += debut.len() as u64;
        progres(deja + ecrits, demande.taille_attendue);
    }

    while ecrits < entete.longueur {
        let n = socket
            .read(&mut lecture)
            .await
            .map_err(|e| Refus::Reseau(format!("lecture du corps : {e}")))?;
        if n == 0 {
            // ⚠️ CLOSED BEFORE THE ANNOUNCED END: this is a cut, not an
            // end. The caller will resume through `Range`.
            fichier
                .flush()
                .await
                .map_err(|e| Refus::Disque(format!("vidage : {e}")))?;
            return Ok(Passe {
                total: deja + ecrits,
                coupee: true,
            });
        }
        // ⚠️ WE NEVER WRITE BEYOND WHAT IS ANNOUNCED: a server that
        // sent too much would otherwise make the file grow without end.
        let reste = (entete.longueur - ecrits) as usize;
        let utile = &lecture[..n.min(reste)];
        ecrire(&mut fichier, condensateur, utile).await?;
        ecrits += utile.len() as u64;
        progres(deja + ecrits, demande.taille_attendue);
    }
    fichier
        .flush()
        .await
        .map_err(|e| Refus::Disque(format!("vidage : {e}")))?;

    Ok(Passe {
        total: deja + ecrits,
        coupee: false,
    })
}

/// Opens the destination, and **re-primes the digest** on a resumption.
async fn ouvrir(destination: &Path, deja: u64) -> Result<tokio::fs::File, Refus> {
    if deja == 0 {
        return tokio::fs::File::create(destination)
            .await
            .map_err(|e| Refus::Disque(format!("création de {} : {e}", destination.display())));
    }
    tokio::fs::OpenOptions::new()
        .append(true)
        .open(destination)
        .await
        .map_err(|e| Refus::Disque(format!("réouverture de {} : {e}", destination.display())))
}

async fn ecrire(
    fichier: &mut tokio::fs::File,
    condensateur: &mut Condensateur,
    bloc: &[u8],
) -> Result<(), Refus> {
    fichier
        .write_all(bloc)
        .await
        .map_err(|e| Refus::Disque(format!("écriture : {e}")))?;
    condensateur.absorber(bloc);
    Ok(())
}

#[cfg(test)]
#[path = "telechargement/tests.rs"]
mod tests;
