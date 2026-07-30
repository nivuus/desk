//! Contrôleur de congestion : décide du débit et de la résolution d'encodage
//! à partir de ce que le pair rapporte.
//!
//! Aucune dépendance à Windows, à str0m ni au socket — c'est ce qui rend
//! toute la politique testable sur Linux, sans VM et sans réseau. Même
//! raison d'être que `geometry.rs`, `rebuild.rs` et `clock.rs`.
//!
//! Découpé en quatre sous-modules : `echelle` (les résolutions disponibles),
//! `hysteresis` (les délais avant changement), `controleur` (l'asservissement
//! continu) et `reconfiguration` (le changement de taille de source).

mod controleur;
mod echelle;
mod hysteresis;
mod reconfiguration;

pub use controleur::Controleur;

use std::time::{Duration, Instant};

/// Part de l'estimation qu'on s'autorise à consommer.
///
/// Les 10 % restants laissent la place aux retransmissions RTX et aux paquets
/// de sondage que le sous-système BWE émet pour tester à la hausse. Viser
/// 100 % de l'estimation, c'est garantir de la dépasser.
const MARGE: f32 = 0.9;

/// Écart relatif en dessous duquel on ne reconfigure pas le débit. Sans lui,
/// une estimation qui frémit ferait écrire l'encodeur à chaque seconde.
const ECART_MINIMAL_DEBIT: f32 = 0.10;

/// Plafond du pourcentage de perte déclaré à Opus. Au-delà, la redondance
/// LBRR coûte plus de débit qu'elle n'en sauve.
const PERTE_MAX_OPUS: i32 = 25;

/// Réglages figés d'une session.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Plafond de débit vidéo, en bits par seconde (variable `BITRATE`).
    /// Sert aussi de valeur de repli quand aucune estimation n'arrive.
    pub plafond_bps: u32,
    /// Budget réservé à la piste audio, retiré de l'estimation.
    pub audio_bps: u32,
    /// Taille de la source capturée, sommet de l'échelle.
    pub source: (u32, u32),
    pub fps: u32,
}

/// Ce que le transport observe, une fois par seconde.
#[derive(Debug, Clone, Copy)]
pub struct Observation {
    /// Estimation de bande passante sortante. `None` tant qu'aucune n'est
    /// arrivée — cas normal au démarrage, cas permanent si TWCC n'est pas
    /// négocié.
    pub estimate_bps: Option<u32>,
    /// Non consommé par la décision : journalisé par `transport.rs` pour que
    /// la recette dispose du RTT vu par l'agent, à confronter à celui que le
    /// navigateur rapporte.
    pub rtt: Option<Duration>,
    /// Fraction de paquets perdus, entre 0 et 1.
    pub loss: Option<f32>,
    pub at: Instant,
}

/// État du lien tel qu'on l'annonce à l'utilisateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qualite {
    /// Barreau le plus haut.
    Bonne,
    /// Résolution réduite : l'utilisateur doit savoir pourquoi l'image a molli.
    Degradee,
    /// Plancher atteint. On ne dégrade plus — on le dit.
    Insuffisante,
}

/// Le contrôleur reçoit-il de quoi s'asservir ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adaptation {
    Active,
    /// Aucune estimation n'est jamais arrivée. Le débit reste au plafond, et
    /// ce fait doit être annoncé — un silence ressemblerait à « tout va bien ».
    Indisponible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub video_bitrate_bps: u32,
    pub encode_size: (u32, u32),
    pub opus_loss_perc: i32,
    pub qualite: Qualite,
    pub adaptation: Adaptation,
}
