//! Resolution of an audio **render** endpoint, for TWO
//! consumers with opposite policies.
//!
//! | Consumer | What it looks for | What it does on failure |
//! | --- | --- | --- |
//! | [`resoudre`] — the loopback (fix "A-bis", `wasapi.rs`) | the render device the machine PLAYS, to capture | **falls back** to Windows' default, with `warn!` |
//! | [`resoudre_cable`] — the microphone (block E2, `windows_micro.rs`) | the CABLE's render endpoint, to write to | **refuses**, and there is no microphone |
//!
//! 🔴 **This fallback asymmetry is this module's design fact, and
//! it is not an inconsistency.** A-bis falls back because "sound,
//! perhaps the wrong one, and a `warn!` that says so" is better than "no
//! sound". For the microphone the arbitration **is inverted**: "the user's voice,
//! perhaps in the wrong device" is not a lesser evil, it is a
//! **leak** — on a machine where Windows' default is the sound card, that
//! voice would come out of the VM's speakers. Better no microphone than a
//! microphone in the wrong pipe.
//!
//! ⚠️ **The module has only ever had one direction: RESOLVING.** It is its
//! callers that have opposite directions — one captures, the other writes — and
//! each keeps its own. That is what made block E2 prefer a second
//! entry point here rather than a hundred lines of COM copied
//! elsewhere: the twin of the production path that E1's task 8 had
//! precisely had to remove.
//!
//! ⚠️ **[`defaut`] is only reachable through [`resoudre`].** The cable path
//! never calls it, and a test guards the pure predicate that forbids it
//! (`wasapi_peripherique::demande_cable`, which never returns `None`).
//!
//! The selection rule, for its part, is **pure** and lives in
//! `agent/src/wasapi/peripherique.rs` (hoisted to the crate root through
//! `#[path]`, cf. `main.rs`): this module only translates the COM
//! enumeration into its types, queries it, and **logs what is retained**.
//!
//! ## Windows' default is no longer an implicit dependency
//!
//! Until now `LoopbackCapture::open` called
//! `GetDefaultAudioEndpoint(eRender, eConsole)` directly. On August 19th, 2026,
//! installing VB-Cable on the VM (preparation of workstream E) switched
//! that default to the virtual cable: the product started capturing
//! silence, **without any log line saying so**. The fix
//! is not to put the right device back as default — that would fix
//! the occurrence and leave the failure class — but to let
//! the operator **explicitly designate** their device, and to trace
//! the one actually retained at each opening.
//!
//! ## `AUDIO_PERIPHERIQUE` — VALUED convention, absence = previous behaviour
//!
//! This repository has three variable conventions, and one had to be chosen:
//!
//! - `PLEIN_ECRAN=0` **disarms** a shipped mechanism, and mere presence
//!   does not arm (otherwise writing `PLEIN_ECRAN=0` would enable it);
//! - `SOURCE_TRACE` is enabled by **mere presence**;
//! - `MULTIFENETRE_SORTIE=<\\.\DISPLAYn>` and `BUDGET_BPS=<bps>` carry a
//!   **value** that designates or tunes.
//!
//! `AUDIO_PERIPHERIQUE` follows the **third**, and it is
//! `MULTIFENETRE_SORTIE` that is the exact precedent: like it, it
//! designates a target by a **stable name** rather than by a rank. The first two
//! conventions are off topic here — there is nothing to arm or to
//! disarm, there is a target to name, and a target has no boolean
//! value. **Absent (or empty), the behaviour is exactly the one
//! from before the fix**: Windows' default render device. No
//! regression for an agent launched without it.

#![cfg(windows)]

use anyhow::{bail, Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IMMDevice, IMMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoTaskMemFree, STGM_READ};

use crate::micro::boucle_locale;
use crate::wasapi_peripherique::{choisir, demande_cable, inventaire, Choix, Peripherique};

/// Name of the LOOPBACK environment variable. See the convention at the head
/// of the module.
pub const VARIABLE: &str = "AUDIO_PERIPHERIQUE";

