//! Neutralisation de l'accélération et de la sensibilité pointeur de Windows.
//!
//! `SendInput` en mode relatif traverse la courbe d'accélération et le
//! réglage de vitesse de la session — sans neutralisation, la visée est
//! déformée et non linéaire, ce qui est inacceptable en jeu. C'est le
//! « piège majeur » du cadrage.
//!
//! Le réglage est appliqué SANS `SPIF_UPDATEINIFILE` : il vaut pour la
//! session courante et disparaît à la déconnexion. L'agent ne laisse donc
//! aucune trace persistante sur la VM, et le réglage ne peut pas être oublié
//! lors d'un réapprovisionnement — il est réappliqué à chaque démarrage.
//!
//! Aucun effet de bord sur la souris absolue, qui ignore déjà ces réglages.

#[cfg(windows)]
mod win {
    use anyhow::{Context, Result};
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETMOUSE, SPI_GETMOUSESPEED, SPI_SETMOUSE, SPI_SETMOUSESPEED,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    /// Cran de vitesse 1:1. L'échelle va de 1 à 20 ; 10 est le seul cran qui
    /// ne multiplie ni ne divise le déplacement.
    const VITESSE_1_POUR_1: usize = 10;

    /// Seuils d'accélération. Les deux premiers sont les paliers, le
    /// troisième active ou non l'accélération : tout à zéro la désactive.
    const SANS_ACCELERATION: [u32; 3] = [0, 0, 0];

    pub fn neutraliser() -> Result<String> {
        // Vérifié contre la définition générée par windows-rs 0.62 (le brief
        // n'était qu'une forme plausible) : `SystemParametersInfoW` a bien la
        // signature `(uiaction, uiparam: u32, pvparam: Option<*mut c_void>,
        // fwinini) -> windows_core::Result<()>` pour les DEUX actions, y
        // compris SPI_SETMOUSESPEED — le troisième paramètre change de SENS
        // (pointeur vers une donnée, ou donnée elle-même déguisée en
        // pointeur) selon l'action, mais pas de TYPE. Aucune correction de
        // signature n'a donc été nécessaire ici, contrairement à
        // `ClientToScreen` dans `input.rs`.
        let mut params = SANS_ACCELERATION;
        unsafe {
            SystemParametersInfoW(
                SPI_SETMOUSE,
                0,
                Some(params.as_mut_ptr().cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .context("SPI_SETMOUSE")?;

        // SPI_SETMOUSESPEED est l'asymétrie annoncée : contrairement à
        // SPI_SETMOUSE, la valeur n'est pas pointée par `pvparam`, elle EST
        // `pvparam`, casté en pointeur (c'est ainsi que Win32 documente
        // cette action précise — un entier transporté par un paramètre
        // pointeur, pas une erreur du brief).
        unsafe {
            SystemParametersInfoW(
                SPI_SETMOUSESPEED,
                0,
                Some(VITESSE_1_POUR_1 as *mut core::ffi::c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .context("SPI_SETMOUSESPEED")?;

        // Relecture : un appel qui réussit ne garantit pas que la valeur a
        // été prise, notamment sous stratégie de groupe.
        let mut relu = [0u32; 3];
        unsafe {
            SystemParametersInfoW(
                SPI_GETMOUSE,
                0,
                Some(relu.as_mut_ptr().cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .context("SPI_GETMOUSE")?;

        let mut vitesse: u32 = 0;
        unsafe {
            SystemParametersInfoW(
                SPI_GETMOUSESPEED,
                0,
                Some((&mut vitesse as *mut u32).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .context("SPI_GETMOUSESPEED")?;

        Ok(format!(
            "seuils relus = {relu:?}, vitesse relue = {vitesse} (attendu : [0, 0, 0] et {VITESSE_1_POUR_1})"
        ))
    }
}

#[cfg(windows)]
pub use win::neutraliser;
