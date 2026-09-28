//! The SIXTH memorisation site: the pool wrote `eveillee` before
//! the order went out.
//!
//! **Dedicated sibling file rather than tests added to `sommeil/tests.rs`**
//! (fix round 2, 25 August 2026): that suite is at 473 lines
//! for a project cap of 500, and these two tests would have pushed it
//! over. Extract, never compress — and here, avoid growing rather
//! than having to extract afterwards. Same idiom and same precedent as
//! `superviseur/table.rs`, which likewise keeps its restart tests
//! in a second file.
//!
//! 🔴 WHAT THESE TWO TESTS HOLD, AND NOTHING ELSE HOLDS.
//! `Vivier::arbitrer` writes `eveillee` AT STEPS 1 AND 5, that is **before
//! the corresponding order has been dropped** into its window's queue. If
//! that drop is REFUSED (queue full), the pool lies about the real state — and
//! `arbitrer` being idempotent, **no future re-arbitration re-emits the
//! order**. The remedy is `Vivier::annuler_ordre_non_livre`, called by
//! `registre::distribuer`: see its doc for what each of the two directions
//! costs.
//!
//! ⚠️ **BOTH DIRECTIONS ARE TESTED, AND THAT IS THE POINT**: the first draft of the
//! diagnosis only named `Reveiller` ("a window that no longer falls
//! asleep"). `Dormir` is the other half, and it is **the costlier** — the
//! pool frees the place while the window is still encoding, so
//! ~~the cap of eight encoders is over-subscribed~~. **OVER-CLAIMED HERE
//! TOO, AND LEFT UNSTRUCK WHILE THE SAME FILE CORRECTS IT FURTHER DOWN**
//! (around line 112, and `vivier.rs` rewrites it in turn): the
//! over-subscription does happen, but that is not what the remedy prevents
//! — it makes it TRANSIENT instead of permanent. There is an
//! `echec_de_reveil` for the first direction; there is **no**
//! `echec_de_sommeil`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

use super::file::PROFONDEUR_MAX;
use super::tests::verrouiller_pour_le_test;
use super::{distribuer, etat, inscrire, retirer, signaler, Message};
use crate::capteur::vivier::Ordre;

/// Clogs a session's queue with NON-COALESCABLE messages — `Sommeil`
/// never coalesces, that is what makes it possible to reach the bound.
fn boucher_la_file(session: &str) {
    let garde = etat();
    let emetteur = garde.canaux.get(session).expect("la session est inscrite");
    for _ in 0..PROFONDEUR_MAX {
        let _ = emetteur.envoyer(Message::Sommeil(Ordre::Reveiller));
    }
}

/// One wheel round, without waiting the 250 ms it takes in production.
fn un_tour_de_roue() {
    let mut garde = etat();
    let ordres = garde.vivier.rearbitrer(Instant::now());
    distribuer(&mut garde, ordres);
}

/// 🔴 DIRECTION 1 — A REFUSED `Reveiller` MUST NOT LEAVE THE POOL BELIEVING THE
/// WINDOW AWAKE.
///
/// Without the remedy, `eveillee` stays `true` for a window that never
/// received the order: its place in the pool is occupied without any real encoder
/// occupying it, and ten `rearbitrer` in a row re-emit nothing.
///
/// **Turns red on its FIRST assertion** — `un Reveiller non déposé ne doit pas
/// laisser le vivier croire la fenêtre éveillée` —, `eveillee` then being
/// `Some(true)`.
#[test]
fn un_reveil_non_depose_laisse_le_vivier_intact_et_repart_au_tour_suivant() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("r2-reveil", 6500);
    // Precondition: asleep, and the pool knows it.
    assert_eq!(etat().vivier.eveillee("r2-reveil"), Some(false));
    boucher_la_file("r2-reveil");

    // The window becomes visible and focused: the pool elects it and emits
    // `Reveiller` — which is REFUSED, its queue being full.
    signaler("r2-reveil", true, true);

    assert_eq!(
        etat().vivier.eveillee("r2-reveil"),
        Some(false),
        "un Reveiller non déposé ne doit pas laisser le vivier croire la fenêtre éveillée"
    );

    // The window resumes reading, and the next wheel round must re-emit
    // the order by itself — that is the whole point of not having lied.
    let recus = canal.vider();
    assert_eq!(
        recus.len(),
        PROFONDEUR_MAX,
        "précondition : la file était bien pleine"
    );
    un_tour_de_roue();
    let ordres: Vec<Ordre> = canal
        .vider()
        .into_iter()
        .filter_map(|m| match m {
            Message::Sommeil(o) => Some(o),
            _ => None,
        })
        .collect();
    assert!(
        ordres.contains(&Ordre::Reveiller),
        "le Reveiller non déposé doit repartir au tour suivant : {ordres:?}"
    );

    retirer("r2-reveil", generation);
}

