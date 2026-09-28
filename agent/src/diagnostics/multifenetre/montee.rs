//! Measurement ①: how many simultaneous virtual outputs does this driver accept?
//!
//! Without this figure, the "one virtual monitor per window" path — the one the
//! capture probe recommended — cannot be specified, and the arbitration of
//! work stream D tips over.
//!
//! Separate from `moniteurs.rs`, which is *the driver*: here it is *the measurement*. The
//! split is not cosmetic — `moniteurs.rs` was already at 456 lines,
//! and the project's 500-line ceiling forbade pouring it in there.
//!
//! **Two probes.** `MULTIFENETRE_VDD_VEILLE` observes the driver's
//! watchdog; `MULTIFENETRE_VDD` takes the measurement of the ceiling.
//!
//! # The watchdog, and why it does not skew this measurement
//!
//! The driver advertises `delai = 3` in a unit the upstream header does NOT document.
//! If that unit were the second, a scale-up in N could see its outputs
//! removed along the way, and the published ceiling would be the watchdog's,
//! not the driver's — a wrong measurement, and wrong in the direction that
//! would wrongly condemn the recommended path.
//!
//! **What rules out this risk is the scale-up log itself, and nothing
//! else.** At the moment the driver refuses the 11th creation, the eleven outputs
//! already obtained are all present, surveyed BY NAME (`\\.\DISPLAY1`,
//! then `DISPLAY5` to `DISPLAY14`) — thirty seconds after the first
//! creation. None was removed during the scale-up. This argument depends
//! on no assumption about the unit of `delai`, nor on the effectiveness of the ping.
//!
//! **What the test without ping does NOT establish.** It notes that no output
//! was removed in 180 s of silence on our side — but Apollo runs on
//! this VM and pings the same driver the whole time. A watchdog
//! of three SECONDS rearmed by someone else would be perfectly compatible with this
//! survey. The unit of `delai` therefore remains entirely unknown, and the test
//! says nothing about what would happen to a lone and silent client.
//!
//! **The ping is proven accepted; that it ACTS is only indicated.**
//! `IOCTL_DRIVER_PING` answers without error, which confirms an IOCTL code the
//! reconnaissance had not found in bytes. On its effect, two surveys
//! and two outcomes: in the first, the countdown was 3 before as after, so the
//! criterion stayed silent; in the second, 2 before and 3 after, the two readings
//! bracketing the ping within less than a millisecond — a serious hint, on ONE
//! occurrence, which a coincidence with an Apollo ping in that window
//! does not formally exclude. The scale-up therefore pings as a precaution, and rests
//! its validity on none of this.
//!
//! # Results measured on 31 July 2026
//!
//! Logs in `docs/superpowers/plans/journaux-mesures-prealables/`:
//! `moniteurs-montee-en-n.log`, `moniteurs-chien-de-garde.log`,
//! `moniteurs-etat-initial.log`.
//!
//! - **Ceiling: 10 simultaneous virtual outputs.** The 11th creation is
//!   refused by the driver itself, with `0x80070044` (`ERROR_TOO_MANY_NAMES`),
//!   the ten previous ones all staying attached and all named in the survey.
//! - Independent corroboration of the same figure: the `TargetId`s returned by the
//!   driver run through a cycle of ten values (256 to 265) and replay it —
//!   which looks like a fixed pool of ten targets rather than a counter.

use std::time::{Duration, Instant};

use anyhow::Result;

use crate::capture::SortieDxgi;
use crate::moniteurs_virtuels::pilote::{ouvrir_pilote, PiloteParIoctl};
use crate::moniteurs_virtuels::Sorties;

/// Beyond that, we stop searching: work stream D targets 8 windows, and the encoder
/// probe already uses this same search ceiling.
///
/// **ONLY concerns the measurement.** `purge.rs` also used this constant
/// to regenerate the GUIDs of a run killed outright; it now has
/// its own, `moniteurs_virtuels::numeros::PLAFOND_NUMEROS`, next to the
/// allocator that enforces it (fix I1 of the final review). Both
/// are sixteen, and this coincidence commits to nothing: this one says
/// how high a measurement climbs, the other bounds the numbers a monitor can
/// carry — the second decides the recoverability of a system state, not the
/// first.
pub(super) const PLAFOND_RECHERCHE: usize = 16;

