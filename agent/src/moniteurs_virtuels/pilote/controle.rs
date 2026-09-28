//! The three driver IOCTLs that neither add nor remove anything: protocol
//! version, watchdog ping, reading of its countdown.
//!
//! **CHILD module of `pilote`**, and not sibling: it is what gives it access to
//! `PiloteParIoctl::commander`, which stays private. Extracted from `pilote.rs` at the final
//! branch review because fix I1 added number recycling to it
//! and brought the file to 507 lines, above the project's ceiling of 500
//! (see `CLAUDE.md`): the addition comes with its extraction.
//!
//! These three form a natural block — none touches the state of the
//! outputs, none has a side effect on the topology, and all three only
//! serve to query or maintain the driver. No value changed
//! in the move.

use anyhow::Result;

use super::PiloteParIoctl;
use crate::moniteurs_virtuels::sudovda::{
    Veille, VersionProtocole, IOCTL_LIRE_VEILLE, IOCTL_LIRE_VERSION_PROTOCOLE, IOCTL_PINGUER,
};

impl PiloteParIoctl {
    /// Protocol version announced by the installed driver. Without side
    /// effect — the simplest buffer of the six.
    pub(crate) fn version_protocole(&self) -> Result<(VersionProtocole, u32)> {
        let mut version = VersionProtocole::default();
        let rendus = self.commander(
            IOCTL_LIRE_VERSION_PROTOCOLE,
            None,
            Some((
                &mut version as *mut _ as *mut _,
                std::mem::size_of::<VersionProtocole>() as u32,
            )),
            "reading the driver's protocol version",
        )?;
        Ok((version, rendus))
    }

    /// Rearms the driver's watchdog for THIS handle.
    ///
    /// Neither input nor output: it is the only one of the six IOCTLs whose two buffers
    /// are empty, hence the only one where no assumed layout can
    /// be wrong.
    ///
    /// **Why this beat is not launched here, in an internal thread.** The
    /// driver most likely associates its watchdog with the *file object*
    /// opened by `CreateFile` — that is what the upstream client does, pinging
    /// on the very handle it used to add its outputs. Pinging
    /// from a second handle would therefore save nothing. An internal thread would
    /// then have to share THIS handle, which would force making it `Send`; yet the
    /// only two callers of this module are sequential by construction and
    /// only need to punctuate their waits. We expose the beat,
    /// the caller keeps the cadence.
    pub(crate) fn pinguer(&self) -> Result<()> {
        self.commander(IOCTL_PINGUER, None, None, "pinging the driver's watchdog")?;
        Ok(())
    }

    /// Delay and countdown of the driver's watchdog. Without side effect.
    pub(crate) fn veille(&self) -> Result<(Veille, u32)> {
        let mut veille = Veille::default();
        let rendus = self.commander(
            IOCTL_LIRE_VEILLE,
            None,
            Some((
                &mut veille as *mut _ as *mut _,
                std::mem::size_of::<Veille>() as u32,
            )),
            "reading the driver's watchdog",
        )?;
        Ok((veille, rendus))
    }
}
