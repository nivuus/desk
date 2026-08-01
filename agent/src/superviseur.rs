//! Le superviseur : il détecte les fenêtres, leur donne une sortie virtuelle,
//! lance un processus enfant par fenêtre et parle à la page-shell.
//!
//! Ce fichier reste mince à dessein — il assemble, il ne décide pas. Les
//! décisions vivent dans `fenetres` (quelle fenêtre mérite d'exister côté
//! navigateur) et `table` (où en est chacune), tous deux en logique pure et
//! testés sur l'hôte.

pub mod fenetres;
#[cfg(windows)]
pub mod hook;
pub mod table;
