//! Discriminating campaign of sub-block D3: what does the ceiling of
//! four simultaneous DXGI duplications bear on?
//!
//! What we know: **8 duplications side by side in A SINGLE process hold**
//! (31 July 2026), and **the 5th, in a 5th process, is refused** with
//! `0x887A0022` (sub-block D2), by a lasting limit that resists three
//! seconds of explicit patience. Putting these two surveys together to conclude
//! "the ceiling bears on processes" is an **inference**: the two
//! set-ups differ in at least two variables. This module exists to
//! remove that inference.
//!
//! # Le montage
//!
//! A **bearer** process creates K = P×D virtual outputs, beats the driver's
//! watchdog and **duplicates nothing itself** — it is the exact position
//! of the supervisor in D2, and it is what makes the measurement comparable to the product
//! symptom. It then launches P **minimal probe** processes, which each
//! open D duplications and hold them.
//!
//! **Why the bearer does not duplicate**: a probe can die with
//! `0xc0000005`, like all these APIs. The bearer, which only touches the driver,
//! survives, and its guard destroys the K outputs. A bearer that duplicated
//! would risk taking them down with it.
//!
//! Spec : `docs/superpowers/specs/2026-08-02-multifenetres-plafond-concurrence-design.md`

mod sonde;

pub(super) use sonde::sonder;

use anyhow::{Context, Result};

use super::compteurs;
use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};

/// Maximum number of outputs the campaign allows itself to create. The driver's
/// pool is **10** (measured on 31 July 2026, refusal at the 11th creation with
/// `ERROR_TOO_MANY_NAMES`), and Apollo draws from the same one: eight leaves two of
/// margin.
const SORTIES_MAX: u16 = 8;

/// Analyse `"<P>x<D>"` : P processus sondes, D duplications chacune.
pub(super) fn analyser(valeur: &str) -> Result<(u8, u8)> {
    let (p, d) = valeur
        .split_once('x')
        .with_context(|| format!("MULTIFENETRE_PLAFOND attend « <P>x<D> », reçu « {valeur} »"))?;
    let processus: u8 = p
        .trim()
        .parse()
        .with_context(|| format!("nombre de processus illisible dans « {valeur} »"))?;
    let duplications: u8 = d
        .trim()
        .parse()
        .with_context(|| format!("nombre de duplications illisible dans « {valeur} »"))?;
    anyhow::ensure!(processus >= 1, "au moins un processus sonde est nécessaire");
    anyhow::ensure!(
        duplications >= 1,
        "au moins une duplication par sonde est nécessaire"
    );
    let total = u16::from(processus) * u16::from(duplications);
    anyhow::ensure!(
        total <= SORTIES_MAX,
        "{processus}x{duplications} demande {total} sorties, le maximum est {SORTIES_MAX} \
         (vivier du pilote de 10, dont deux de marge pour Apollo)"
    );
    Ok((processus, duplications))
}

