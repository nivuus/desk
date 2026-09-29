//! What the loop keeps from one tick to the next, and the reconciliation that
//! produces it: read the disk, filter, compare, measure the icons.
//!
//! 🔴 THIS MODULE IS A VERBATIM EXTRACTION, PERFORMED **BEFORE** THE ADDITION THAT
//! MADE IT NECESSARY. `apps/boucle.rs` was 385 lines; polling the
//! watcher, its trigger, its mode and their documentation would have
//! taken it past the 450-line gate this sub-block imposes on itself. The strong
//! form — extract first, add afterwards — was invented by sub-block
//! D9 (task 6) and played three times by D10; this repository paid **five** times for the
//! weak form, "cross the line then catch up", twice of them through a
//! COMPRESSION that `CLAUDE.md` forbids by name.
//!
//! ⚠️ **THE ONLY DIFFERENCE FROM THE ORIGINAL TEXT IS THIS HEADER, THESE `use`,
//! AND FIVE `pub(super)` QUALIFIERS** — on `Memoire`, on `reconcilier`, and
//! on the three fields `boucle.rs` reads (`catalogue`, `lancables`,
//! `icones`). ⚠️ *This line first announced FOUR, and the diff of the check
//! refuted it by showing five hunks: a count written from memory instead of
//! being read from the command's output, in the very file whose
//! header promises completeness.* They are not an embellishment: in Rust a private item of a
//! CHILD module is NOT visible from its parent, and a strictly
//! verbatim extraction would therefore not compile. The divergence is declared
//! rather than slipped through, and the check that accompanies it compares the body
//! line by line so that no other one remains.
//!
//! ⚠️ `mesurer` and the `vues` / `ecartes` fields stay PRIVATE: they do not
//! cross the boundary, and widening them "for uniformity" would open
//! a surface nobody asks for.
//!
//! ORDINARY `mod memoire;` in `boucle.rs`, **no `#[path]`**: both
//! are `#[cfg(windows)]`, and the "Child module convention" of
//! `docs/claude/module-conventions.md` states in black and white that a gated module that does not need
//! to exist on the host stays a normal child.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use proto::plateforme::{Application, SourceMax};

use crate::apps::icone::{self, magasin::Magasin};
use crate::apps::{lecture, raccourci, reconciliation};

/// What the loop keeps from one tick to the next.
#[derive(Default)]
pub(super) struct Memoire {
    /// The previous tick's catalogue, for the diff.
    pub(super) catalogue: Vec<Application>,
    /// The `.lnk` path and the `nShow` of each key, for launching.
    ///
    /// ⚠️ IT IS THE AGENT'S CATALOGUE THAT IS AUTHORITATIVE FOR LAUNCHING, never
    /// the platform's: the order only carries a key, and the
    /// platform's copy may be one reconciliation old when this one has just
    /// been read from disk.
    pub(super) lancables: BTreeMap<String, (String, i32)>,
    /// 🔴 THE ICONS OF THE CURRENT CATALOGUE, CONTENT-ADDRESSED. On this
    /// corpus, **153 applications yield 99 distinct PNGs** — 54
    /// uploads avoided. It is REPLACED at every reconciliation, never
    /// accumulated: on a process that lives for days, merging it would make it
    /// grow with no end.
    pub(super) icones: Magasin,
    /// The fingerprint and provenance already known for a key, plus the `chemin`
    /// of the `.lnk` at the time they were measured.
    ///
    /// 🔴 IT IS WHAT AVOIDS RE-EXTRACTING ON EVERY TICK. Extraction costs
    /// **2,298 ms for 153 icons** on the first tick (measured): redoing it
    /// every thirty seconds would make application discovery the
    /// most expensive item of the agent, for a disk that does not move.
    ///
    /// ⚠️ **NAMED GAP, NOT FORGOTTEN**: an application that updates itself by
    /// rewriting its `.exe` IN PLACE — same path, same declared icon,
    /// different image — will NOT be re-read. Closing it would require a
    /// timestamp or a fingerprint of the source, hence one disk access per
    /// application and per tick.
    /// ⚠️ THE FOURTH MEMBER IS THE ACCENT (sub-block G5): it DERIVES from the
    /// pixels of the icon, so recomputing it would require re-extracting the image
    /// — that is, undoing exactly the saving this cache exists
    /// to make.
    vues: BTreeMap<String, (String, Option<String>, SourceMax, Option<String>)>,
    /// The paths already reported as discarded.
    ///
    /// 🔴 WITHOUT THIS SET, THE SEVEN DISCARDED ENTRIES OF THIS VM WOULD MAKE 20,160
    /// LINES PER DAY for files that do not change — in a log
    /// shared by the supervisor, the sensor and all the children since D4.
    /// One line is emitted when a path ENTERS this set, another
    /// when it LEAVES it.
    ///
    /// ⚠️ CONSEQUENCE FOR ANY ACCEPTANCE RUN: the criterion "seven lines each naming
    /// their file" is measured on the FIRST reconciliation, within
    /// an explicit time window. Never on a whole-file total.
    ecartes: BTreeSet<String>,
}