/// Name of the CABLE environment variable. **Valued** convention
/// too, and for the same reason: there is nothing to arm or disarm, there is
/// a target to name. Absent, the BUILT-IN designation
/// (`wasapi_peripherique::DESIGNATION_CABLE`) applies — never Windows'
/// default.
pub const VARIABLE_CABLE: &str = "MICRO_PERIPHERIQUE";

/// Elects the render device to capture and returns it, **after having traced
/// which one was retained**.
///
/// The fallback is explicit and loud: if the request does not succeed, we fall back
/// to Windows' default — but with `warn!`, naming what was
/// requested, what existed, and what is finally retained. **Never a
/// silent return to the default**: it is exactly the silent failure this
/// fix exists to remove, and replacing it with another silent
/// failure would make no sense.
///
/// The choice *not to fail* when the request does not succeed is deliberate and
/// comes from the caller's established behaviour: `demarrage/audio.rs::brancher`
/// logs and lets the session continue **silent** if the audio source
/// refuses to open. Failing here would trade "sound, perhaps the wrong one,
/// and a `warn!` that says so" for "no sound at all" — a worse deal
/// for the operator, for equal information.
pub fn resoudre(enumerateur: &IMMDeviceEnumerator) -> Result<IMMDevice> {
    let demande = std::env::var(VARIABLE).ok();
    let disponibles = enumerer(enumerateur)?;
    let choix = choisir(&disponibles, demande.as_deref());

    let (peripherique, critere) = match &choix {
        Choix::Elu {
            peripherique,
            critere,
        } => (
            ouvrir_par_identifiant(enumerateur, &peripherique.identifiant)?,
            critere.libelle(),
        ),
        Choix::Defaut => (defaut(enumerateur)?, "Windows default"),
        Choix::Introuvable { demande } => {
            tracing::warn!(
                variable = VARIABLE,
                demande = %demande,
                disponibles = %inventaire(&disponibles),
                "no render audio device matches: FALLING BACK to the Windows default, \
                 which is not necessarily the one we want to capture"
            );
            (defaut(enumerateur)?, "Windows default (fallback)")
        }
        Choix::Ambigu { demande, candidats } => {
            tracing::warn!(
                variable = VARIABLE,
                demande = %demande,
                candidats = %candidats.join(" | "),
                disponibles = %inventaire(&disponibles),
                "several render audio devices match, the designation is too \
                 broad to decide: FALLING BACK to the Windows default. Narrow the request, or \
                 give the endpoint identifier"
            );
            (defaut(enumerateur)?, "Windows default (fallback)")
        }
    };

    // The trace that counts: it names the device ACTUALLY retained, whatever
    // path led there. An acceptance run can thus check what
    // is captured without guessing it — and without it, the fix would not be
    // falsifiable.
    let retenu = decrire(&peripherique);
    tracing::info!(
        variable = VARIABLE,
        demande = demande.as_deref().unwrap_or("(none)"),
        retenu = %retenu.0,
        identifiant = %retenu.1,
        critere,
        repli = choix.est_repli(),
        "render audio device retained"
    );

    Ok(peripherique)
}