/// 🔴 DIRECTION 2 — A REFUSED `Dormir` MUST NOT LEAVE THE POOL COUNTING
/// AS ASLEEP A WINDOW THAT IS STILL ENCODING. **It is the costlier
/// half**, and the one the first diagnosis had missed: without the
/// remedy, `eveillee` goes to `false` for good — the window never
/// received the order, still holds its encoder, and **no arbitration will
/// ever order it again**, since the pool believes it already asleep.
///
/// ⚠️ ~~The cap of eight encoders is over-subscribed.~~ **THAT IS NOT WHAT
/// THE REMEDY PREVENTS, and writing it that way was over-claimed** (round 3):
/// `arbitrer` frees the slot and elects the replacement IN THE SAME PASS,
/// steps 1, 4 and 5, **before the drop is even attempted** —
/// the cancellation only runs afterwards. The over-subscription does happen; what
/// the remedy achieves is that it is **TRANSIENT** instead of
/// permanent. See
/// `une_sur_souscription_par_un_dormir_non_depose_est_resorbee_au_tour_suivant`,
/// just below, which measures it at both times.
///
/// **Turns red on its FIRST assertion** — `un Dormir non déposé ne doit pas
/// laisser le vivier compter endormie une fenêtre qui encode encore` —,
/// `eveillee` then being `Some(false)`.
#[test]
fn un_sommeil_non_depose_laisse_le_vivier_intact_et_repart_au_tour_suivant() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("r2-sommeil", 6501);
    // It wakes up for good, queue free: the order is delivered.
    signaler("r2-sommeil", true, true);
    assert_eq!(
        etat().vivier.eveillee("r2-sommeil"),
        Some(true),
        "précondition : la fenêtre est bien éveillée"
    );
    let _ = canal.vider();
    boucher_la_file("r2-sommeil");

    // It becomes invisible: the pool emits `Dormir(Masquee)` — REFUSED.
    signaler("r2-sommeil", false, false);

    assert_eq!(
        etat().vivier.eveillee("r2-sommeil"),
        Some(true),
        "un Dormir non déposé ne doit pas laisser le vivier compter endormie une \
         fenêtre qui encode encore"
    );

    let recus = canal.vider();
    assert_eq!(
        recus.len(),
        PROFONDEUR_MAX,
        "précondition : la file était bien pleine"
    );
    un_tour_de_roue();
    let ordres: Vec<Ordre> = canal
        .vider()
        .into_iter()
        .filter_map(|m| match m {
            Message::Sommeil(o) => Some(o),
            _ => None,
        })
        .collect();
    assert!(
        ordres.iter().any(|o| matches!(o, Ordre::Dormir(_))),
        "le Dormir non déposé doit repartir au tour suivant : {ordres:?}"
    );

    retirer("r2-sommeil", generation);
}

/// Counts the `tracing` events emitted by `sommeil::registre` on this thread.
///
/// 🔴 **IT IS WHAT MAKES THE PACING MEASURABLE RATHER THAN ASSERTED.** Without it,
/// we could only test the arithmetic predicate — true whatever
/// the code using it, hence a check unable to fail.
///
/// The filter is the `target`, which `tracing` fills with the path of the emitting
/// module: only the lines of `registre.rs` are counted, never
/// those of `file.rs` that come out at the same step.
struct CompteurDeTraces(Arc<AtomicUsize>);