/// Resolution requested for each output: the one work stream D targets per
/// window, not the desktop's.
///
/// `pub(super)`: `capture_virtuelle.rs` creates ITS output at the same resolution
/// as the one whose ceiling the scale-up in N measured — two measurements of the same
/// work stream diverging on this point would no longer be comparable.
pub(super) const RESOLUTION: (u32, u32, u32) = (1280, 720, 60);

/// An indirect display driver does not publish its output instantly:
/// Windows reconfigures its display topology. Querying DXGI too early
/// would conclude to a refusal where there is only a delay.
///
/// `pub(super)`: `purge.rs` surveys the same topology, before and after its
/// purge, with the same grace delay.
// `pub(crate)`: `moniteurs_virtuels::purge::purger` (production) uses it
// for the same before/after survey.
pub(crate) const DELAI_TOPOLOGIE: Duration = Duration::from_secs(3);

/// Watchdog ping cadence, **as a precaution and not as a demonstrated
/// remedy**: the ping is proven accepted by the driver, its effect on the countdown
/// being only indicated by one occurrence (see the header comment).
///
/// One second is a third of `delai = 3` read in seconds, the most
/// unfavourable reading of this field of unknown unit. It is also the cadence of the
/// upstream client (`sleepInterval = timeout * 1000 / 3` in its ping thread). If
/// the unit is larger, pinging too often only costs one empty IOCTL per
/// second.
const CADENCE_PING: Duration = Duration::from_secs(1);

/// Default duration of the watchdog test: ten times the advertised delay
/// read in seconds, the shortest horizon worth observing.
///
/// The value of `MULTIFENETRE_VDD_VEILLE` replaces it when it is an integer
/// — the test is made to be replayed at different horizons, and
/// recompiling to change a wait duration would be absurd.
const DUREE_EPREUVE_PAR_DEFAUT: Duration = Duration::from_secs(30);

/// Surveys the DXGI topology and logs it, output by output.
///
/// Returns the whole list and not a cardinality: everything these two probes
/// check rests on the IDENTITY of the outputs (their `nom_sortie`), not on
/// their number. A cardinality does not distinguish an addition from a compensated
/// replacement.
///
/// `pub(super)`: `purge.rs` uses it for the same before/after survey.
// `pub(crate)`: `moniteurs_virtuels::purge::purger` (production) uses it
// for the same before/after survey.
pub(crate) fn relever_topologie(moment: &str) -> Result<Vec<SortieDxgi>> {
    let sorties = crate::capture::enumerer_sorties()?;
    let attachees = sorties.iter().filter(|s| s.attachee_au_bureau).count();
    tracing::info!(
        moment,
        nombre = sorties.len(),
        attachees,
        "topologie relevée"
    );
    for sortie in &sorties {
        tracing::info!(
            moment,
            nom = %sortie.nom_sortie,
            adaptateur = %sortie.adaptateur,
            index_adaptateur = sortie.index_adaptateur,
            index_sortie = sortie.index_sortie,
            attachee = sortie.attachee_au_bureau,
            x = sortie.rect.x,
            y = sortie.rect.y,
            largeur = sortie.rect.width,
            hauteur = sortie.rect.height,
            "sortie"
        );
    }
    Ok(sorties)
}

/// Names of the outputs attached to the desktop, sorted — hence comparable as
/// sets, DXGI's enumeration order having no meaning.
///
/// `pub(crate)`: `capture_virtuelle.rs` compares the same sets before and
/// after its measurement, and `superviseur::boucle` (production) only pairs a freshly
/// created output among the names that APPEARED — otherwise it would place the
/// window on a pre-existing monitor of the same dimensions.
pub(crate) fn noms_attaches(sorties: &[SortieDxgi]) -> Vec<String> {
    let mut noms: Vec<String> = sorties
        .iter()
        .filter(|s| s.attachee_au_bureau)
        .map(|s| s.nom_sortie.clone())
        .collect();
    noms.sort();
    noms
}

