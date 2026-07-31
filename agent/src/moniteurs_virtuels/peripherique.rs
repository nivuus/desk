//! Résolution du chemin du périphérique SudoVDA, par SetupAPI.
//!
//! Séparé de `moniteurs.rs` — qui est le client une fois le périphérique
//! ouvert — parce que c'est une autre affaire : celle de le TROUVER. Le patron
//! d'énumération ci-dessous est celui, verbeux et plein de pièges, qu'impose
//! SetupAPI ; il n'a rien à voir avec le dialogue IOCTL qui suit.

use anyhow::{Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};

use crate::moniteurs_virtuels::sudovda::INTERFACE_PILOTE;

/// Libère la liste d'informations de périphériques sur TOUS les chemins, y
/// compris les sorties en erreur — SetupAPI ne pardonne pas les fuites de
/// `HDEVINFO`, et il y a quatre `?` entre son ouverture et sa fermeture.
struct ListeDePeripheriques(HDEVINFO);

impl Drop for ListeDePeripheriques {
    fn drop(&mut self) {
        let _ = unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

/// Résout le chemin `\\?\…` du périphérique qui expose `INTERFACE_PILOTE`.
pub(super) fn chemin_du_peripherique() -> Result<Vec<u16>> {
    let liste = ListeDePeripheriques(
        unsafe {
            SetupDiGetClassDevsW(
                Some(&INTERFACE_PILOTE),
                PCWSTR::null(),
                None,
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
        }
        .context("énumération des périphériques exposant l'interface SudoVDA")?,
    );

    // Index 0 : le pilote n'expose qu'une instance de cette interface (un seul
    // device node `ROOT\DISPLAY\0003`). Si un jour il y en avait plusieurs, ce
    // serait un fait à relever avant de choisir — pas à trancher en silence.
    let mut interface = SP_DEVICE_INTERFACE_DATA {
        cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        ..Default::default()
    };
    unsafe { SetupDiEnumDeviceInterfaces(liste.0, None, &INTERFACE_PILOTE, 0, &mut interface) }
        .context(
            "aucun périphérique ne présente l'interface SudoVDA — pilote absent, \
             désactivé, ou device node non créé",
        )?;

    // Patron imposé par SetupAPI : un premier appel pour la taille (qui échoue
    // toujours en `ERROR_INSUFFICIENT_BUFFER`, d'où l'erreur ignorée), un
    // second pour le contenu.
    let mut requis = 0u32;
    let _ = unsafe {
        SetupDiGetDeviceInterfaceDetailW(liste.0, &interface, None, 0, Some(&mut requis), None)
    };
    let entete = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
    anyhow::ensure!(
        requis as usize > entete,
        "taille de détail d'interface aberrante ({requis} octets)"
    );

    // Tampon en `u32` et non en `u8` : `SP_DEVICE_INTERFACE_DETAIL_DATA_W`
    // s'aligne sur 4, et `Vec<u8>` ne garantit que 1. Le `cbSize` à écrire est
    // celui de l'en-tête seul (8 sur x64), jamais celui du tampon — c'est la
    // convention de SetupAPI, contre-intuitive et source classique de
    // `ERROR_INVALID_USER_BUFFER`.
    let mut tampon = vec![0u32; requis.div_ceil(4) as usize];
    let detail = tampon.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
    unsafe { (*detail).cbSize = entete as u32 };
    unsafe {
        SetupDiGetDeviceInterfaceDetailW(liste.0, &interface, Some(detail), requis, None, None)
    }
    .context("lecture du chemin du périphérique SudoVDA")?;

    // `DevicePath` est déclaré `[u16; 1]` mais se prolonge jusqu'au NUL au-delà
    // de la fin nominale de la structure : c'est un tableau de longueur
    // variable à la mode C, il faut le lire à la main.
    //
    // Le nombre d'unités lisibles se compte depuis l'OFFSET de `DevicePath`
    // (4 octets, juste après `cbSize`) et NON depuis `entete` (8 octets, qui
    // inclut le remplissage d'alignement de fin de structure). L'écart est
    // d'exactement deux octets, soit une unité UTF-16 : partir d'`entete`
    // amputait le chemin de son terminateur nul et faisait échouer la
    // résolution — constaté à la première exécution réelle de la sonde.
    let offset_chemin = std::mem::offset_of!(SP_DEVICE_INTERFACE_DETAIL_DATA_W, DevicePath);
    let debut = unsafe { (*detail).DevicePath.as_ptr() };
    let maximum = (requis as usize - offset_chemin) / 2;
    let mut chemin = Vec::with_capacity(maximum);
    for decalage in 0..maximum {
        let unite = unsafe { *debut.add(decalage) };
        chemin.push(unite);
        if unite == 0 {
            return Ok(chemin);
        }
    }
    anyhow::bail!("chemin de périphérique SudoVDA sans terminateur nul");
}
