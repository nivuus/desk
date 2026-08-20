//! Découverte et lancement des applications de la VM.
//!
//! ⚠️ CE MODULE EST DÉCLARÉ SANS `cfg` dans `main.rs`, et ce sont ses enfants
//! Windows qui portent le leur. C'est ce qui fait exister `apps::raccourci` et
//! `apps::reconciliation` sur l'hôte Linux, où leurs tests courent — la
//! « Convention de module enfant » de `CLAUDE.md` n'est donc pas mobilisée :
//! aucun module ne franchit ici de frontière `#[cfg(windows)]`.

pub mod raccourci;
pub mod reconciliation;
pub mod sha256;