/// Names present in `reference` and absent from `observes`.
fn manquants(reference: &[String], observes: &[String]) -> Vec<String> {
    reference
        .iter()
        .filter(|nom| !observes.contains(nom))
        .cloned()
        .collect()
}

/// Waits `duree` while beating the driver's watchdog.
///
/// A bare `sleep` would leave the driver free to remove our outputs during
/// the reconfiguration wait. That this beat really prevents it is
/// not established — it is a precaution, whose cost is nil.
pub(super) fn attendre_en_pinguant(pilote: &PiloteParIoctl, duree: Duration) -> Result<()> {
    let debut = Instant::now();
    loop {
        pilote.pinguer()?;
        let restant = duree.saturating_sub(debut.elapsed());
        if restant.is_zero() {
            return Ok(());
        }
        std::thread::sleep(restant.min(CADENCE_PING));
    }
}

/// `MULTIFENETRE_VDD_VEILLE` probe: what does the watchdog do with an output
/// whose creator stays silent?
///
/// Two observations, made once per second and without ever pinging:
///
/// - **the driver's own countdown** (`IOCTL_GET_WATCHDOG`), the only window
///   onto the unit of `delai`;
/// - **the presence of THE created output**, spotted by its DXGI name and followed
///   by name — not a cardinality, which an external addition would compensate.
///
/// **This probe cannot conclude on its own, and that is known in advance.** Apollo
/// runs on this VM and pings the same driver: everything it surveys is
/// compatible with a three-second watchdog rearmed by someone else. It
/// is here to say what happens under the real conditions of the VM, and
/// to test the code of `IOCTL_DRIVER_PING` — not to establish a unit.
pub(super) fn eprouver_chien_de_garde() -> Result<()> {
    let duree = std::env::var("MULTIFENETRE_VDD_VEILLE")
        .ok()
        .and_then(|valeur| valeur.parse().ok())
        .map_or(DUREE_EPREUVE_PAR_DEFAUT, Duration::from_secs);
    let noms_avant = noms_attaches(&relever_topologie("avant création")?);

    let pilote = ouvrir_pilote()?;
    let (veille_initiale, _) = pilote.veille()?;
    tracing::info!(
        delai = veille_initiale.delai,
        decompte = veille_initiale.decompte,
        duree_epreuve_s = duree.as_secs(),
        "chien de garde avant création — l'épreuve qui suit ne pingue JAMAIS"
    );

    let (largeur, hauteur, hertz) = RESOLUTION;
    let mut sorties = Sorties::nouvelles(&pilote);
    let id = sorties.creer(largeur, hauteur, hertz)?;
    let debut = Instant::now();

    let mut nom_cree: Option<String> = None;
    let mut disparue_a = None;
    let mut decomptes = Vec::new();
    while debut.elapsed() < duree {
        std::thread::sleep(Duration::from_secs(1));
        let seconde = debut.elapsed().as_secs();
        let (veille, _) = pilote.veille()?;
        let noms = match crate::capture::enumerer_sorties() {
            Ok(liste) => noms_attaches(&liste),
            Err(erreur) => {
                tracing::warn!(seconde, %erreur, "énumération DXGI en échec pendant l'épreuve");
                continue;
            }
        };
        if nom_cree.is_none() {
            if let Some(nouveau) = noms.iter().find(|nom| !noms_avant.contains(nom)) {
                tracing::info!(
                    seconde,
                    id,
                    nom = %nouveau,
                    "la sortie créée est repérée par son nom — c'est SA présence qui est \
                     suivie ensuite, pas un cardinal"
                );
                nom_cree = Some(nouveau.clone());
            }
        }
        let presente = nom_cree.as_ref().map(|nom| noms.contains(nom));
        decomptes.push(veille.decompte);
        tracing::info!(
            seconde,
            id,
            delai = veille.delai,
            decompte = veille.decompte,
            attachees = noms.len(),
            presente = ?presente,
            "épreuve sans ping"
        );
        if presente == Some(false) && disparue_a.is_none() {
            disparue_a = Some(seconde);
        }
    }

    match (&nom_cree, disparue_a) {
        (None, _) => tracing::error!(
            id,
            duree_epreuve_s = duree.as_secs(),
            "la sortie créée n'a JAMAIS paru dans DXGI — cas distinct d'un retrait"
        ),
        (Some(nom), Some(seconde)) => tracing::error!(
            nom = %nom,
            disparue_apres_s = seconde,
            delai_annonce = veille_initiale.delai,
            decomptes = ?decomptes,
            "SANS PING, la sortie est retirée — toute mesure de plafond doit pinguer"
        ),
        (Some(nom), None) => tracing::info!(
            nom = %nom,
            duree_epreuve_s = duree.as_secs(),
            delai_annonce = veille_initiale.delai,
            decomptes = ?decomptes,
            "rien n'a été retiré pendant l'épreuve, sans un seul ping DE NOTRE PART \
             — Apollo pinguant le même pilote pendant tout ce temps, cela n'établit \
             ni l'unité de « delai », ni ce qu'il adviendrait d'un client seul et muet"
        ),
    }

    // One ping, ONLY ONE, and at the very end: it is the only way to establish that
    // `IOCTL_DRIVER_PING` — not confirmed by bytes, unlike two of the
    // six codes — is indeed the right code, otherwise the scale-up in N would rest
    // its precaution on a call that does not exist.
    //
    // The countdown is read BEFORE and AFTER, so that the log carries the
    // COMPARISON rather than an isolated value: reading 3 after the ping says
    // nothing if it was already 3 before, and that is what happened in the first
    // survey of this probe — the criterion stayed silent there. In the second, 2 before
    // and 3 after, the two readings bracketing the ping within less than a
    // millisecond: a serious hint that the ping ACTS, on a single occurrence,
    // which a coincidence with an Apollo ping does not formally exclude.
    let avant_ping = pilote.veille().map(|(veille, _)| veille.decompte).ok();
    match pilote.pinguer() {
        Ok(()) => {
            let apres_ping = pilote.veille().map(|(veille, _)| veille.decompte).ok();
            tracing::info!(
                decompte_avant_ping = ?avant_ping,
                decompte_apres_ping = ?apres_ping,
                "IOCTL_DRIVER_PING accepté par le pilote — le code non confirmé par octets \
                 est le bon. ACCEPTÉ n'est pas AGISSANT : seul un décompte qui REMONTE \
                 prouverait un réarmement"
            );
        }
        Err(erreur) => tracing::error!(
            causes = %super::causes(erreur),
            "IOCTL_DRIVER_PING REFUSÉ — la montée en N ne pourra pas s'en servir"
        ),
    }

    // The guard destroys here. If the watchdog has already removed the output, the
    // removal will fail and be logged as an error: that is expected in
    // that case, and it is not a leak — the output no longer exists.
    //
    // This GUID then joins `a_purger`, and is deliberately NOT replayed by
    // `purge::rejouer_purge_due` here (unlike `monter_en_n`): a
    // replay would fail for the same reason as the first attempt — the output
    // already no longer exists, it is not a removal that a second attempt
    // would save. Leaving it in `a_purger` documents the fact without promising
    // a cure no replay can bring.
    Ok(())
}

