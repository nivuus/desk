//! The sensor: a single process that holds the N DXGI duplications and the N
//! encoders, and distributes the media to the children through a named pipe.
//!
//! This file stays thin on purpose — it assembles, it does not decide. Same
//! split as `superviseur.rs`: the pure logic (protocol, remote
//! source, resumption) is outside `cfg` and is tested on the host; what touches
//! DXGI and the pipes is gated.

pub mod audio;
pub mod distante;
#[cfg(windows)]
pub mod fenetre;
pub mod horloge;
pub mod plein_ecran;
pub mod pont_media;
pub mod protocole;
pub mod repartiteur;
pub mod reprise;
#[cfg(windows)]
pub mod serveur;
pub mod sommeil;
#[cfg(windows)]
pub mod tube;
pub mod vivier;

/// Entry point of sensor mode.
#[cfg(windows)]
pub fn executer() -> anyhow::Result<()> {
    tracing::info!(tube = protocole::NOM_TUBE, "sensor started");
    serveur::servir()
}

#[cfg(not(windows))]
pub fn executer() -> anyhow::Result<()> {
    anyhow::bail!("the sensor mode only exists on Windows")
}
