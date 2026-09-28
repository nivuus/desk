//! **The protocol's FOURTH family**: the announcements going UP, from the
//! browser to the bridge — `Bonjour` and `Rafraichir`.
//!
//! # Why this extraction, and why NOW
//!
//! `service.rs` was at **463** lines, margin **37**, after F5 put its
//! routing and these two functions there (+122). The cross-cutting review closing every
//! branch of this repository **adds comments** — S3 put 48 lines spread
//! over nine files, and S2 saw a file's margin drop from 30 to 17 through that
//! gesture alone. **The extraction happens BEFORE the addition, never after**, and
//! `CLAUDE.md` forbids by name compressing to get back under the line.
//!
//! ⚠️ **THE ROUTING, FOR ITS PART, STAYS IN [`super`]**, and it is not an oversight:
//! it must run **before `table.resoudre`**, and moving it here would make this
//! module the place where one believes it lives. *This module carries what the
//! announcements DO; the fact that they are routed early is a property of
//! `traiter`, and it is read there.*

use super::Etat;
use proto::fichiers::entetes;

/// L'annonce `Rafraichir` : le pont oublie ce qu'il croyait savoir.
///
/// **Two caches, and they are not both ours.** The first is
/// ours ([`crate::pont::cache`]); the second belongs to ProjFS — the **negative
/// cache**, which memorises the paths the provider said did
/// not exist. Emptying one without the other would leave a file created on the
/// local workstation not found, **by ProjFS and not by us**.
///
/// 🔵 **`PrjClearNegativePathCache` GAINS ITS FIRST PRODUCTION CALLER
/// HERE.** It has been loaded since F1, and
/// `grep -rn vider_cache_negatif agent/src/` returned until now only its
/// declaration. **R7 closes by ONE entry point out of five** — not "R7 is closed":
/// `PrjDeleteFile` and three others stay without a `PRJ_*_CB` twin.
///
/// 🔵 **And its result is TRACED, which makes the negative cache observable for
/// the first time.** F4 could only measure its differential, **zero**, because
/// no Explorer probe reached the provider. *If this number
/// is always zero, it is a FACT and not a failure* — and it will have to be
/// reported as F4 reported its zero differential.
pub(super) fn rafraichir(etat: &Etat) {
    let memorises = match etat.cache.lock() {
        Ok(mut cache) => {
            let n = cache.taille();
            cache.vider();
            n
        }
        Err(_) => 0,
    };
    let purgees = vider_le_cache_negatif(etat);
    tracing::info!(
        repertoires_oublies = memorises,
        cache_negatif_purge = purgees,
        cache_arme = etat.cache_arme,
        "rafraichissement demande par le navigateur"
    );
}

/// Empties ProjFS's negative cache and returns the number of purged entries.
///
/// ⚠️ **`None` is distinguished from `Some(0)` in the trace**: the first says
/// the virtualisation context was not there, the second that the cache was
/// empty. *Confusing them would read an absence of measurement as a zero
/// measurement* — it is the trap this repository paid for on `grep` without `-a`.
fn vider_le_cache_negatif(etat: &Etat) -> i64 {
    let Some(contexte) = etat.contexte() else {
        return -1;
    };
    let mut total: u32 = 0;
    // SAFETY: the context comes from `PrjStartVirtualizing` and lives as long as
    // virtualisation runs; `total` is a local stack slot valid for the call.
    let hr = unsafe { (etat.projfs.vider_cache_negatif)(contexte.0, &mut total) };
    if hr.is_err() {
        tracing::warn!(code = hr.0, "PrjClearNegativePathCache a echoue");
        return -1;
    }
    i64::from(total)
}

/// The `Bonjour` announcement: the browser says on which root it is mounted.
///
/// **The bridge has pushed nothing so far**, and that is the whole point: the resumption
/// of due writes left `Fil::demarrer` to wait for this announcement.
pub(super) fn bonjour(etat: &Etat, entete: &[u8]) {
    let annonce = match serde_json::from_slice::<entetes::Bonjour>(entete) {
        Ok(annonce) => annonce,
        Err(erreur) => {
            // 🔴 **A `warn!`, never a `debug!`.** An unreadable announcement means
            // that no due write will ever go out — a silence, hence
            // worse than the thirty seconds F2 measured.
            tracing::warn!(%erreur, "annonce Bonjour illisible : aucune ecriture due ne partira");
            return;
        }
    };
    tracing::info!(
        racine = %annonce.racine,
        forcer = annonce.forcer,
        "bonjour du navigateur"
    );
    let _ = etat
        .vers_ecriture
        .send(crate::pont::ecriture::fil::Ordre::Bonjour {
            racine: annonce.racine,
            forcer: annonce.forcer,
        });
}
