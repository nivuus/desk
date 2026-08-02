//! Le capteur : un seul processus qui tient les N duplications DXGI et les N
//! encodeurs, et distribue le média aux enfants par tube nommé.
//!
//! Ce fichier reste mince à dessein — il assemble, il ne décide pas. Même
//! découpage que `superviseur.rs` : la logique pure (protocole, source
//! distante, reprise) est hors `cfg` et se teste sur l'hôte ; ce qui touche
//! DXGI et les tubes est gaté.

pub mod distante;
pub mod protocole;
pub mod reprise;
