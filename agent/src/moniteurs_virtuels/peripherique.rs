//! Resolution of the SudoVDA device path, through SetupAPI.
//!
//! Separated from `moniteurs.rs` — which is the client once the device
//! is opened — because it is another matter: that of FINDING it. The
//! enumeration pattern below is the verbose, trap-laden one
//! SetupAPI imposes; it has nothing to do with the IOCTL dialogue that follows.

use anyhow::{Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};

use crate::moniteurs_virtuels::sudovda::INTERFACE_PILOTE;

/// Frees the device information list on ALL paths,
/// including error exits — SetupAPI does not forgive leaks of
/// `HDEVINFO`, and there are four `?` between its opening and its closing.
struct DeviceInfoList(HDEVINFO);

impl Drop for DeviceInfoList {
    fn drop(&mut self) {
        let _ = unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

/// Resolves the `\\?\…` path of the device that exposes `INTERFACE_PILOTE`.
pub(super) fn chemin_du_peripherique() -> Result<Vec<u16>> {
    let list = DeviceInfoList(
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

    // Index 0: the driver only exposes one instance of this interface (a single
    // device node `ROOT\DISPLAY\0003`). If one day there were several, it
    // would be a fact to note before choosing — not to settle silently.
    let mut interface = SP_DEVICE_INTERFACE_DATA {
        cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        ..Default::default()
    };
    unsafe { SetupDiEnumDeviceInterfaces(list.0, None, &INTERFACE_PILOTE, 0, &mut interface) }
        .context(
            "aucun périphérique ne présente l'interface SudoVDA — pilote absent, \
             désactivé, ou device node non créé",
        )?;

    // Pattern imposed by SetupAPI: a first call for the size (which always
    // fails with `ERROR_INSUFFICIENT_BUFFER`, hence the ignored error), a
    // second for the content.
    let mut requis = 0u32;
    let _ = unsafe {
        SetupDiGetDeviceInterfaceDetailW(list.0, &interface, None, 0, Some(&mut requis), None)
    };
    let entete = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
    anyhow::ensure!(
        requis as usize > entete,
        "taille de détail d'interface aberrante ({requis} octets)"
    );

    // Buffer in `u32` and not in `u8`: `SP_DEVICE_INTERFACE_DETAIL_DATA_W`
    // aligns on 4, and `Vec<u8>` only guarantees 1. The `cbSize` to write is
    // that of the header alone (8 on x64), never that of the buffer — it is
    // SetupAPI's convention, counter-intuitive and a classic source of
    // `ERROR_INVALID_USER_BUFFER`.
    let mut tampon = vec![0u32; requis.div_ceil(4) as usize];
    let detail = tampon.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
    unsafe { (*detail).cbSize = entete as u32 };
    unsafe {
        SetupDiGetDeviceInterfaceDetailW(list.0, &interface, Some(detail), requis, None, None)
    }
    .context("lecture du chemin du périphérique SudoVDA")?;

    // `DevicePath` is declared `[u16; 1]` but extends up to the NUL beyond
    // the nominal end of the structure: it is a C-style variable-length
    // array, it must be read by hand.
    //
    // The number of readable units is counted from the OFFSET of `DevicePath`
    // (4 bytes, right after `cbSize`) and NOT from `entete` (8 bytes, which
    // includes the structure's trailing alignment padding). The gap is
    // exactly two bytes, that is one UTF-16 unit: starting from `entete`
    // cut the path's null terminator off and made the
    // resolution fail — found at the probe's first real run.
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