impl<S: tracing::Subscriber> Layer<S> for CompteurDeTraces {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() == "agent::capteur::sommeil::registre" {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// The CUMULATIVE count of a session's refused drops.
fn refus_de(session: &str) -> u64 {
    etat()
        .canaux
        .get(session)
        .expect("la session est inscrite")
        .refuses()
}

/// 🔴 THE DEFECT THE ROUND 2 REMEDY CREATED ITSELF, AND ITS PACING.
///
/// As long as the order was LOST, `distribuer` only emitted an order on an
/// arbitration CHANGE — never at every round. That was the reason written for
/// not pacing its trace, and **it was true**.
///
/// 🔴 **THE CANCELLATION REVERSED IT.** The state now being GIVEN BACK, the
/// next re-arbitration sees the same divergence again, re-emits the same order, and it
/// is refused again: **+1 per round, strictly, without bound**. At
/// `PERIODE_REARBITRAGE` (250 ms), that would make **four lines per second and
/// per blocked window, indefinitely** — the trap `file.rs` avoids ten
/// lines further on, and which `CLAUDE.md` names (18,619 lines in a few
/// seconds have already prevented a session from establishing).
///
/// 🔴 **THIS TEST COUNTS THE TRACES ACTUALLY EMITTED**, through a `tracing` subscriber
/// set on this thread — and not the arithmetic predicate of the pacing. A first
/// draft did that, and that assertion was **structurally unable
/// to fail**: it would have been true whatever the code of `distribuer`.
///
/// **Turns red on its SECOND assertion, ON BOTH SIDES** — and it is an
/// equality (`assert_eq!`), not an upper bound: a `<= 4` bound does not
/// denounce a REMOVED trace (`0 <= 4` is true). Without the trace
/// (`if refuses.is_power_of_two()` → `if false` in `registre.rs`):
/// `0` instead of `3`. Without the pacing (→ `if true`): `10` traces for ten
/// rounds, instead of the `3` steps actually crossed (2, 4, 8).
#[test]
fn un_ordre_refuse_a_chaque_tour_est_trace_a_cadence_logarithmique() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("r3-cadence", 6600);
    signaler("r3-cadence", true, true);
    let _ = canal.vider();
    boucher_la_file("r3-cadence");
    // Hiding generates a `Dormir` — refused, and cancelled.
    signaler("r3-cadence", false, false);
    let depart = refus_de("r3-cadence");

    const TOURS: u64 = 10;
    let compte = Arc::new(AtomicUsize::new(0));
    let abonne = tracing_subscriber::registry().with(CompteurDeTraces(Arc::clone(&compte)));
    let comptes: Vec<u64> = tracing::subscriber::with_default(abonne, || {
        (0..TOURS)
            .map(|_| {
                un_tour_de_roue();
                refus_de("r3-cadence")
            })
            .collect()
    });

    // 🔴 THE PHENOMENON: every wheel round re-emits the order, and every
    // re-emission is refused. That is what round 2's cancellation achieves —
    // and it is what makes an unpaced trace unbounded.
    let attendus: Vec<u64> = (1..=TOURS).map(|i| depart + i).collect();
    assert_eq!(comptes, attendus, "l'ordre doit être réémis à CHAQUE tour");