/// `MULTIFENETRE_VDD` probe: the scale-up in N.
pub(super) fn monter_en_n() -> Result<()> {
    // Surveyed BEFORE any creation: without it, a manual restoration after a
    // crash would be done blindly (spec §6.3).
    let avant = relever_topologie("avant toute création")?;
    let noms_avant = noms_attaches(&avant);

    let pilote = ouvrir_pilote()?;
    let (veille, _) = pilote.veille()?;
    tracing::info!(
        delai = veille.delai,
        decompte = veille.decompte,
        cadence_ping_s = CADENCE_PING.as_secs(),
        "chien de garde pingué par précaution pendant toute la montée — effet non établi"
    );
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Explicit scope: the guard must have destroyed BEFORE the final survey,
    // otherwise the latter would describe a transient state.
    let mut arret = None;
    {
        let mut sorties = Sorties::nouvelles(&pilote);
        let mut noms_connus = noms_avant.clone();
        for rang in 1..=PLAFOND_RECHERCHE {
            pilote.pinguer()?;
            match sorties.creer(largeur, hauteur, hertz) {
                Err(erreur) => {
                    tracing::info!(
                        plafond = rang - 1,
                        causes = %super::causes(erreur),
                        presentes = ?noms_connus,
                        "plafond de sorties virtuelles atteint — le pilote refuse la suivante, \
                         et toutes les précédentes sont encore là, nommément"
                    );
                    arret = Some("refus du pilote");
                    break;
                }
                Ok(id) => {
                    attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;
                    let apres = relever_topologie(&format!("après création {rang}"))?;
                    let noms = noms_attaches(&apres);

                    // The check bears on NAMES, not on a cardinality.
                    // Comparing `attachees` with `attachees_avant + rang` would let
                    // through the case where one output disappears while another
                    // appears — yet Apollo drives the display configuration of
                    // this VM and can add one at any moment, which
                    // would exactly compensate a removal by the watchdog.
                    // Three distinct defects can be read here:
                    //   - `disparues` not empty: an already obtained output was
                    //     removed — that is the replacement Apollo made through
                    //     its `ensure_only_display` setting;
                    //   - `parues` empty: the driver accepted the request but the
                    //     output did not appear;
                    //   - `parues` with more than one name: someone else added
                    //     an output during the measurement, which is therefore no longer
                    //     attributable to the driver alone.
                    let disparues = manquants(&noms_connus, &noms);
                    let parues = manquants(&noms, &noms_connus);
                    tracing::info!(
                        rang,
                        id,
                        sorties_dxgi = apres.len(),
                        attachees = noms.len(),
                        parues = ?parues,
                        "sortie virtuelle créée"
                    );
                    if !disparues.is_empty() || parues.len() != 1 {
                        tracing::error!(
                            rang,
                            disparues = ?disparues,
                            parues = ?parues,
                            attachees = noms.len(),
                            attendu = noms_connus.len() + 1,
                            "le pilote a accepté la demande mais la topologie ne suit pas \
                             — sortie non parue, retrait d'une précédente, ou addition externe"
                        );
                        arret = Some("topologie non suivie");
                        break;
                    }
                    noms_connus = noms;
                }
            }
        }
        if arret.is_none() {
            tracing::info!(
                plafond_recherche = PLAFOND_RECHERCHE,
                "aucun plafond atteint sous {PLAFOND_RECHERCHE} sorties virtuelles"
            );
        }
        // Destruction by the guard, here, when going out of scope.
    }

    // Second attempt, before `pilote` itself goes away: if the guard
    // left a due removal (the driver refused it a first time), this is
    // the last chance of this PROCESS to replay it — beyond that, only
    // the inter-process purge of `purge.rs` will still be able to reach it.
    let rejoues = crate::moniteurs_virtuels::purge::rejouer_purge_due(&pilote);
    if rejoues > 0 {
        tracing::info!(rejoues, "retraits dus rejoués avec succès après la garde");
    }

    // A virtual output outlives the process: not checking the return to
    // the initial state would leave the VM polluted for all subsequent
    // measurements, without anyone knowing. This check remains that of the
    // measuring process, hence judge and party — the check that counts is a
    // `MULTIFENETRE_DXGI=1` survey from a fresh process, afterwards.
    std::thread::sleep(DELAI_TOPOLOGIE);
    let apres = relever_topologie("après destruction")?;
    let noms_apres = noms_attaches(&apres);
    if noms_apres == noms_avant && apres.len() == avant.len() {
        tracing::info!(
            arret = arret.unwrap_or("plafond de recherche épuisé"),
            noms = ?noms_apres,
            "état initial restauré — mêmes sorties, nommément"
        );
    } else {
        tracing::error!(
            noms_avant = ?noms_avant,
            noms_apres = ?noms_apres,
            total_avant = avant.len(),
            total_apres = apres.len(),
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }
    Ok(())
}