/// The bearer: K = P×D virtual outputs, P probes launched in a STAIRCASE.
///
/// **The staggering is not a convenience.** P concurrent probes would return
/// a refusal without an identifiable rank, and the matrix would settle nothing: it is
/// the rank of the first refusal that distinguishes "ceiling on processes" from
/// "ceiling on duplications".
pub(super) fn mesurer(processus: u8, duplications: u8) -> Result<()> {
    let total = u16::from(processus) * u16::from(duplications);
    tracing::info!(
        processus,
        duplications,
        total,
        "campagne du plafond — début"
    );

    // No residue from a previous draw: a stale verdict would read as a
    // success where the probe never started.
    let _ = std::fs::remove_file(sonde::chemin_arret());
    for rang in 0..processus {
        let _ = std::fs::remove_file(sonde::chemin_verdict(rang));
    }
    // Fallback verdict of an unreadable rank: no effect on the measurement (the
    // bearer never reads it), but misleading for whoever digs through `%TEMP%`
    // afterwards if a previous draw left it behind.
    let _ = std::fs::remove_file(sonde::chemin_verdict_rang_invalide());

    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Explicit scope of the guard: the outputs must be destroyed BEFORE
    // the final survey, otherwise the latter would describe a transient state.
    //
    // Wrapped in an immediately invoked closure, and not a mere
    // `{ }` block: a `?` INSIDE a block exits the whole `mesurer` function,
    // not only the block — it would then skip the replay of due removals,
    // the removal of the stop signal and the final topology restoration
    // survey below. The closure captures a failure in `issue` as
    // a normal value: execution continues after it on ALL
    // paths, including those where the creation, the wait or the check of the
    // appeared names fail — not only those internal to
    // `conduire_les_sondes`. `sorties` stays local to this closure, hence
    // always destroyed before `issue` is produced: the scope
    // invariant does not change nature with the closure.
    let issue = (|| -> Result<()> {
        let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
        for rang in 1..=total {
            sorties
                .creer(largeur, hauteur, hertz)
                .with_context(|| format!("création de la sortie virtuelle n°{rang}/{total}"))?;
        }

        // Wait for the K outputs to be ATTACHED, while beating the
        // watchdog: the driver removes the outputs of a client that stops
        // pinging, including those just created.
        //
        // Gap from the brief (Step 1): `attendre_en_pinguant` does NOT take
        // `(&pilote, &noms_avant, attendues)` and does not return the appeared
        // names — its real signature, noted in `montee.rs:176`, is
        // `(&PiloteParIoctl, Duration) -> Result<()>`: it waits `duree` while
        // pinging, full stop. Same pattern as `paralleles.rs`, the
        // set-up closest to this one: it is a topology survey
        // AFTER the wait that returns the names, not the wait function
        // itself.
        attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;

        let apres_creation = relever_topologie("après création")?;
        let noms_apres_creation = noms_attaches(&apres_creation);
        let apparues: Vec<String> = noms_apres_creation
            .iter()
            .filter(|nom| !noms_avant.contains(nom))
            .cloned()
            .collect();
        // Addition beyond the brief: without this check, fewer than `total` appeared
        // names would make `conduire_les_sondes` panic on an out-of-bounds
        // slicing rather than return a readable error — the
        // same risk that `designer_sorties_neuves` (`paralleles.rs`) covers
        // for its own measurement. `noms_attaches` already filters on
        // `attachee_au_bureau`: an output created but not composed by
        // Windows would therefore not count as appeared, hence the message
        // below which NAMES this case rather than only speaking
        // of "external addition or removal" (the only exact diagnosis would be
        // to take the raw survey again to distinguish the two causes, as
        // `designer_sorties_neuves` does — out of reach of a one-line
        // fix, so only the message is fixed here).
        anyhow::ensure!(
            apparues.len() == usize::from(total),
            "{} sortie(s) DXGI attachée(s) neuve(s) après création de {total} ({apparues:?}) \
             — addition externe, retrait par le chien de garde, ou sortie créée mais jamais \
             composée par Windows (non attachée au bureau)",
            apparues.len()
        );
        tracing::info!(attachees = apparues.len(), noms = ?apparues, "sorties rattachées");

        // The `Garde` keeps beating the watchdog DURING the whole
        // conduct of the probes, not only during creation: without it,
        // not a single ping would leave between this point and the end of
        // `conduire_les_sondes`, that is up to P × (D × 3 s + margin) of
        // silence — far beyond the 11.1 s gap that `Garde` was created
        // to close (`compteurs.rs`), on a watchdog whose `delai` unit
        // is still not established. Built here, right after the
        // last ping of `attendre_en_pinguant`: the seam between the two
        // is negligible (66 µs measured elsewhere on the same pattern), not
        // counted in `intervalle_max`.
        let mut garde = compteurs::Garde::nouvelle(&pilote);
        let issue_conduite = conduire_les_sondes(processus, duplications, &apparues, &mut garde);
        tracing::info!(
            intervalle_ping_max_ms = garde.intervalle_max().as_millis() as u64,
            "chien de garde : plus grand écart entre deux battements sur toute la conduite"
        );
        issue_conduite
    })();

    // Second attempt at the removals the guard did not obtain: last
    // chance of THIS process to replay them, beyond that only the
    // inter-process purge (`MULTIFENETRE_VDD_PURGE`) will reach them. Same gesture
    // as `monter_en_n` and `paralleles::mesurer` — without it, on a pool of
    // 10 and a matrix of five ranks played one after another, an orphaned output
    // of one rank would poison the following ranks.
    let rejoues = crate::moniteurs_virtuels::purge::rejouer_purge_due(&pilote);
    if rejoues > 0 {
        tracing::info!(rejoues, "retraits dus rejoués avec succès après la garde");
    }

    // Stop signal removed: the next draw starts clean.
    let _ = std::fs::remove_file(sonde::chemin_arret());

    // An indirect display driver does not undo its topology
    // instantly: Windows reconfigures. Without this grace delay (same value and
    // same reason as `monter_en_n` and `paralleles::mesurer`), the survey
    // below would risk still seeing the K outputs being removed
    // and wrongly writing "topologie NON restaurée".
    std::thread::sleep(DELAI_TOPOLOGIE);
    let apres = relever_topologie("après destruction")?;
    let noms_apres = noms_attaches(&apres);
    // Compare SETS OF NAMES, never cardinalities: Apollo can
    // add an output at any moment, and an external addition would exactly
    // compensate a removal.
    if noms_apres != noms_avant {
        tracing::error!(
            avant = ?noms_avant, apres = ?noms_apres,
            "topologie NON restaurée — contrôler depuis un processus neuf (MULTIFENETRE_DXGI=1)"
        );
    } else {
        tracing::info!("topologie restaurée nom pour nom");
    }
    issue
}

