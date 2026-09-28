//! Measurement ①: the virtual display driver, commanded from OUR code.
//!
//! Spec §2 decides: we do not relaunch Apollo to obtain this measurement.
//! First because it would depend on a second paired client device the
//! workstation's owner does not have — that is what blocked the previous probe.
//! Then because the product will have to do without Apollo anyway.
//!
//! Control channel surveyed at task 3:
//! `docs/superpowers/plans/journaux-mesures-prealables/canal-de-controle.md`.
//! Its verdict is **form B**: the SudoVDA driver only exports
//! `FxDriverEntryUm` (the generic UMDF entry point), hence no callable
//! control function — the hypothesis "load the DLL and call
//! `AddVirtualDisplay`" is dead, and was not implemented. We talk to the
//! driver as its real client (`sunshine.exe`) does: enumerating its
//! device interface through SetupAPI, `CreateFile`, `DeviceIoControl`.
//!
//! **What is established and what is not.** The interface GUID and two
//! of the six IOCTL codes were found byte for byte in the
//! `SudoVDA.dll` installed on THIS VM. The four other codes and the
//! layout of ALL the structures below come from an upstream header
//! (`Apollo/third-party/sudovda`) whose last known modification precedes
//! by eleven months the installed driver (`DriverVer 07/14/2025, 1.10.9.289`). Nothing
//! excludes that a field was added or reordered since. It is the purpose
//! of the neighbouring module `contrat.rs`: testing the contract on the two
//! simplest buffers BEFORE the next task commits
//! `IOCTL_ADD_VIRTUAL_DISPLAY` and its 56 input bytes. Without it, a
//! wrong contract and a failed measurement would be indistinguishable.
//!
//! **The watchdog is COMMANDABLE here, not armed.** The driver exposes
//! `IOCTL_DRIVER_PING` and `IOCTL_GET_WATCHDOG`: a client that stops
//! pinging sees its outputs removed. This module makes both available
//! (`pinguer`, `veille`) but launches no cadence by itself — it is
//! the caller that holds live outputs, so it is up to it to beat. See
//! `montee.rs`, which pings and which measured what this watchdog really
//! does.

use std::sync::Mutex;

use anyhow::{Context, Result};
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;

use super::{Adaptateur, IdSortie, PiloteAffichageVirtuel};
use crate::moniteurs_virtuels::guid::{guid_pour, numero_de};
use crate::moniteurs_virtuels::peripherique::chemin_du_peripherique;
use crate::moniteurs_virtuels::sudovda::{
    en_champ_14, DemandeAjout, DemandeRetrait, SortieAjoutee, IOCTL_ADD_OUTPUT,
    IOCTL_RETIRER_SORTIE,
};

/// The three side-effect-free IOCTLs (version, ping, watchdog), extracted to
/// fit under the 500-line ceiling — see its header comment. CHILD
/// module: it is what leaves it access to `commander`, which stays private.
mod controle;

/// The state kept between two calls, extracted for the same reason as
/// `controle` just above — see its header comment. CHILD module:
/// it is what leaves its `pub(super)` fields readable here, and nowhere
/// else.
///
/// ⚠️ The `etat` module and the `etat()` method below carry the same name
/// in two different namespaces: it is legal, and it is deliberate.
mod etat;
use etat::EtatSorties;

pub(crate) struct PiloteParIoctl {
    peripherique: HANDLE,
    /// A `Mutex` and not a `RefCell` because `create(&self, …)` must stay
    /// usable from a shared context. See `etat()` for the only
    /// subtlety it introduces.
    etat: Mutex<EtatSorties>,
}