/// Elects the **CABLE** to write the microphone to, and returns its `IMMDevice` with
/// its endpoint identifier. **NO FALLBACK** (Decision 4 of plan E2).
///
/// `MICRO_PERIPHERIQUE` designates the target; absent or empty,
/// `DESIGNATION_CABLE` applies. The request is therefore **never** `None`,
/// and `Choix::Defaut` is unreachable through this path by construction — that is
/// what guarantees [`defaut`] will never be called here. A pure test guards
/// this predicate (`wasapi_peripherique::demande_cable`).
///
/// `Introuvable` **and** `Ambigu` mean failure, both with the inventory in
/// the message: the caller logs, sets no sink, and the session
/// continues without a microphone. On a machine carrying two VB-Audio cables, the
/// built-in designation becomes ambiguous and the microphone is unavailable — that is the
/// intended behaviour, not a defect.
///
/// The returned **identifier** is not an ornament: it is what the local
/// loop guard (`micro/boucle_locale::evaluer`) compares to what the loopback
/// would capture, and a comparison on the friendly name would be worthless — this
/// VM carries two render devices whose name starts with "Haut-parleurs (".
pub fn resoudre_cable(enumerateur: &IMMDeviceEnumerator) -> Result<(IMMDevice, String)> {
    let brut = std::env::var(VARIABLE_CABLE).ok();
    let demande = demande_cable(brut.as_deref());
    let disponibles = enumerer(enumerateur)?;

    let (peripherique, critere) = match choisir(&disponibles, Some(demande)) {
        Choix::Elu {
            peripherique,
            critere,
        } => (
            ouvrir_par_identifiant(enumerateur, &peripherique.identifiant)?,
            critere.libelle(),
        ),
        Choix::Introuvable { demande } => {
            bail!(
                "no render device matches « {demande} » ({VARIABLE_CABLE}):                  no mic. Available: {}. Is the virtual cable installed?",
                inventaire(&disponibles)
            );
        }
        Choix::Ambigu { demande, candidats } => {
            bail!(
                "« {demande} » ({VARIABLE_CABLE}) designates several render devices and the                  rule refuses to decide: no mic, rather than a mic in the wrong                  pipe. Candidates: {}. Available: {}. Narrow the request, or give                  the endpoint identifier",
                candidats.join(" | "),
                inventaire(&disponibles)
            );
        }
        // ⚠️ UNREACHABLE: `demande_cable` never returns an empty request, and
        // `choisir` only returns `Defaut` for an absent or empty request. The
        // arm exists so that the compiler keeps this property if either of the
        // two changed — and it FAILS rather than falling back to Windows'
        // default, which is exactly the leak this path exists to
        // prevent.
        Choix::Defaut => bail!(
            "internal inconsistency: the cable request cannot be empty              (see wasapi_peripherique::demande_cable)"
        ),
    };

    // The trace that makes Decision 4 falsifiable: without it, one cannot
    // know WHAT the microphone was written to, nor whether `MICRO_PERIPHERIQUE` even
    // reached the process. Same pattern as `resoudre`'s.
    let (nom, identifiant) = decrire(&peripherique);
    tracing::info!(
        variable = VARIABLE_CABLE,
        demande = %demande,
        integree = brut.is_none(),
        retenu = %nom,
        identifiant = %identifiant,
        critere,
        "render cable retained for mic writing"
    );

    Ok((peripherique, identifiant))
}

/// The endpoint identifier the session loopback would capture, **without
/// logging anything or opening a stream**.
///
/// The local loop guard needs to know it **before**
/// `LoopbackCapture::open` is called — the microphone's render thread starts
/// before, or at the same time, and it must not open the cable if that is what
/// the agent captures.
///
/// ⚠️ **It logs NOTHING, and that is deliberate.** [`resoudre`] already emits its
/// "render audio device retained" line; a second one, identical and
/// with no visible cause, would suggest two openings — and `agent.log` mixes
/// the supervisor and all its children since D4.
///
/// The decision itself is **pure** and lives in
/// `micro::boucle_locale::identifiant_capte`, where the four branches are
/// tested on the host against those of [`resoudre`]. Here we only
/// feed it, and resolve its `None` — "Windows' default" — through the only
/// COM call that can name it.
pub fn identifiant_capte(enumerateur: &IMMDeviceEnumerator) -> Result<String> {
    let demande = std::env::var(VARIABLE).ok();
    let disponibles = enumerer(enumerateur)?;
    match boucle_locale::identifiant_capte(&disponibles, demande.as_deref()) {
        Some(identifiant) => Ok(identifiant),
        None => {
            let peripherique = defaut(enumerateur)?;
            Ok(decrire(&peripherique).1)
        }
    }
}

