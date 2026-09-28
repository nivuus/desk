//! Creating a virtual output for a session, and handing it back to the
//! driver.
//!
//! Extracted from `boucle.rs` (task 1 of sub-block D10) to stay under the
//! project's 500-line ceiling — **before** the addition that would have made it
//! cross, and not after. It is the only gesture that worked in D9
//! (`capteur/serveur/instances.rs`); the two files handled after the fact there
//! were compressed, a gesture `CLAUDE.md` forbids, then extracted
//! anyway.
//!
//! No decision here — the table decides, this module acts —, exactly like
//! the parent module.

use super::*;
use crate::geometry::Rect;

/// Creates a session's virtual output, pairs it with its DXGI place, puts
/// the window on it, and returns the effects to chain.
///
/// Extracted from the loop for a fundamental reason: **every failure path under
/// creation must undo the output**. A created output the DXGI topology
/// does not return would otherwise stay held until supervisor shutdown.
///
/// Each failure path calls `table.enfant_mort`, which takes the entry out
/// of `AttendLaSortie`. **Since task 10, this call no longer frees the
/// place in the capacity**: the entry switches to `Etat::SansSession` and the
/// periodic check restarts it, up to `RELANCES_MAX` times (see
/// `superviseur::table`). Consequence to know: a window whose
/// output creation fails systematically therefore sends up to
/// `RELANCES_MAX + 2` `VersLaShell::Refus` to the shell page — i.e. **five**
/// with `RELANCES_MAX = 3`: one per aborted attempt (the original plus the
/// three restarts, `relances` being 0, 1, 2 then 3), **plus** the final abandonment
/// `relancer_les_orphelines` emits when `relances >= RELANCES_MAX` — and
/// no longer just one as before this task.
pub(super) fn creer_sortie(
    pilote: &PiloteParIoctl,
    sorties: &mut Sorties<'_>,
    table: &mut Table,
    prises: &mut Vec<String>,
    envoyer: &impl Fn(&VersLaShell),
    demande: Demande,
) -> Vec<Effet> {
    let Demande {
        session,
        titre,
        largeur,
        hauteur,
    } = demande;

    // D9's LEG 5. `borner_a_la_taille_max` had been waiting for its caller since
    // output mode switching was removed: it is here.
    //
    // ⚠️ The viewport arrives in DEVICE PIXELS since D9's task 5
    // (`client/src/main.ts`, `innerWidth × devicePixelRatio`): a client at
    // `devicePixelRatio = 2` requests 2560×1440 where it requested 1280×720,
    // i.e. four times the pixels to capture and encode. And the ceiling of
    // 8 concurrent encoders has NEVER been measured beyond 720p — NVENC
    // bounds in macroblocks per second, not in number of sessions.
    //
    // `TAILLE_MAX_SORTIE` (1920×1080) is NOT calibrated: it is a
    // prudence safeguard, and no visual judgement has judged it.
    let (largeur, hauteur) =
        crate::windows_source_sortie::borner_a_la_taille_max((largeur, hauteur));

    // Recorded BEFORE creation. ⚠️ **It STOPPED being the centrepiece of
    // pairing in batch 32** — it is now its FALLBACK, the main path
    // being to designate our output by the pair the driver
    // returned to us (see `superviseur::designation`). It has not
    // become useless for all that, and what follows says why it is still computed.
    //
    // `sortie_pour_viewport` only filters on "attached, LARGE ENOUGH, not
    // already taken": nothing there excludes PRE-EXISTING outputs. Yet the viewport
    // announced by the browser can perfectly equal, or even be smaller
    // than, the resolution of a physical monitor — it is even the mundane
    // case in full screen. Without this survey, the window would be put on
    // the VM's REAL screen and the virtual output just created
    // would become orphaned. When designation returns nothing, we therefore
    // only pair among the outputs that APPEARED, and the repository has already written the
    // doctrine: compare sets of NAMES, never numbers.
    //
    // 🔴 **THIS SURVEY ALONE WAS NOT ENOUGH, and batch 30 measured it**: when
    // the new output REPLACES a forced target on the same source, it
    // inherits the previous name, hence never appears, and this set
    // difference refused a perfectly usable output. See the header
    // of `superviseur::designation`.
    let avant = match relever_topologie("avant création de sortie") {
        Ok(avant) => noms_attaches(&avant),
        Err(erreur) => {
            tracing::error!(session = %session.0, %erreur, "topologie DXGI illisible avant création");
            envoyer(&VersLaShell::Refus {
                titre: titre.clone(),
                motif: "topologie d'affichage illisible".into(),
            });
            // No output was created: nothing to hand back to the driver. But
            // the entry must leave `AttendLaSortie` — `enfant_mort`
            // switches it to `SansSession` (its place stays counted, see the doc
            // of that function) rather than removing it: the periodic
            // check will restart it.
            return table.enfant_mort(&session);
        }
    };

    let id_pilote = match sorties.creer(largeur, hauteur, 60) {
        Ok(id) => id,
        Err(erreur) => {
            tracing::error!(session = %session.0, %erreur, "création de sortie refusée");
            envoyer(&VersLaShell::Refus {
                titre,
                motif: format!("{erreur}"),
            });
            return table.enfant_mort(&session);
        }
    };

    // Waits for the FACT — that OUR output is there — rather than a flat delay, and
    // without ceasing to beat the driver's watchdog (see the doc
    // of `attendre_notre_sortie`).
    let (designee, candidates) =
        attendre_notre_sortie(pilote, id_pilote, &avant, LIMITE_RATTACHEMENT);

    let Some(cible) =
        placement::sortie_pour_viewport(&candidates, largeur, hauteur, prises, designee.as_deref())
    else {
        // This refusal can no longer come from an output born TOO LARGE — it is
        // D9's leg 4, which capped the product at three windows on a VM
        // with a polluted registry. Listing them all is the only service this
        // comment renders to whoever debugs this `ERROR`, and **the
        // `designee` field of the log below says which of the two families
        // applies**:
        //
        // `designee` NON-EMPTY — our output was named, and refused
        // anyway. ⚠️ **THIS FAMILY LOST ITS CAUSE NO. 1 ON AUGUST 31ST, 2026**:
        // "it is SMALLER than the request" can no longer refuse a
        // DESIGNATED output — `sortie_pour_viewport` exempts it, because the
        // driver does not create it at the requested size and eight looping refusals
        // had made the product entirely mute. What remains:
        //   1. it is ALREADY TAKEN — `sortie_pour_viewport` also filters on
        //      `!deja_prises`, and `rendre_la_sortie` deliberately CREATES this case:
        //      when destruction is refused by the driver, the name stays
        //      reserved so as not to be reassigned;
        //   2. it is not ATTACHED to the desktop — Windows named it without
        //      having attached it, and there would be nothing to duplicate.
        //
        // `designee` EMPTY — designation returned nothing (driver without a
        // known adapter, mute or failing CCD, target not yet in an
        // active path, ambiguous pair) and the FALLBACK ran:
        //   3. no output appeared at all;
        //   4. the one that appeared is too small, or already taken (1 and 2
        //      above, but on an output that is not necessarily
        //      ours);
        //   5. 🔴 **our output REPLACED a pre-existing output**, so
        //      it did not "appear" — batch 30's defect, which
        //      designation closes and which the fallback, for its part, cannot see.
        //
        // ❌ **This comment said "only two causes remain" for
        // a whole branch** (finding of the review of D10's task 6,
        // deferred then taken up at the final review), then "THREE" until
        // batch 32: it sent a debugger to stop searching too early, on
        // an `ERROR` whose missing causes were produced by the code
        // of the same module. **Any addition to this path recounts this list.**
        tracing::error!(
            session = %session.0,
            demande = format!("{largeur}x{hauteur}"),
            // Empty when designation returned nothing: it is what decides between
            // the two families of causes listed just above, and without this
            // field they would be indistinguishable in the log.
            designee = designee.as_deref().unwrap_or(""),
            candidates = ?candidates
                .iter()
                .map(|s| format!("{} {}x{}", s.nom_sortie, s.rect.width, s.rect.height))
                .collect::<Vec<_>>(),
            "aucune sortie candidate ne peut servir ce viewport — elle est rendue au pilote"
        );
        rendre_sans_apparier(sorties, id_pilote);
        envoyer(&VersLaShell::Refus {
            titre,
            motif: "aucune sortie d'affichage ne peut servir cette fenêtre".into(),
        });
        return table.enfant_mort(&session);
    };

    // 🔴 **THE BRANCH TAKEN, NAMED — not only the nominal case.** This repository
    // has just paid (batch 33) for a trace placed AFTER a short circuit, which
    // made a `0` in the log indistinguishable between "the message never
    // arrives" and "it arrives and changes nothing". Here the output is kept
    // through TWO paths nothing else distinguishes in the log: it was
    // large enough, or it is OURS and was exempted from the size
    // criterion (August 31st, 2026). Without this field, a served window does not say
    // which one served it, and the exemption would be unverifiable in production.
    let exemptee = designee.as_deref() == Some(cible.nom_sortie.as_str())
        && !placement::sortie_assez_grande(
            (cible.rect.width, cible.rect.height),
            (largeur, hauteur),
        );
    tracing::info!(
        session = %session.0,
        sortie = %cible.nom_sortie,
        demande = format!("{largeur}x{hauteur}"),
        sortie_reelle = format!("{}x{}", cible.rect.width, cible.rect.height),
        exemptee,
        "sortie retenue pour cette fenetre"
    );

    // The output can be much larger than the window: it is the nominal
    // case on a VM whose registry was polluted. The window is put at
    // THIS size, at the output's origin, and the capture crops the same
    // rectangle in the duplication of THIS output — never in the desktop's,
    // hence the absence of the cross-session leak risk that
    // `ModeCapture::FenetreRecadree` carries (see the header of
    // `windows_source/sortie.rs`).
    // 🔴 **BOUNDED BY THE WORK AREA SINCE BATCH 33, NO LONGER BY THE
    // OUTPUT'S RECTANGLE.** A 48 px SECONDARY taskbar is
    // stuck at the bottom of each served output (Windows default, measured in
    // session 1 on August 31st, 2026): putting the window at the monitor's rectangle
    // made the bar cover it — 48 px of application content lost —
    // and put those 48 rows into the crop. `taille_pour_viewport`
    // composes the same `borner_a_la_taille_max` and the same `taille_retenue`
    // as before: only the BOUND changes.
    let retenue = crate::windows_source_sortie::taille_pour_viewport(
        (largeur, hauteur),
        placement_periodique::borne_de(&cible),
    );

    let nom = cible.nom_sortie.clone();
    prises.push(nom.clone());
    // The TWO identifiers: the driver's for destruction, the DXGI
    // name for capture. No computable relation between them. The table
    // keeps the RETAINED size, not the output's: it is the one that
    // then travels to the capturer (tasks 8 and 9), and that the periodic
    // placement check rereads without recomputing it.
    let suite = table.sortie_creee(&session, id_pilote, nom, retenue);

    // A table that has nothing to say about this output keeps it nowhere:
    // `id_pilote` would no longer be known to anyone (neither the table, nor a
    // coming effect), one place lost out of ten, and the name would stay stuck
    // in `prises` forever. The case is not reachable with the loop's current
    // ordering — but that ordering is declared load-bearing
    // nowhere, and it will be enough for a step to slip in one day.
    if suite.is_empty() {
        tracing::error!(
            session = %session.0, id_pilote,
            "la table n'attendait plus cette sortie — elle est rendue au pilote"
        );
        rendre_sans_apparier(sorties, id_pilote);
        prises.retain(|p| *p != cible.nom_sortie);
        return Vec::new();
    }

    // Put the window on it before the child captures. We read the window
    // in the effects the table HAS JUST returned, and not in the global
    // queue: that one can carry another session's `LancerEnfant`,
    // and we would then place the wrong window.
    //
    // At the output's origin, but at the RETAINED size — not at `cible.rect`,
    // which can be much larger (polluted registry, see above). Putting it
    // at the output's size would cover more than what the capture crops.
    if let Some(Effet::LancerEnfant { fenetre, .. }) = suite.first() {
        let hwnd = windows::Win32::Foundation::HWND(fenetre.0 as *mut core::ffi::c_void);
        let rect = Rect {
            x: cible.rect.x,
            y: cible.rect.y,
            width: retenue.0,
            height: retenue.1,
        };
        if let Err(erreur) = placement::poser(hwnd, &rect) {
            tracing::warn!(session = %session.0, %erreur, "placement de la fenêtre échoué");
        }
    }
    suite
}

