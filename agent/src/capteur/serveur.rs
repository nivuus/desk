//! Le serveur de tube nommé du capteur.
//!
//! Chaque enfant s'y connecte et se décrit lui-même dans sa première trame :
//! il n'y a donc AUCUN canal superviseur → capteur, et aucune table d'état
//! partagée entre trois processus. La fermeture du tube EST le signal de fin
//! de vie d'une fenêtre.

#![cfg(windows)]

use std::io::{BufReader, BufWriter};
use std::sync::mpsc::{channel, Sender};

use anyhow::{bail, Context, Result};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use crate::capteur::fenetre::servir_une_fenetre;
use crate::capteur::protocole::{lire_trame, Trame, VersCapteur, NOM_TUBE};

/// Tampon de tube, dans les deux sens. Généreux à dessein : c'est lui qui
/// absorbe les à-coups avant que la contre-pression ne remonte jusqu'au fil
/// de capture.
const TAMPON: u32 = 1024 * 1024;

pub fn servir() -> Result<()> {
    loop {
        // Une instance NEUVE par client. `PIPE_UNLIMITED_INSTANCES` autorise
        // autant d'instances simultanées que de fenêtres.
        let tube = creer_instance().context("création d'une instance de tube")?;

        // Bloque jusqu'à ce qu'un enfant se connecte.
        unsafe { ConnectNamedPipe(tube, None) }.context("attente d'un enfant")?;

        match accueillir(tube) {
            Ok(()) => {}
            // Une attache ratée ne fait PAS tomber le serveur : les autres
            // fenêtres continuent. C'est tout l'intérêt d'avoir un capteur
            // qui survit à ses fenêtres.
            Err(erreur) => tracing::warn!(%erreur, "attache d'un enfant refusée"),
        }
    }
}

fn creer_instance() -> Result<HANDLE> {
    let nom: Vec<u16> = NOM_TUBE.encode_utf16().chain(std::iter::once(0)).collect();
    let tube = unsafe {
        CreateNamedPipeW(
            windows::core::PCWSTR(nom.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            TAMPON,
            TAMPON,
            0,
            None,
        )
    };
    if tube.is_invalid() {
        bail!("CreateNamedPipeW a rendu un handle invalide");
    }
    Ok(tube)
}

/// Lit la première trame — qui DOIT être une attache — puis détache les deux
/// fils de cette fenêtre.
fn accueillir(tube: HANDLE) -> Result<()> {
    // `std::fs::File` depuis le handle : il donne `Read`/`Write` sans écrire
    // d'enveloppe, et sa fermeture ferme le tube.
    use std::os::windows::io::FromRawHandle;
    let fichier = unsafe { std::fs::File::from_raw_handle(tube.0 as *mut _) };
    let mut lecteur = BufReader::new(fichier.try_clone().context("clone du tube en lecture")?);
    let ecrivain = BufWriter::new(fichier);

    let attache = match lire_trame(&mut lecteur).context("première trame de l'enfant")? {
        Trame::Json(octets) => serde_json::from_slice::<VersCapteur>(&octets)
            .context("première trame illisible")?,
        Trame::Image(_) => bail!("le premier message d'un enfant ne peut pas être une image"),
    };
    if !matches!(attache, VersCapteur::Attache { .. }) {
        bail!("le premier message d'un enfant doit être une attache, reçu {attache:?}");
    }

    let (tx, rx) = channel::<VersCapteur>();

    // Fil LECTEUR : lit le tube en bloquant, dépose les commandes.
    std::thread::spawn(move || lire_les_commandes(lecteur, tx));

    // Fil de SERVICE : tient le `WindowsSource`. DÉTACHÉ, jamais joint — la
    // boucle d'acceptation ne doit pas pouvoir être bloquée par un démontage
    // d'encodeur (`Drop for H264Encoder` peut geler).
    std::thread::spawn(move || {
        if let Err(erreur) = servir_une_fenetre(rx, ecrivain, attache) {
            tracing::warn!(%erreur, "fil de fenêtre terminé sur erreur");
        }
    });
    Ok(())
}

fn lire_les_commandes<R: std::io::Read>(mut lecteur: R, tx: Sender<VersCapteur>) {
    loop {
        match lire_trame(&mut lecteur) {
            Ok(Trame::Json(octets)) => match serde_json::from_slice::<VersCapteur>(&octets) {
                Ok(message) => {
                    if tx.send(message).is_err() {
                        return; // le fil de service est parti
                    }
                }
                Err(erreur) => {
                    tracing::warn!(%erreur, "commande illisible, canal abandonné");
                    return;
                }
            },
            Ok(Trame::Image(_)) => {
                tracing::warn!("un enfant a envoyé une image, canal abandonné");
                return;
            }
            // Fin de tube : l'enfant est parti. Laisser tomber `tx` signale
            // `Disconnected` au fil de service, qui démonte sa source.
            Err(_) => return,
        }
    }
}