/// Guard that drops the stop signal and waits for the probes still alive,
/// **whatever happens** — including on an error path (a `?` from
/// `spawn`, or now a failed ping relayed by `Garde::battre_si_du`
/// from `attendre_le_verdict`) or during the unwinding of a panic.
/// Same pattern as `moniteurs_virtuels::Sorties`.
///
/// Without it, a `?` that exits after a few probes already launched leaves them
/// looping indefinitely at 10 Hz on `chemin_arret()`, holding their
/// DXGI duplications and skewing the next rank of the matrix — a ceiling
/// measured at rank N+1 would then reflect rank N duplications still
/// alive, not only those of N+1.
struct SondesEnCours {
    enfants: Vec<(u8, std::process::Child)>,
}

impl SondesEnCours {
    fn nouvelle() -> Self {
        Self {
            enfants: Vec::new(),
        }
    }

    fn ajouter(&mut self, rang: u8, enfant: std::process::Child) {
        self.enfants.push((rang, enfant));
    }
}

impl Drop for SondesEnCours {
    fn drop(&mut self) {
        if self.enfants.is_empty() {
            return;
        }
        // Best-effort: on this path, something has already gone wrong before
        // reaching this `drop` — it is the signal that counts, a failed
        // write here would worsen nothing beyond the failure already logged upstream.
        if let Err(erreur) = std::fs::write(sonde::chemin_arret(), b"1") {
            tracing::error!(
                %erreur,
                "dépôt du signal d'arrêt échoué — sondes potentiellement orphelines"
            );
        }
        for (rang, enfant) in &mut self.enfants {
            match enfant.wait() {
                Ok(statut) => tracing::info!(sonde = *rang, ?statut, "sonde terminée"),
                Err(erreur) => {
                    tracing::error!(sonde = *rang, %erreur, "attente de la sonde échouée")
                }
            }
        }
    }
}

/// Margin added to the worst case of opening retries (`D × 3 s`) to bound
/// `attendre_le_verdict`: start-up of the child process, up to D D3D11 device
/// creations, and the write latency of the verdict file.
/// **A judgement, not a measurement** — no survey of these work streams puts a figure on this
/// combined cost.
const MARGE_ATTENTE_VERDICT: std::time::Duration = std::time::Duration::from_secs(15);

/// Launches the probes in a staircase and logs the verdict table.
fn conduire_les_sondes(
    processus: u8,
    duplications: u8,
    noms: &[String],
    garde: &mut compteurs::Garde<'_>,
) -> Result<()> {
    let executable = std::env::current_exe().context("chemin de l'exécutable courant")?;
    let mut sondes = SondesEnCours::nouvelle();
    let mut verdicts: Vec<(u8, String)> = Vec::new();

    for rang in 0..processus {
        let debut = usize::from(rang) * usize::from(duplications);
        let lot: Vec<&str> = noms[debut..debut + usize::from(duplications)]
            .iter()
            .map(|s| s.as_str())
            .collect();
        tracing::info!(sonde = rang, sorties = ?lot, "lancement de la sonde");

        let enfant = std::process::Command::new(&executable)
            .env("MULTIFENETRE_PLAFOND_SONDE", lot.join(","))
            .env("MULTIFENETRE_PLAFOND_RANG", rang.to_string())
            // The bearer's variable must NOT be inherited: the child
            // would recreate K outputs. The routing already handles the probe
            // first (moved to the head, see `multifenetre.rs::aiguiller`),
            // but removing the variable makes the invariant explicit.
            .env_remove("MULTIFENETRE_PLAFOND")
            .spawn()
            .with_context(|| format!("lancement de la sonde {rang}"))?;
        sondes.ajouter(rang, enfant);

        let verdict = attendre_le_verdict(rang, duplications, garde)?;
        tracing::info!(sonde = rang, %verdict, "verdict reçu");
        // `MORTE` stops the staircase just as a `KO` does: letting it
        // through would launch the next probe while this one may
        // still be opening its duplications (up to `D × 3 s` of retries alone),
        // breaking the staircase SILENTLY — exactly what the set-up exists
        // to avoid (see the file's header comment).
        let arret = verdict.starts_with("KO") || verdict.starts_with("MORTE");
        verdicts.push((rang, verdict));
        if arret {
            // We stop at the first refusal (or at the first silent probe):
            // it is ITS rank that is the measurement. Continuing would only return
            // derived refusals.
            break;
        }
    }

    // Stop signal and waiting for the children, explicit here rather than
    // left to the destruction of `sondes` at the end of the function: the summary
    // below must be logged once all probes are released, and
    // only a `drop` placed at this precise place guarantees it. `SondesEnCours`
    // would make the same gesture anyway if this line were skipped by an
    // error path — that is the whole point of the guard.
    drop(sondes);

    tracing::info!(
        processus,
        duplications,
        lancees = verdicts.len(),
        verdicts = ?verdicts,
        "campagne du plafond — bilan"
    );
    Ok(())
}