/// Hands back to the driver an output that was never paired with a session.
///
/// Nothing to remove from `prises`: by construction, none of these paths
/// registered anything there — or the caller takes care of it.
fn rendre_sans_apparier(sorties: &mut Sorties<'_>, id_pilote: u32) {
    if let Err(erreur) = sorties.detruire(id_pilote) {
        tracing::error!(
            id_pilote, %erreur,
            "sortie orpheline NON rendue — la garde la retentera à l'arrêt"
        );
    }
}

/// Waits for OUR output to be there, without ceasing to beat the watchdog.
///
/// 🔴 **"OUR", and not "a new output" — that is the whole of batch 32.** The
/// function was called `attendre_une_sortie_neuve`, and its predicate
/// (`!avant.contains(…)`) was the defect measured by batch 30: an output
/// that REPLACES a forced target inherits the previous name and is therefore never
/// "new". See the header of `superviseur::designation`.
///
/// Returns the DESIGNATED name (empty if designation returned nothing) and the
/// candidates. The first only serves the caller's log, where it decides between
/// two families of causes that would otherwise be indistinguishable.
///
/// The heartbeat is not a detail: the driver removes the outputs of a client
/// that stops pinging, **including those just created**, and the loop's
/// ping step is outside the effects run.
///
/// **`enumerer_sorties_silencieux`, never `relever_topologie`, IN THE
/// POLLING.** At 10 Hz, `relever_topologie` would log one line per
/// existing DXGI output at each turn — the repository has already paid twice for
/// a trace emitted at a loop's cadence (TURN work item, fix I2 of
/// D1). The named and logged survey is still done once before the call
/// (`creer_sortie`), and — since this function was reread — once
/// more ONLY if the wait expires, right before returning the empty vector.
///
/// **This expiry survey is not cosmetic.** Without it, a failure only
/// leaves in the log the survey from BEFORE creation (which by
/// construction cannot show the new output) and the caller's "candidates"
/// log, which only lists outputs already filtered `attachee_au_
/// bureau && nouvelles` — empty by construction if the output was never
/// attached. Two distinct failures were then confused under the same
/// log: "the output appeared but Windows never composed anything on it"
/// (the refusal the multi-window probe already names) versus "it never
/// appeared at all". The complete and named survey — all outputs,
/// attached and unattached — decides between the two, and costs nothing in
/// normal operation: it only runs on the failure path.
fn attendre_notre_sortie(
    pilote: &PiloteParIoctl,
    id_pilote: crate::moniteurs_virtuels::IdSortie,
    avant: &[String],
    limite: std::time::Duration,
) -> (Option<String>, Vec<SortieDxgi>) {
    // Reread ONCE: the pair does not move during the wait, and a
    // round trip under the driver's lock has nothing to do in a loop
    // at 10 Hz. `None` for a driver that does not know this identifier —
    // the caller then falls back on the fallback path, never on a guess.
    let adaptateur = pilote.adaptateur_de(id_pilote);
    // 🔴 THE RETRY HAPPENS ON THE SAME OUTPUT, NEVER ON A NEW ONE. Destroying
    // then recreating would change the topology, hence would re-trigger Apollo's
    // encoder probe — the retry would feed what it waits for — and
    // would consume the pool of ten (the red arm recorded 9 outputs created
    // for 7 refusals). See `superviseur::reprise`, which carries the rule and its
    // host tests.
    let mut tour: u32 = 1;
    let mut echeance = std::time::Instant::now() + limite;
    loop {
        if let Err(erreur) = pilote.pinguer() {
            tracing::warn!(%erreur, "ping du chien de garde pendant l'attente de rattachement");
        }
        let toutes = enumerer_sorties_silencieux().unwrap_or_default();

        // ① DESIGNATE — by what we GAVE the driver, not by what changed
        // around. `chemins_actifs` is SILENT, and it must be: we are
        // in a 10 Hz loop, and this repository paid twice for a trace
        // emitted at a loop's cadence.
        let designee = adaptateur
            .filter(|_| designation::armee())
            .and_then(|adaptateur| {
                let chemins = config_affichage::chemins_actifs().ok()?;
                config_affichage::nom_gdi_de_la_cible(&chemins, adaptateur, id_pilote)
                    .map(str::to_owned)
            });

        // ② The FALLBACK lives in `designation::candidates`, with its host
        // tests — the loop only passes it what it recorded. It is
        // what makes the rule exercisable without Windows: `creation_sortie` is
        // `#[cfg(windows)]` end to end.
        let candidates = designation::candidates(&toutes, designee.as_deref(), avant);
        if !candidates.is_empty() {
            // 🔴 THE PATH TRACE, AND IT IS NOT COSMETIC. Without it,
            // a served window does not say BY WHICH PATH it was served, and
            // a green obtained by the fallback — because Windows did not
            // fabricate a forced target that day — would be indistinguishable
            // from a green obtained by designation. The log's `designee`
            // only appeared on REFUSAL, hence never when everything
            // goes well: success was mute about its own cause.
            //
            // Emitted ONCE per creation (the loop returns control here), and
            // not at the polling cadence.
            match designee.as_deref() {
                Some(nom) => tracing::info!(
                    id_pilote,
                    ?adaptateur,
                    nom_designe = nom,
                    "sortie DESIGNEE par son identifiant de cible (chemin ① — \
                     la correspondance CCD a rendu son nom GDI)"
                ),
                None => tracing::info!(
                    id_pilote,
                    "sortie retenue par DIFFERENCE D'ENSEMBLES (chemin ② de repli — \
                     la designation n'a rien rendu)"
                ),
            }
            return (designee, candidates);
        }
        if std::time::Instant::now() >= echeance {
            // The turn has elapsed. The rule — bounded, tested on the host — says
            // whether one remains.
            if let reprise::Suite::Reessayer {
                tour_suivant,
                apres,
            } = reprise::apres_un_tour(tour, reprise::TOURS, reprise::REPIT)
            {
                // ⚠️ `warn!` and not `error!`: it is not a refusal yet.
                // A lost turn and an abandonment must not read the same.
                tracing::warn!(
                    id_pilote,
                    tour,
                    tours = reprise::TOURS,
                    limite_ms = limite.as_millis() as u64,
                    repit_ms = apres.as_millis() as u64,
                    "la sortie ne s'est pas attachée dans ce tour — on RÉESSAIE \
                     sur la MÊME sortie (l'attachement est intermittent, pas lent)"
                );
                std::thread::sleep(apres);
                tour = tour_suivant;
                echeance = std::time::Instant::now() + limite;
                continue;
            }
            tracing::error!(
                tours_epuises = tour,
                limite_ms = limite.as_millis() as u64,
                "aucune sortie neuve n'est apparue — TOUS LES TOURS DE REPRISE \
                 SONT ÉPUISÉS"
            );
            // Complete, named survey, ONCE — on this failure path only.
            // It is here, and only here, that this diagnosis is worth anything: see the
            // function's doc.
            if let Err(erreur) = relever_topologie("attente de rattachement expirée") {
                tracing::error!(%erreur, "topologie DXGI illisible au moment de l'expiration");
            }
            // The complete survey above does not say WHY designation
            // went quiet. This line says it, once, on this failure path
            // only: without it, "CCD never named our target" and
            // "CCD named it but DXGI does not enumerate it" would be confused.
            match adaptateur {
                None => tracing::error!(
                    id_pilote,
                    "le pilote ne connaît pas l'adaptateur de cette sortie — la désignation n'a pas pu être tentée, seul le repli a couru"
                ),
                Some(adaptateur) => tracing::error!(
                    id_pilote,
                    ?adaptateur,
                    "la cible n'a jamais été nommée par la configuration d'affichage dans la limite — voir moniteurs_virtuels::config_affichage"
                ),
            }
            return (None, Vec::new());
        }
        std::thread::sleep(PAS_RATTACHEMENT);
    }
}

/// Hands an output back to the driver and frees its DXGI place.
pub(super) fn rendre_la_sortie(
    sorties: &mut Sorties<'_>,
    prises: &mut Vec<String>,
    sortie_pilote: u32,
    nom_sortie: String,
) {
    match sorties.detruire(sortie_pilote) {
        Ok(()) => {
            tracing::info!(sortie_pilote, "sortie virtuelle rendue au pilote");
            prises.retain(|p| *p != nom_sortie);
        }
        // The DXGI place stays RESERVED on failure, and that is the fundamental point.
        //
        // A destruction refusal very probably means the output
        // still exists — and therefore stays attached to the desktop. Freeing
        // its place would make it a candidate again: a later window of the
        // same dimensions could be put on it while the table
        // kept the `id_pilote` of the NEW output, which would never serve
        // and would wrongly be destroyed at closing — the old one staying
        // orphaned. Keeping the place reserved costs at worst one DXGI place
        // until shutdown; freeing it costs an identity confusion.
        Err(erreur) => tracing::error!(
            sortie_pilote, %nom_sortie, %erreur,
            "sortie virtuelle NON rendue — la garde la retentera à l'arrêt, \
             et sa place DXGI reste réservée d'ici là"
        ),
    }
}