/// A complete reconciliation: read the disk, decide, return the diff.
///
/// Also returns the whole catalogue, because the caller needs it for the
/// `complet` resend of a re-enrolment.
pub(super) fn reconcilier(
    memoire: &mut Memoire,
    contexte: super::Contexte,
) -> (reconciliation::Diff, Vec<Application>) {
    let depart = Instant::now();
    let mut brutes = Vec::new();
    let mut total = 0usize;
    for racine in lecture::racines() {
        for chemin in lecture::lnk_under(&racine) {
            total += 1;
            match lecture::lire(&chemin) {
                Ok(r) => brutes.push(r),
                // ⚠️ AN UNREADABLE `.lnk` IS SKIPPED WITH ITS TRACE. A
                // reconciliation that failed entirely on one file
                // would make the WHOLE catalogue disappear — one corrupt byte on
                // the Desktop would empty the list of applications.
                Err(error) => tracing::warn!(
                    chemin = %chemin.display(), %error, "unreadable shortcut, skipped"
                ),
            }
        }
    }

    let existe = |c: &str| std::path::Path::new(c).is_file();
    let mut vus = BTreeSet::new();
    let mut catalogue = Vec::new();
    let mut lancables = BTreeMap::new();
    let mut ecartes = BTreeSet::new();
    // 🔴 G1 LEGACY NO. 7, CLOSED HERE. The `retenus` field emitted below was
    // `lancables.len()` — a table indexed by KEY, hence ALWAYS equal to
    // `cles`. The shortcuts actually retained were emitted NOWHERE, and
    // the field lied about its name.
    //
    // **Measured on 21 August 2026 on the development VM: `retenus=167` for
    // `cles=154`.** TWO DIFFERENT NUMBERS, hence a check that can
    // fail — that is all that is asked of it.
    //
    // ⚠️ **The same survey gives `169`/`156` at the end of the G2 acceptance run**, and the gap
    // is not a drift: the run created TWO witness shortcuts on the
    // Desktop (`G2 Temoin 48.lnk`, `G2 Temoin 256.lnk`) for criterion ②, and
    // **they STAYED there** — that is what makes the criterion replayable without rebuilding
    // anything. A later work stream counting 154 on this VM will
    // look for them: they carry distinct arguments (`--g2-temoin-48` and
    // `--g2-temoin-256`), otherwise their key would be the same and the catalogue
    // would only keep one.
    let mut retenus = 0usize;
    let mut icones = Magasin::new();
    let mut vues = BTreeMap::new();
    // 🔴 THE REGISTRY IS READ **ONCE PER RECONCILIATION**, NOT ONCE
    // PER APPLICATION. This VM's corpus has 156 applications:
    // querying them one by one would re-read every entry of `FileExts` 156 times,
    // every `PERIODE_RECONCILIATION`. Each application then LOOKS UP
    // its path in this table.
    let associations = crate::apps::associations::table_de_la_machine();
    let mut extraites = 0usize;
    let mut echecs_icone = 0usize;
    for r in brutes {
        match raccourci::retenir(&r.brut, &existe) {
            Ok(()) => {
                retenus += 1;
                let chemin_lnk = r.brut.chemin.clone();
                let icone_location = r.icone.clone();
                let mut app = raccourci::depuis_brut(r.brut);
                lancables.insert(app.cle.clone(), (chemin_lnk.clone(), r.montrer));
                if vus.insert(app.cle.clone()) {
                    // 🔴 EXTRACT ONLY IF THE KEY IS NEW OR IF THE `.lnk`
                    // HAS MOVED — never on every tick.
                    match memoire.vues.get(&app.cle) {
                        Some((ancien, empreinte, source, accent)) if *ancien == chemin_lnk => {
                            app.icone = empreinte.clone();
                            app.source_max = *source;
                            // 🔴 THE ACCENT IS STORED WITH THE ICON, NOT
                            // RECOMPUTED: it derives from the PIXELS, so
                            // recomputing it would require re-extracting the image —
                            // that is, undoing the saving this cache
                            // exists to make (153 COM extractions per tick).
                            app.accent = accent.clone();
                            // The bytes are still needed: the store is
                            // replaced on every tick, and the platform may
                            // ask again for an icon it lost.
                            if let Some(e) = empreinte {
                                if let Some(o) = memoire.icones.octets(e) {
                                    icones.add(o.to_vec());
                                }
                            }
                        }
                        _ => {
                            let (e, s, a) =
                                mesurer(&chemin_lnk, &icone_location, &app.cible, &mut icones);
                            // 🔴 A DISARM IS NOT A FAILURE, and
                            // counting them together would read 156 failures on a
                            // perfectly healthy agent that was just switched off
                            // with `ICONES=0` — MEASURED on 21 August 2026, before
                            // this line. It is exactly the defect that
                            // G1 legacy no. 7 carried on `retenus`: a
                            // counter that lies about its name.
                            if e.is_some() {
                                extraites += 1;
                            } else if icone::armee() {
                                echecs_icone += 1;
                            }
                            app.icone = e;
                            app.source_max = s;
                            app.accent = a;
                        }
                    }
                    // ⚠️ THE ASSOCIATIONS ARE NOT STORED WITH THE ICON,
                    // and that is deliberate: they only cost one lookup
                    // in a table already at hand, and they CHANGE without
                    // the `.lnk` moving — it is enough for the user to choose
                    // another default application. Putting them in the icon's
                    // cache would freeze that choice until the next move
                    // of the shortcut.
                    app.associations =
                        crate::apps::associations::pour_cible(&associations, &app.cible);
                    vues.insert(
                        app.cle.clone(),
                        (
                            chemin_lnk,
                            app.icone.clone(),
                            app.source_max,
                            app.accent.clone(),
                        ),
                    );
                    catalogue.push(app);
                }
            }
            Err(motif) => {
                let chemin = r.brut.chemin.clone();
                if !memoire.ecartes.contains(&chemin) {
                    match &motif {
                        raccourci::Ecart::CibleVide => tracing::info!(
                            chemin = %chemin, motif = "target-empty", "shortcut discarded"
                        ),
                        raccourci::Ecart::Extension(e) => tracing::info!(
                            chemin = %chemin, motif = "extension", extension = %e,
                            "shortcut discarded"
                        ),
                        raccourci::Ecart::CibleAbsente => tracing::info!(
                            chemin = %chemin, motif = "target-absent", "shortcut discarded"
                        ),
                    }
                }
                ecartes.insert(chemin);
            }
        }
    }
    for parti in memoire.ecartes.difference(&ecartes) {
        tracing::info!(chemin = %parti, "shortcut reinstated");
    }

    let diff = reconciliation::diff(&memoire.catalogue, &catalogue);
    tracing::info!(
        total,
        retenus,
        cles = catalogue.len(),
        icones = extraites,
        icones_echouees = echecs_icone,
        icones_distinctes = icones.len(),
        apparues = diff.apparues.len(),
        modifiees = diff.modifiees.len(),
        disparues = diff.disparues.len(),
        duree_ms = depart.elapsed().as_millis(),
        // 🔴 THE THREE G4 FIELDS. `declencheur` makes criterion ① readable:
        // without it, a reconciliation arriving just after the creation of a
        // shortcut is indistinguishable from a periodic one that happened to fall there.
        // `notifications` and `debordements` are CUMULATIVE since the
        // thread started — two successive lines can be subtracted, a delta already taken cannot
        // be recomposed.
        declencheur = contexte.declencheur.mot(),
        notifications = contexte.notifications,
        debordements = contexte.debordements,
        "catalogue reconciled"
    );

    memoire.catalogue = catalogue.clone();
    memoire.lancables = lancables;
    memoire.ecartes = ecartes;
    memoire.vues = vues;
    // 🔴 REPLACE, NEVER MERGE: the store carries the CURRENT catalogue.
    memoire.icones.remplacer(icones);
    (diff, catalogue)
}

