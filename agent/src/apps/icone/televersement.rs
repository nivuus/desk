//! Uploading icons to the platform, over HTTP.
//!
//! 🔴 THE BYTES NEVER GO THROUGH THE `/agent` CHANNEL. It is the
//! literal transposition of decision D7 of the specification ("the channel
//! never carries an installer"), and the reasons are the same: the
//! channel is JSON, it carries the heartbeat, and **4.4 MB in base64 there
//! would cost +33 % and block that heartbeat**. What goes through the channel
//! is an INVENTORY; what goes through here are the images, one by one.
//!
//! 🔴 **THE HTTP WRITING IS DONE BY HAND, AND IT IS A CONSTRAINT ENDURED,
//! NOT A TASTE.** Sub-block G2 forbids itself any new production
//! dependency, and **the agent has NO HTTP client**: `Cargo.lock` carries neither
//! `reqwest`, nor `ureq`, nor `hyper` — recorded by the command. `tokio-tungstenite`
//! only opens a WebSocket. An HTTP/1.1 `PUT` request fits in thirty lines
//! over a `TcpStream`; adding a crate for it would cost more than
//! it would give.
//!
//! 🔴 **NAMED CONSEQUENCE, AND IT IS REAL: THIS PATH DOES NOT DO
//! TLS.** No TLS stack exists in the tree — neither `rustls` nor `native-tls`,
//! recorded in `Cargo.lock` on 20 August 2026 —, so a `SIGNALING_URL` with
//! `wss://` or `https://` **is EXPLICITLY REFUSED, with its trace**, never
//! attempted in clear text and never silent. That is today's nominal case:
//! the agent reaches the platform on the internal network (`ws://192.168.3.1:8080`
//! by default in `scripts/run-agent.sh`), and it is **nginx that terminates TLS
//! for the BROWSER**, never for the agent (specification §9 of sub-project
//! ⑤). **The day the agent has to cross an untrusted link, it will need a
//! TLS stack — and it will be a dependency to decide on, not to slip in.**
//!
//! ⚠️ IT IS ONLY CHECKED BY `cargo check --target x86_64-pc-windows-gnu`.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::sync::watch;

use super::magasin::Magasin;
use crate::plateforme::Identite;

/// ⚠️ **NOT CALIBRATED.** An icon weighs ~30 KB on average on a local network;
/// ten seconds is an eyeballed upper bound. What it bounds is the WORST case of an
/// upload that never returns — without it, the apps thread
/// would stay blocked and reconciliation would stop, which would cost far more
/// than one icon.
const DELAI: Duration = Duration::from_secs(10);

/// Uploads those of `empreintes` that the store carries.
///
/// ⚠️ **A FINGERPRINT REQUESTED BUT MISSING FROM THE STORE IS SKIPPED WITH ITS
/// TRACE, never an error**: the reconciliation may have changed between the announcement
/// and the request, and the announced icon then no longer belongs to the current
/// catalogue. It is the reasoning of "an unknown key in `disparues` is
/// ignored", on the platform side.
///
/// ⚠️ **A FAILURE KILLS NEITHER THE LOOP NOR THE CHANNEL.** The platform will ask again
/// at the next reconciliation: the complete resend is the safety net, exactly
/// as for a lost `Catalogue`.
pub fn honorer(
    magasin: &Magasin,
    empreintes: &[String],
    base: &str,
    identite: &watch::Receiver<Option<Identite>>,
) {
    let base = match base_http(base) {
        Ok(b) => b,
        Err(erreur) => {
            tracing::warn!(base, %erreur, "aucun televersement d'icone possible");
            return;
        }
    };
    let Some(jeton) = identite.borrow().as_ref().map(|i| i.jeton.clone()) else {
        // Without a token there is no agent identity: the platform would refuse
        // with `403`, and insisting would cost a round trip per icon.
        tracing::warn!("aucun jeton d'agent : televersement d'icones differe");
        return;
    };

    let (mut envoyees, mut sautees, mut echouees) = (0usize, 0usize, 0usize);
    let mut deja = BTreeSet::new();
    for empreinte in empreintes {
        if !deja.insert(empreinte.clone()) {
            continue;
        }
        let Some(octets) = magasin.octets(empreinte) else {
            sautees += 1;
            tracing::debug!(
                empreinte,
                "empreinte demandee absente du magasin courant, sautee \
                 (la reconciliation a change depuis l'annonce)"
            );
            continue;
        };
        match envoyer(&base, &jeton, empreinte, octets) {
            Ok(()) => envoyees += 1,
            Err(erreur) => {
                echouees += 1;
                tracing::warn!(empreinte, %erreur, "televersement d'icone echoue");
            }
        }
    }
    tracing::info!(
        demandees = empreintes.len(),
        envoyees,
        sautees,
        echouees,
        "televersement d'icones termine"
    );
}

