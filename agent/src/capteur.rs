//! Le capteur : un seul processus qui tient les N duplications DXGI et les N
//! encodeurs, et distribue le média aux enfants par tube nommé.
//!
//! Ce fichier reste mince à dessein — il assemble, il ne décide pas. Même
//! découpage que `superviseur.rs` : la logique pure (protocole, source
//! distante, reprise) est hors `cfg` et se teste sur l'hôte ; ce qui touche
//! DXGI et les tubes est gaté.

pub mod distante;
#[cfg(windows)]
pub mod fenetre;
pub mod horloge;
pub mod protocole;
pub mod reprise;
pub mod vivier;
#[cfg(windows)]
pub mod serveur;
#[cfg(windows)]
pub mod tube;

/// Point d'entrée du mode capteur.
#[cfg(windows)]
pub fn executer() -> anyhow::Result<()> {
    tracing::info!(tube = protocole::NOM_TUBE, "capteur démarré");
    serveur::servir()
}

#[cfg(not(windows))]
pub fn executer() -> anyhow::Result<()> {
    anyhow::bail!("le mode capteur n'existe que sur Windows")
}
