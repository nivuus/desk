//! L'horloge commune au capteur et à ses enfants.
//!
//! `clock_origin` est un `std::time::Instant`, partagé côté enfant entre la
//! vidéo et l'audio : c'est cette origine commune qui rend les deux lignes de
//! temps comparables, donc la synchro A/V exacte. Un `Instant` n'a aucun sens
//! dans un autre processus — mais `QueryPerformanceCounter` est monotone et
//! **commun à toute la machine**. L'enfant envoie donc son origine en tics QPC,
//! et le capteur reconstruit l'`Instant` équivalent chez lui.
//!
//! La conversion est PURE et hors `cfg` : c'est elle qui peut être fausse, pas
//! l'appel système.

use std::time::{Duration, Instant};

/// Reconstruit l'origine d'horloge de l'enfant dans le référentiel `Instant`
/// du capteur.
///
/// Retombe sur `maintenant` dans les deux cas dégénérés — origine postérieure
/// à la lecture courante, ou fréquence nulle — plutôt que de paniquer : une
/// origine fausse décale la synchro A/V, une panique tue la fenêtre.
pub fn origine_depuis_qpc(
    origine_qpc: i64,
    qpc_maintenant: i64,
    frequence: i64,
    maintenant: Instant,
) -> Instant {
    if frequence <= 0 || qpc_maintenant <= origine_qpc {
        return maintenant;
    }
    let tics = (qpc_maintenant - origine_qpc) as u128;
    let nanos = tics * 1_000_000_000u128 / frequence as u128;
    maintenant
        .checked_sub(Duration::from_nanos(nanos.min(u64::MAX as u128) as u64))
        .unwrap_or(maintenant)
}

#[cfg(windows)]
mod systeme {
    use anyhow::{Context, Result};
    use windows::Win32::System::Performance::{
        QueryPerformanceCounter, QueryPerformanceFrequency,
    };

    pub fn lire_qpc() -> Result<i64> {
        let mut valeur = 0i64;
        unsafe { QueryPerformanceCounter(&mut valeur) }.context("QueryPerformanceCounter")?;
        Ok(valeur)
    }

    pub fn frequence_qpc() -> Result<i64> {
        let mut valeur = 0i64;
        unsafe { QueryPerformanceFrequency(&mut valeur) }.context("QueryPerformanceFrequency")?;
        Ok(valeur)
    }
}

#[cfg(windows)]
pub use systeme::{frequence_qpc, lire_qpc};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn une_origine_anterieure_se_reconstruit_en_arriere() {
        let maintenant = Instant::now();
        // 10 MHz, et 25 millions de tics écoulés = 2,5 s.
        let reconstruite = origine_depuis_qpc(1_000_000, 26_000_000, 10_000_000, maintenant);
        let ecart = maintenant.duration_since(reconstruite);
        assert!(
            ecart.abs_diff(Duration::from_millis(2500)) < Duration::from_millis(1),
            "écart reconstruit : {ecart:?}"
        );
    }

    #[test]
    fn une_origine_egale_a_maintenant_ne_recule_pas() {
        let maintenant = Instant::now();
        assert_eq!(origine_depuis_qpc(42, 42, 10_000_000, maintenant), maintenant);
    }

    /// Une origine POSTÉRIEURE ne peut pas exister, mais une horloge lue de
    /// travers la produirait : on retombe alors sur `maintenant` plutôt que de
    /// paniquer en soustrayant au-delà de l'origine de l'`Instant`.
    #[test]
    fn une_origine_posterieure_retombe_sur_maintenant() {
        let maintenant = Instant::now();
        assert_eq!(origine_depuis_qpc(100, 50, 10_000_000, maintenant), maintenant);
    }

    #[test]
    fn une_frequence_nulle_retombe_sur_maintenant() {
        let maintenant = Instant::now();
        assert_eq!(origine_depuis_qpc(1, 2, 0, maintenant), maintenant);
    }
}
