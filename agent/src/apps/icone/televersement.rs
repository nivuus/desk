//! Le téléversement des icônes vers la plateforme, par HTTP.
//!
//! 🔴 LES OCTETS NE PASSENT JAMAIS PAR LE CANAL `/agent`. C'est la
//! transposition littérale de la décision D7 de la spécification (« le canal
//! ne transporte jamais un installeur »), et les raisons sont les mêmes : le
//! canal est en JSON, il porte le battement de cœur, et **4,4 Mo en base64 y
//! coûteraient +33 % et bloqueraient ce battement**. Ce qui passe par le canal
//! est un INVENTAIRE ; ce qui passe ici sont les images, une par une.
//!
//! 🔴 **L'ÉCRITURE HTTP EST FAITE À LA MAIN, ET C'EST UNE CONTRAINTE SUBIE,
//! PAS UN GOÛT.** Le sous-bloc G2 s'interdit toute dépendance de production
//! neuve, et **l'agent n'a AUCUN client HTTP** : `Cargo.lock` ne porte ni
//! `reqwest`, ni `ureq`, ni `hyper` — relevé par la commande. `tokio-tungstenite`
//! n'ouvre qu'un WebSocket. Une requête `PUT` HTTP/1.1 tient en trente lignes
//! sur un `TcpStream` ; y ajouter une caisse pour cela coûterait plus que ce
//! qu'elle rendrait.
//!
//! 🔴 **CONSÉQUENCE NOMMÉE, ET ELLE EST RÉELLE : CE CHEMIN NE FAIT PAS DE
//! TLS.** Aucune pile TLS n'existe dans l'arbre — ni `rustls`, ni `native-tls`,
//! relevé dans `Cargo.lock` le 20 août 2026 —, donc un `SIGNALING_URL` en
//! `wss://` ou `https://` **est REFUSÉ EXPLICITEMENT, avec sa trace**, jamais
//! tenté en clair et jamais silencieux. C'est le cas nominal d'aujourd'hui :
//! l'agent joint la plateforme sur le réseau interne (`ws://192.168.3.1:8080`
//! par défaut dans `scripts/run-agent.sh`), et c'est **nginx qui termine TLS
//! pour le NAVIGATEUR**, jamais pour l'agent (spécification §9 du sous-projet
//! ⑤). **Le jour où l'agent devra franchir un lien non fiable, il faudra une
//! pile TLS — et ce sera une dépendance à décider, pas à glisser.**
//!
//! ⚠️ IL N'EST VÉRIFIÉ QUE PAR `cargo check --target x86_64-pc-windows-gnu`.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::sync::watch;

use super::magasin::Magasin;
use crate::plateforme::Identite;

/// ⚠️ **NON CALIBRÉ.** Une icône pèse ~30 Ko en moyenne sur un réseau local ;
/// dix secondes sont un majorant à vue. Ce qu'il borne est le PIRE cas d'un
/// téléversement qui ne rend jamais la main — sans lui, le fil d'apps
/// resterait bloqué et la réconciliation cesserait, ce qui coûterait bien plus
/// qu'une icône.
const DELAI: Duration = Duration::from_secs(10);

/// Téléverse celles des `empreintes` que le magasin porte.
///
/// ⚠️ **UNE EMPREINTE DEMANDÉE MAIS ABSENTE DU MAGASIN EST SAUTÉE AVEC SA
/// TRACE, jamais une erreur** : la réconciliation a pu changer entre l'annonce
/// et la demande, et l'icône annoncée n'appartient alors plus au catalogue
/// courant. C'est le raisonnement de « une clé inconnue de `disparues` est
/// ignorée », côté plateforme.
///
/// ⚠️ **UN ÉCHEC NE TUE NI LA BOUCLE NI LE CANAL.** La plateforme redemandera
/// à la réconciliation suivante : le renvoi complet est le filet, exactement
/// comme pour un `Catalogue` perdu.
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
        // Sans jeton il n'y a pas d'identité d'agent : la plateforme refuserait
        // en `403`, et insister coûterait un aller-retour par icône.
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

/// `hôte:port` à joindre, dérivé de l'URL du canal.
///
/// ⚠️ ELLE EST DÉRIVÉE DE `SIGNALING_URL` PLUTÔT QUE LUE D'UNE VARIABLE DE
/// PLUS : le canal `/agent` et les routes HTTP vivent sur le MÊME service, et
/// deux variables divergeraient le jour où l'une serait mise à jour sans
/// l'autre — panne muette dont le seul symptôme serait des icônes qui
/// n'arrivent jamais.
fn base_http(signaling_url: &str) -> Result<String> {
    let url = signaling_url.trim();
    // 🔴 REFUS EXPLICITE, JAMAIS UNE TENTATIVE EN CLAIR : voir l'en-tête.
    if url.starts_with("wss://") || url.starts_with("https://") {
        bail!(
            "TLS demande ({url}) mais l'agent n'a AUCUNE pile TLS : le televersement \
             d'icones ne sait parler qu'en clair, et il refuse plutot que d'essayer"
        );
    }
    // ⚠️ LE SCHÉMA SE RETIRE AVANT TOUTE AUTRE COUPE. Rogner d'abord les
    // barres finales transformerait `ws://` en `ws:`, que la suite prendrait
    // pour une autorité — une URL vide deviendrait une adresse, et la
    // connexion échouerait loin de sa cause.
    let sans = url
        .strip_prefix("ws://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    // Le chemin éventuel est retiré : on ne garde que l'autorité.
    let autorite = sans.split('/').next().unwrap_or(sans).trim();
    if autorite.is_empty() {
        bail!("URL de plateforme sans hote : {url}");
    }
    Ok(autorite.to_string())
}

fn envoyer(autorite: &str, jeton: &str, empreinte: &str, octets: &[u8]) -> Result<()> {
    let mut flux = TcpStream::connect(autorite)
        .with_context(|| format!("connexion a {autorite}"))?;
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
    flux.write_all(entete.as_bytes()).context("envoi de l'en-tete")?;
    flux.write_all(octets).context("envoi du corps")?;
    flux.flush().context("vidage")?;

    // ⚠️ ON LIT LA RÉPONSE, ON NE LA JETTE PAS. Sans cette lecture, un `413`
    // ou un `400 {refus:'empreinte'}` passerait pour un succès, et l'agent
    // retéléverserait la même icône indéfiniment sans jamais savoir pourquoi.
    let mut reponse = Vec::new();
    flux.read_to_end(&mut reponse).context("lecture de la reponse")?;
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