/// Waits for a probe's verdict, or concludes that it died without returning one.
///
/// **Bounded** on `duplications × DUREE_FENETRE_OUVERTURE + MARGE_ATTENTE_VERDICT`
/// and not on a fixed constant: `DesktopCapture::sur_sortie` retries
/// during `DUREE_FENETRE_OUVERTURE` (3 s) PER refused duplication, so a
/// probe at D=8 can legitimately spend 24 s on retries alone before
/// writing its verdict. A fixed 30 s bound would declare `MORTE` a probe
/// still alive at that rank, and the bearer would launch the next one in
/// parallel — the staircase would be broken without any message saying so.
///
/// Keeps beating the watchdog during the wait
/// (`Garde::battre_si_du`, without a per-round trace): without it, not a single
/// ping would leave as long as a probe has not returned its verdict.
fn attendre_le_verdict(
    rang: u8,
    duplications: u8,
    garde: &mut compteurs::Garde<'_>,
) -> Result<String> {
    let limite = crate::capture_reprise::DUREE_FENETRE_OUVERTURE
        .saturating_mul(u32::from(duplications))
        + MARGE_ATTENTE_VERDICT;
    let echeance = std::time::Instant::now() + limite;
    let chemin = sonde::chemin_verdict(rang);
    loop {
        // Read race with `sonde.rs`, which writes through `fs::write`
        // (truncation then write, not atomic): a read falling
        // between the two would return a readable but EMPTY file. An empty
        // content is not a verdict — treating it as one would read
        // `"".starts_with("KO")` = false, so a refusal would read as a
        // success and the staircase would wrongly continue.
        if let Ok(contenu) = std::fs::read_to_string(&chemin) {
            let contenu = contenu.trim();
            if !contenu.is_empty() {
                return Ok(contenu.to_string());
            }
        }
        if std::time::Instant::now() >= echeance {
            return Ok(format!("MORTE (aucun verdict en {} s)", limite.as_secs()));
        }
        garde.battre_si_du()?;
        // No trace here: this loop polls at 10 Hz, and a per-round
        // trace has already destroyed a measurement of this project (18,619 lines in
        // a few seconds on a network share, TURN work stream).
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_cinq_rangs_de_la_matrice_sont_acceptes() {
        for (valeur, attendu) in [
            ("1x8", (1, 8)),
            ("2x4", (2, 4)),
            ("4x2", (4, 2)),
            ("8x1", (8, 1)),
            ("4x1", (4, 1)),
        ] {
            assert_eq!(analyser(valeur).unwrap(), attendu, "rang {valeur}");
        }
    }

    #[test]
    fn un_produit_au_dela_du_vivier_est_refuse() {
        let erreur = analyser("4x4").unwrap_err().to_string();
        assert!(erreur.contains("16 sorties"), "reçu « {erreur} »");
    }

    #[test]
    fn un_zero_est_refuse() {
        assert!(analyser("0x4").is_err());
        assert!(analyser("4x0").is_err());
    }

    #[test]
    fn une_valeur_malformee_est_refusee() {
        assert!(analyser("4").is_err());
        assert!(analyser("quatre x deux").is_err());
    }
}