/// The `host:port` to reach, derived from the channel URL.
///
/// ⚠️ IT IS DERIVED FROM `SIGNALING_URL` RATHER THAN READ FROM ONE MORE
/// VARIABLE: the `/agent` channel and the HTTP routes live on the SAME service, and
/// two variables would diverge the day one was updated without
/// the other — a silent failure whose only symptom would be icons that
/// never arrive.
fn base_http(signaling_url: &str) -> Result<String> {
    let url = signaling_url.trim();
    // 🔴 EXPLICIT REFUSAL, NEVER A CLEAR-TEXT ATTEMPT: see the header.
    if url.starts_with("wss://") || url.starts_with("https://") {
        bail!(
            "TLS demande ({url}) mais l'agent n'a AUCUNE pile TLS : le televersement \
             d'icones ne sait parler qu'en clair, et il refuse plutot que d'essayer"
        );
    }
    // ⚠️ THE SCHEME IS REMOVED BEFORE ANY OTHER CUT. Trimming the
    // trailing slashes first would turn `ws://` into `ws:`, which what follows would take
    // for an authority — an empty URL would become an address, and the
    // connection would fail far from its cause.
    let sans = url
        .strip_prefix("ws://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    // Any path is removed: only the authority is kept.
    let autorite = sans.split('/').next().unwrap_or(sans).trim();
    if autorite.is_empty() {
        bail!("URL de plateforme sans hote : {url}");
    }
    Ok(autorite.to_string())
}

fn envoyer(autorite: &str, jeton: &str, empreinte: &str, octets: &[u8]) -> Result<()> {
    let mut flux =
        TcpStream::connect(autorite).with_context(|| format!("connexion a {autorite}"))?;
    flux.set_read_timeout(Some(DELAI))?;
    flux.set_write_timeout(Some(DELAI))?;

    let entete = format!(
        "PUT /icone/{empreinte} HTTP/1.1\r\n\
         Host: {autorite}\r\n\
         Authorization: Bearer {jeton}\r\n\
         Content-Type: application/octet-stream\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        octets.len()
    );
    flux.write_all(entete.as_bytes())
        .context("envoi de l'en-tete")?;
    flux.write_all(octets).context("envoi du corps")?;
    flux.flush().context("vidage")?;

    // ⚠️ THE RESPONSE IS READ, NOT DISCARDED. Without this read, a `413`
    // or a `400 {refus:'empreinte'}` would pass for a success, and the agent
    // would re-upload the same icon indefinitely without ever knowing why.
    let mut reponse = Vec::new();
    flux.read_to_end(&mut reponse)
        .context("lecture de la reponse")?;
    let statut = statut_http(&reponse)
        .context("reponse HTTP illisible : la plateforme n'a pas repondu ce qu'on attend")?;
    if statut != 204 {
        let corps = String::from_utf8_lossy(&reponse);
        let corps = corps.rsplit("\r\n\r\n").next().unwrap_or("");
        bail!("la plateforme a repondu {statut} au lieu de 204 : {corps}");
    }
    Ok(())
}

/// `HTTP/1.1 204 No Content` -> `204`.
fn statut_http(reponse: &[u8]) -> Option<u16> {
    let ligne = reponse.split(|&o| o == b'\r' || o == b'\n').next()?;
    let ligne = std::str::from_utf8(ligne).ok()?;
    ligne.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(test)]
#[path = "televersement/tests.rs"]
mod tests;