/// Extracts the icon of a shortcut, and measures its PROVENANCE.
///
/// 🔴 `source_max` COMES FROM THE RESOURCE, NEVER FROM THE PNG. Code that
/// deduced it from the rendered size would give `256` to EVERYTHING — measured twice on
/// two fabricated witnesses, and that is the whole purpose of the sub-block.
///
/// ⚠️ **A FAILURE IS NOT A LOST APPLICATION**: an application without an
/// icon is better than a missing application (specification §7). The failure is
/// logged, `icone` is `None`, and `source_max` is `NonMesuree`.
fn mesurer(
    lnk: &str,
    icone_location: &str,
    cible: &str,
    icones: &mut Magasin,
) -> (Option<String>, SourceMax, Option<String>) {
    if !icone::armee() {
        return (None, SourceMax::NonMesuree, None);
    }
    match icone::extraire(std::path::Path::new(lnk)) {
        Ok((png, accent)) => {
            let octets = png.len();
            let empreinte = icones.add(png);
            // ⚠️ `debug!` AND NOT `info!`, AND IT IS MEASURED: this line is emitted
            // once PER APPLICATION on the first tick — 153 lines on this
            // corpus —, in a log that the supervisor, the sensor and all
            // the children share since D4. Afterwards it is emitted ONLY for
            // new keys, hence zero on an idle disk.
            //
            // 🔴 IT IS WHAT MAKES WIC'S DETERMINISM MEASURABLE: two
            // separate runs of the agent, two logs, and the fingerprints
            // are compared path by path. Without it, the only
            // observable thing would be `icones_distinctes`, which would say NOTHING about a
            // fingerprint that changes from one process to the next — the count
            // would stay the same.
            tracing::debug!(lnk, %empreinte, octets, "icon extracted");
            // ⚠️ THE PROVENANCE IS MEASURED EVEN WHEN IT IS UNKNOWN: it
            // returns `NonMesuree` without error, and that is not a failure — 37 of the
            // 153 applications of this VM are in that case.
            // ⚠️ THE ACCENT FOLLOWS THE ICON, AND `None` IS NOT A FAILURE: an
            // icon that is too pale, too dark or too transparent has NO
            // dominant colour. The manifest then OMITS `theme_color`.
            (
                Some(empreinte),
                icone::provenance_de(icone_location, cible),
                accent,
            )
        }
        Err(error) => {
            tracing::warn!(lnk, %error, "icon extraction failed: the application stays in the catalogue, without an icon");
            (None, SourceMax::NonMesuree, None)
        }
    }
}