    // 🔴 THE PACING, MEASURED ON THE LINES EMITTED AND MATCHED EXACTLY, NOT MERELY
    // BOUNDED FROM ABOVE.
    //
    // ⚠️ **A `<= 4` BOUND DOES NOT TURN RED IF THE TRACE DISAPPEARS ENTIRELY**
    // — measured: `if refuses.is_power_of_two()` replaced by `if false` in
    // `registre.rs` (the trace removed) leaves `cargo test --workspace`
    // entirely GREEN, `0 <= 4` being true. What this test must hold is
    // not only "not too many lines", but "exactly the expected
    // lines".
    //
    // Over the ten rounds of this loop, starting from a non-zero CUMULATIVE
    // refusal count (`depart`), the power-of-two steps actually
    // crossed are **2, 4 and 8** — three lines, no more, no less.
    // `assert_eq!` therefore turns red on BOTH sides: the removed trace (`0`,
    // instead of `3`) AND the unpaced trace (`10`, instead of `3`).
    let traces = compte.load(Ordering::Relaxed);
    assert_eq!(
        traces, 3,
        "la trace de l'ordre non déposé doit être CADENCÉE aux puissances de \
         deux, ni supprimée ni émise à chaque refus : {traces} lignes pour \
         {TOURS} tours, 3 attendues (paliers 2, 4, 8)"
    );

    retirer("r3-cadence", generation);
}

/// 🔴 WHAT THE REMEDY REALLY ACHIEVES, MEASURED AT BOTH TIMES — and what
/// it does NOT achieve.
///
/// ⚠️ **The two tests above cannot see this defect**: they only have
/// ONE session, so they measure `eveillee(s)` and never
/// `eveillees().len()`. It is this gap that let through the claim
/// "the place is not freed", measured false in round 3.
///
/// **The mechanism**: `arbitrer` sets `eveillee = false` at step 1, hence
/// excludes the session from the pinned ones at step 2, fills the freed slot at
/// step 4, and emits the ninth's `Reveiller` at step 5 — **all in
/// the same pass, before the drop of the `Dormir` is even attempted**.
/// The cancellation only runs afterwards: it cannot prevent it.
///
/// **What is true, and what this test holds**: the over-subscription is
/// TRANSIENT. At the next re-arbitration, the pool sees 9 > 8 and puts
/// someone back to sleep. Without the remedy, it would never see 9 — it would count 8 while
/// believing the blocked window asleep, and the drift would be PERMANENT.
#[test]
fn une_sur_souscription_par_un_dormir_non_depose_est_resorbee_au_tour_suivant() {
    let _verrou = verrouiller_pour_le_test();
    let plafond = crate::capteur::vivier::PLAFOND_EVEIL;

    // Saturates the places, keeping the receivers alive.
    let mut occupantes = Vec::new();
    for i in 0..plafond {
        let nom = format!("r3-plein-{i}");
        let (canal, generation) = inscrire(&nom, 6700 + i as u32);
        signaler(&nom, true, true);
        occupantes.push((nom, canal, generation));
    }
    assert_eq!(
        etat().vivier.eveillees().len(),
        plafond,
        "précondition : les places sont toutes prises"
    );

    // One more candidate, waiting for a place to be freed.
    let (attente, generation_attente) = inscrire("r3-attente", 6799);
    signaler("r3-attente", true, true);
    assert_eq!(
        etat().vivier.eveillees().len(),
        plafond,
        "précondition : elle attend"
    );

    // The FIRST occupant's queue gets clogged, then it is hidden: its
    // `Dormir` is refused, and cancelled — but `arbitrer` has already elected the ninth
    // in the same pass.
    let (ref nom_bloquee, ref canal_bloquee, _) = occupantes[0];
    let _ = canal_bloquee.vider();
    boucher_la_file(nom_bloquee);
    signaler(nom_bloquee, false, false);

    assert_eq!(
        etat().vivier.eveillees().len(),
        plafond + 1,
        "la sur-souscription a bien lieu : l'annulation ne court qu'APRÈS l'élection"
    );

    // The blocked window resumes reading; the next round absorbs.
    let _ = canal_bloquee.vider();
    un_tour_de_roue();
    assert_eq!(
        etat().vivier.eveillees().len(),
        plafond,
        "la sur-souscription doit être TRANSITOIRE : résorbée au ré-arbitrage suivant"
    );

    retirer("r3-attente", generation_attente);
    drop(attente);
    for (nom, canal, generation) in occupantes {
        retirer(&nom, generation);
        drop(canal);
    }
}