/// Opens the virtual display driver's device.
///
/// Concrete type and not `impl Trait`: the following tasks take a
/// reference to it, which Rust coerces to `&dyn PiloteAffichageVirtuel`.
pub(crate) fn ouvrir_pilote() -> Result<PiloteParIoctl> {
    let chemin = chemin_du_peripherique()?;
    // POURQUOI PAS `FILE_FLAG_OVERLAPPED`, contrairement au client amont.
    //
    // `sunshine.exe` opens this device with `FILE_FLAG_NO_BUFFERING |
    // FILE_FLAG_OVERLAPPED | FILE_FLAG_WRITE_THROUGH`, then calls
    // `DeviceIoControl` with a null `lpOverlapped`. The Win32 documentation is
    // nevertheless explicit: on an overlapped handle, this parameter cannot
    // be `NULL` — the call may then return before the output buffer
    // is filled, and reading that buffer is a race. That it "works"
    // for the upstream client proves nothing: it would be true of any driver that
    // completes its requests synchronously, until the day it no longer completes
    // one. We therefore open in synchronous mode (neither `OVERLAPPED`, nor the two other
    // flags, which only concern the file cache and make no sense
    // for `METHOD_BUFFERED` IOCTLs): it is the only mode where a
    // null `lpOverlapped` is correct, and all our calls are synchronous.
    //
    // `GENERIC_READ | GENERIC_WRITE` and not `FILE_GENERIC_*`: the security
    // descriptor set by the INF only grants the world `GRGW`
    // (`(A;;GRGW;;;WD)`) — it is exactly these generic rights that
    // must be requested.
    let peripherique = unsafe {
        CreateFileW(
            PCWSTR(chemin.as_ptr()),
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .context("opening the virtual display driver device (SudoVDA)")?;
    Ok(PiloteParIoctl {
        peripherique,
        etat: Mutex::new(EtatSorties::default()),
    })
}

impl PiloteParIoctl {
    /// Access to the state, **without panicking on a poisoned lock**.
    ///
    /// `Sorties::drop` calls `detruire` during the unwinding of a panic
    /// and catches the `Err`s — but not the panics. If the panic occurred
    /// while `create` held this lock, the latter is poisoned:
    /// an `.expect(…)` would panic here, in a `Drop`, which cuts the
    /// process short (`abort`) and would leave the remaining outputs undestroyed.
    /// It is precisely the scenario this module must cover, not
    /// aggravate. `into_inner` returns the table as is: at worst, an
    /// insertion interrupted by the panic is missing from it.
    fn etat(&self) -> std::sync::MutexGuard<'_, EtatSorties> {
        self.etat
            .lock()
            .unwrap_or_else(|empoisonne| empoisonne.into_inner())
    }

    /// Erases any trace of this GUID: the output no longer exists, neither a due
    /// removal nor a pairing must survive it.
    ///
    /// Indexing by GUID assumes their UNIQUENESS, and it is the allocator of
    /// `numeros` that carries it: two live outputs cannot share
    /// a number, since a number is only returned after a SUCCESSFUL removal.
    ///
    /// **Only call on an output actually removed.** It is here that the
    /// number goes back to the allocator (fix I1): calling it on a failed
    /// removal would reassign the GUID of a monitor still alive.
    ///
    /// `pub(super)`: `purge::rejouer_purge_due` calls it after a successful
    /// removal, for the same reason `detruire` calls it here.
    pub(super) fn oublier(&self, guid_moniteur: GUID) {
        let mut etat = self.etat();
        etat.a_purger.retain(|connu| *connu != guid_moniteur);
        etat.apparies
            .retain(|(_, connu, _)| *connu != guid_moniteur);
        // `None` only for a GUID that does not come from our template:
        // nothing to return, and above all nothing to guess (see `guid::numero_de`).
        if let Some(numero) = numero_de(guid_moniteur) {
            etat.numeros.rendre(numero);
        }
    }

    /// Removes from the driver the output carrying this GUID.
    ///
    /// Extracted from `detruire` because `create` must be able to call it too,
    /// on its failure path — where no reliable `IdSortie` exists.
    ///
    /// `pub(super)`: it is also the door through which `purge.rs` removes
    /// outputs this process never created — a GUID regenerated by
    /// `guid_pour`, outside `apparies` and `a_purger`.
    pub(super) fn retirer_par_guid(&self, guid_moniteur: GUID, quoi: &str) -> Result<()> {
        let demande = DemandeRetrait { guid_moniteur };
        self.commander(
            IOCTL_RETIRER_SORTIE,
            Some((
                &demande as *const _ as *const _,
                std::mem::size_of::<DemandeRetrait>() as u32,
            )),
            None,
            quoi,
        )?;
        Ok(())
    }

    /// A synchronous `DeviceIoControl` call, with a check of the number
    /// of bytes returned.
    ///
    /// This check is not belt-and-braces: it is the only
    /// available signal that an output structure does have the size assumed
    /// for it. A driver that gained a field since the upstream header
    /// would return a different count, and we want to see it rather than read a
    /// partially filled buffer.
    fn commander(
        &self,
        code: u32,
        entree: Option<(*const std::ffi::c_void, u32)>,
        sortie: Option<(*mut std::ffi::c_void, u32)>,
        quoi: &str,
    ) -> Result<u32> {
        let (ptr_entree, input_size) = entree.map_or((None, 0), |(p, t)| (Some(p), t));
        let (ptr_sortie, output_size) = sortie.map_or((None, 0), |(p, t)| (Some(p), t));
        let mut rendus = 0u32;
        unsafe {
            DeviceIoControl(
                self.peripherique,
                code,
                ptr_entree,
                input_size,
                ptr_sortie,
                output_size,
                Some(&mut rendus),
                // Null, and legitimately: the handle is opened in
                // synchronous mode (see `ouvrir_pilote`).
                None,
            )
        }
        .with_context(|| format!("{quoi} (IOCTL {code:#010x})"))?;
        Ok(rendus)
    }

    /// Snapshot of the GUIDs whose previous removal failed and remains due.
    ///
    /// `pub(super)` for `purge::rejouer_purge_due`, which closes the debt
    /// left by task 5: without a reader, `a_purger` only served to
    /// log a failed removal, never to retry it.
    pub(super) fn a_purger(&self) -> Vec<GUID> {
        self.etat().a_purger.clone()
    }

    /// The adapter on which a paired output was created.
    ///
    /// `None` for an identifier this driver did not create, or whose
    /// removal failed: the entry has then left `apparies`, on purpose (see
    /// the doc of `EtatSorties`). A `None` makes the caller fall back on its
    /// fallback, never on a guess.
    ///
    /// ⚠️ **The returned pair ONLY serves to DESIGNATE a display target,
    /// never to destroy**: the driver only removes through a GUID.
    pub(crate) fn adaptateur_de(&self, id: IdSortie) -> Option<Adaptateur> {
        self.etat()
            .apparies
            .iter()
            .find(|(connu, _, _)| *connu == id)
            .map(|(_, _, adaptateur)| *adaptateur)
    }
}

impl PiloteAffichageVirtuel for PiloteParIoctl {
    fn create(&self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie> {
        // This lock is held during the add `DeviceIoControl`, a blocking kernel
        // call: without consequence as long as the scale-up in N stays
        // sequential, to be revisited if it stops being so.
        let mut etat = self.etat();
        // A refusal here is a refusal to create, and it is intended: beyond the
        // ceiling, the assigned GUID would go out of the range the
        // inter-process purge sweeps, and the output would become unrecoverable without
        // rebooting the VM (see `numeros`). Better a window
        // refused noisily — the supervisor's table already knows what to do
        // with a creation refusal — than an unremovable ghost monitor.
        let numero = etat.numeros.attribuer()?;
        let guid_moniteur = guid_pour(numero);

        let demande = DemandeAjout {
            largeur,
            hauteur,
            hertz,
            guid_moniteur,
            nom_peripherique: en_champ_14("Guacamole"),
            numero_serie: en_champ_14(&format!("mesure{numero}")),
        };
        let mut ajoutee = SortieAjoutee::default();
        let rendus = match self.commander(
            IOCTL_ADD_OUTPUT,
            Some((
                &demande as *const _ as *const _,
                std::mem::size_of::<DemandeAjout>() as u32,
            )),
            Some((
                &mut ajoutee as *mut _ as *mut _,
                std::mem::size_of::<SortieAjoutee>() as u32,
            )),
            &format!("creating a {largeur}x{hauteur}@{hertz} output"),
        ) {
            Ok(rendus) => rendus,
            Err(error) => {
                // NO output exists: the number is owed to no one and
                // goes back to the allocator. Without it, a series of driver
                // refusals — whose pool of ten is LOWER than our
                // ceiling of sixteen, hence reached first — would consume
                // numbers for nothing and would end up making any
                // creation be refused while the driver, for its part, would have room.
                etat.numeros.rendre(numero);
                return Err(error);
            }
        };

        // FROM HERE ON THE OUTPUT EXISTS. Any failure path below this line
        // must therefore undo what has just been done, or at the very least leave the
        // GUID known — otherwise the monitor outlives the process without any
        // code of the project being able to remove it.
        //
        // The GUID is kept BEFORE any check, and in `a_purger` and
        // not `apparies`: at this instant we know an output exists, but we
        // do not yet know how to DESIGNATE it — `identifiant_cible` is only worth
        // something if the output buffer has the expected size. Any
        // created output therefore first enters the list of due removals, and
        // only leaves it to be paired with a reliable identifier, or because
        // it was removed.
        etat.a_purger.push(guid_moniteur);
        drop(etat);

        // The most probable failure path of this module: `VIRTUAL_DISPLAY_ADD_OUT`
        // is precisely the structure the reconnaissance declares
        // unconfirmed. If its byte count differs, `identifiant_cible` can
        // be anything — the caller will therefore never be able to ask us
        // for this output again by its identifier, and the `Sorties` guard
        // will not record it either since we return `Err`. We therefore
        // remove it OURSELVES, while the GUID is known.
        let attendus = std::mem::size_of::<SortieAjoutee>();
        if rendus as usize != attendus {
            let retrait = self.retirer_par_guid(
                guid_moniteur,
                "removing the output created with an unreadable output buffer",
            );
            match retrait {
                Ok(()) => {
                    self.oublier(guid_moniteur);
                    anyhow::bail!(
                        "the driver returned {rendus} bytes for a created output, \
                         {attendus} expected — the assumed layout of \
                         VIRTUAL_DISPLAY_ADD_OUT is wrong; the output was removed"
                    );
                }
                Err(error) => {
                    // The GUID stays in `a_purger` on purpose: it is the only
                    // trace of what must be removed. It does NOT enter
                    // `apparies` — a dubious identifier appearing there
                    // could pair a later `detruire` and make it
                    // remove the wrong output.
                    tracing::error!(
                        guid = ?guid_moniteur,
                        %error,
                        "virtual output NOT removed after an unreadable buffer — \
                         manual purge required"
                    );
                    anyhow::bail!(
                        "the driver returned {rendus} bytes for a created output, \
                         {attendus} expected — the assumed layout of \
                         VIRTUAL_DISPLAY_ADD_OUT is wrong, AND its removal \
                         failed: {error}"
                    );
                }
            }
        }

        // The byte count is right: the identifier is reliable. The output
        // goes from "removal due" to "paired".
        let id = ajoutee.identifiant_cible;
        // 🔴 THE THREE NUMBERS ARE KEPT, NO LONGER ONLY THE THIRD.
        // The adapter was only logged twelve lines below, then
        // thrown away — and without it, `identifiant_cible` designates nothing: a
        // target identifier is only unique PER adapter. It is this
        // pair that `config_affichage` exchanges for a GDI name.
        let adaptateur: Adaptateur = (ajoutee.adaptateur_bas, ajoutee.adaptateur_haut);
        let mut etat = self.etat();
        etat.a_purger.retain(|connu| *connu != guid_moniteur);
        etat.apparies.push((id, guid_moniteur, adaptateur));
        drop(etat);

        tracing::info!(
            id,
            adaptateur_bas = ajoutee.adaptateur_bas,
            adaptateur_haut = ajoutee.adaptateur_haut,
            guid = ?guid_moniteur,
            largeur,
            hauteur,
            hertz,
            "virtual output created"
        );
        Ok(id)
    }

    fn detruire(&self, id: IdSortie) -> Result<()> {
        let etat = self.etat();
        // Refuse rather than guess: the driver removes through a GUID, and making up
        // a GUID by guesswork would at best destroy nothing, at worst another
        // client's output (Apollo assigns some too).
        let rang = etat
            .apparies
            .iter()
            .position(|(connu, _, _)| *connu == id)
            .with_context(|| format!("output {id} unknown to this driver — nothing to destroy"))?;
        let (_, guid_moniteur, _) = etat.apparies[rang];
        drop(etat);

        // The pairing is only removed AFTER the call, never before: on
        // failure, the GUID is the only handle the project has on this monitor,
        // and forgetting it would make it unrecoverable.
        match self.retirer_par_guid(guid_moniteur, &format!("destroying output {id}")) {
            Ok(()) => {
                self.oublier(guid_moniteur);
                tracing::info!(id, "virtual output destroyed");
                Ok(())
            }
            Err(error) => {
                // The GUID changes list rather than being forgotten or left
                // in place. Leaving it in `apparies` would be the real danger:
                // a display driver commonly reassigns its target
                // identifiers, and this stale entry would then pair a
                // later `detruire` carrying the same identifier — the
                // wrong GUID would go to the driver, and the live output would
                // never be destroyed. The removal stays due, it is
                // simply no longer addressable by identifier.
                let mut etat = self.etat();
                etat.apparies
                    .retain(|(_, connu, _)| *connu != guid_moniteur);
                etat.a_purger.push(guid_moniteur);
                drop(etat);
                tracing::error!(
                    id,
                    guid = ?guid_moniteur,
                    %error,
                    "virtual output NOT destroyed — manual purge required"
                );
                Err(error)
            }
        }
    }
}

impl Drop for PiloteParIoctl {
    fn drop(&mut self) {
        // Last opportunity to say what remains due. Closing the device
        // removes nothing: a virtual output outlives the process. These GUIDs are
        // what a purge — that of task 7, or a human — will have to aim at.
        //
        // BOTH lists are due here, not only `a_purger`. The
        // distinction that separates them — "an entry of `apparies` can still be
        // requested again by identifier" — stops making sense at the
        // precise moment the process ends: no one will request
        // anything again. A caller that uses `create` without going through the
        // `Sorties` guard, or whose guard was neutralised, would otherwise leave N
        // monitors behind it in total silence.
        //
        // The two origins are distinguished because they do not diagnose
        // the same thing: a GUID from `apparies` accuses a caller that did not
        // use the guard, a GUID from `a_purger` accuses a removal the
        // driver refused.
        let etat = self.etat();
        let apparies: Vec<GUID> = etat.apparies.iter().map(|(_, guid, _)| *guid).collect();
        let a_purger = etat.a_purger.clone();
        // Numbers assigned and not returned: normally equal to the number of GUIDs
        // below. A gap would signal a leak of the allocator (a number
        // consumed by a creation that created nothing), that is a slot
        // lost in a deliberately narrow range.
        let numeros_en_vol = etat.numeros.en_vol();
        drop(etat);
        if !apparies.is_empty() || !a_purger.is_empty() {
            tracing::error!(
                count = apparies.len() + a_purger.len(),
                numeros_en_vol,
                guids_jamais_detruits = ?apparies,
                guids_dont_le_retrait_a_echoue = ?a_purger,
                "virtual outputs created and NOT removed — they outlive this \
                 process, purge required"
            );
        }
        let _ = unsafe { CloseHandle(self.peripherique) };
    }
}