/// The session's default render device — the old behaviour, now
/// reached through a single place.
fn defaut(enumerateur: &IMMDeviceEnumerator) -> Result<IMMDevice> {
    // SAFETY: the enumerator comes from a successful `CoCreateInstance` on a thread
    // that is a member of the MTA (guaranteed by the caller, `LoopbackCapture::open`).
    unsafe { enumerateur.GetDefaultAudioEndpoint(eRender, eConsole) }
        .context("no default render audio device")
}

/// Reopens a device by its endpoint identifier.
///
/// We do NOT KEEP the `IMMDevice` collected during enumeration: passing it
/// through the pure rule would force that rule to carry a `windows` type, which
/// would make it untestable on the host — the whole reason for the
/// separation. Reopening by identifier costs one COM call once per
/// session, and keeps the boundary clean.
fn ouvrir_par_identifiant(
    enumerateur: &IMMDeviceEnumerator,
    identifiant: &str,
) -> Result<IMMDevice> {
    let large: Vec<u16> = identifiant
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `large` stays alive during the whole call, and ends with
    // the zero `PCWSTR` requires.
    unsafe { enumerateur.GetDevice(PCWSTR(large.as_ptr())) }
        .with_context(|| format!("opening audio device {identifiant}"))
}

/// Snapshots the ACTIVE render devices.
///
/// ⚠️ **`pub` since block E2**, and for a single reason: when the local
/// loop guard refuses the microphone, its message must carry the inventory of
/// available render devices — otherwise the remedy it names
/// (`AUDIO_PERIPHERIQUE=<another one>`) does not say *which one*. It is the pattern of
/// the `Choix::Ambigu` arm of [`resoudre`], which already lists its candidates.
///
/// `DEVICE_STATE_ACTIVE` alone, on purpose: an unplugged or
/// disabled device can render nothing, and offering it for selection would elect
/// a target that will never produce a byte — the very failure being fixed,
/// in another form.
pub fn enumerer(enumerateur: &IMMDeviceEnumerator) -> Result<Vec<Peripherique>> {
    // SAFETY: see `defaut`. Each `Item` returns a counted reference that
    // `IMMDevice`'s `Drop` releases.
    unsafe {
        let collection = enumerateur
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .context("enumerating the render audio devices")?;
        let count = collection
            .GetCount()
            .context("counting the render audio devices")?;
        let mut peripheriques = Vec::with_capacity(count as usize);
        for index in 0..count {
            // A device we fail to describe does not interrupt the
            // enumeration: it would otherwise be impossible to elect another one
            // because of a neighbour in bad shape. `decrire` then returns
            // empty strings, which will match no non-empty request.
            let peripherique = collection
                .Item(index)
                .with_context(|| format!("reading audio device no. {index}"))?;
            let (nom, identifiant) = decrire(&peripherique);
            peripheriques.push(Peripherique { nom, identifiant });
        }
        Ok(peripheriques)
    }
}

/// Friendly name and endpoint identifier of a device.
///
/// **Never returns an error**: an undescribable device must not
/// make the sound opening fail, it must only be ineligible. An
/// empty string matches no non-empty request (the pure rule discards
/// empty requests before any comparison), so ineligibility is
/// achieved with no extra code.
fn decrire(peripherique: &IMMDevice) -> (String, String) {
    // SAFETY: `GetId` allocates through `CoTaskMemAlloc` and transfers
    // ownership to us — hence the matching `CoTaskMemFree`, after copying. The
    // `PROPVARIANT` returned by `GetValue` implements `Drop` and frees itself.
    unsafe {
        let identifiant = match peripherique.GetId() {
            Ok(brut) => {
                let texte = brut.to_string().unwrap_or_default();
                CoTaskMemFree(Some(brut.0 as *const core::ffi::c_void));
                texte
            }
            Err(_) => String::new(),
        };
        let nom = peripherique
            .OpenPropertyStore(STGM_READ)
            .and_then(|magasin| magasin.GetValue(&PKEY_Device_FriendlyName))
            .map(|value| value.to_string())
            .unwrap_or_default();
        (nom, identifiant)
    }
}
