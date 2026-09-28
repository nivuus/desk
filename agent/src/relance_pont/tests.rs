use super::*;

#[test]
fn fresh_has_nothing_to_relaunch_nor_stabilise() {
    let etat = EtatRelance::neuve();
    assert_eq!(etat.tentative(), 0);
    assert_eq!(etat.espacement_ms(), ESPACEMENT_PLANCHER_MS);
}

/// `delai_de_repli(0)` EQUALS `ESPACEMENT_PLANCHER_MS` — the property
/// the module doc asserts, exercised rather than believed.
///
/// ⚠️ **IT IS ALSO THE WELD** that prevents raising
/// `ESPACEMENT_PLANCHER_MS` alone: this test requires
/// `REPLI_MIN_MS == ESPACEMENT_PLANCHER_MS`, and the constant's doc now
/// says so.
#[test]
fn the_first_spacing_equals_the_floor() {
    assert_eq!(delai_de_repli(0), ESPACEMENT_PLANCHER_MS);
}

/// 🔴 THE TEST THAT ANCHORS ROUND 2'S FIX: `SEUIL_STABILITE_MS`
/// must stay STRICTLY ABOVE `REPLI_MAX_MS`, otherwise the
/// property this module exists to guarantee falls the day one
/// of the two drifts without the other following — same pattern as
/// `frein.test.ts::FENETRE_REQUETES_MS_reste_plus_courte_que_FENETRE_MS`.
///
/// ⚠️ **ITS TWIN WAS REMOVED IN ROUND 4, AND IT IS SAID RATHER THAN PASSED OVER IN SILENCE**:
/// `the_floor_stays_strictly_below_the_stability_threshold` fixed
/// `ESPACEMENT_PLANCHER_MS < SEUIL_STABILITE_MS` to guarantee the ORDER in
/// which `surveiller` called `reset_the_backoff` then `stable`. That
/// order no longer exists: both methods now live in
/// DIFFERENT BRANCHES of `surveiller` (dead versus alive), and can no longer
/// run at the same turn. Keeping this test would have been keeping a
/// false justification — which this repository pays more dearly for than one test
/// fewer.
#[test]
fn the_stability_threshold_stays_strictly_above_the_backoff_ceiling() {
    const {
        assert!(
            SEUIL_STABILITE_MS > REPLI_MAX_MS,
            "SEUIL_STABILITE_MS is not > REPLI_MAX_MS"
        )
    };
}

#[test]
fn should_relaunch_is_false_right_after_an_attempt() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    assert!(!etat.doit_relancer(0));
    assert!(!etat.doit_relancer(ESPACEMENT_PLANCHER_MS - 1));
}

#[test]
fn should_relaunch_becomes_true_at_the_exact_spacing() {
    let etat = EtatRelance::neuve();
    assert!(etat.doit_relancer(ESPACEMENT_PLANCHER_MS));
}

#[test]
fn only_the_first_launch_of_the_cycle_is_reported() {
    let mut etat = EtatRelance::neuve();
    assert!(etat.tentative_lancee(), "the first launch must be reported");
    assert!(
        !etat.tentative_lancee(),
        "the second, of the SAME cycle, must no longer be"
    );
    assert!(!etat.tentative_lancee(), "nor the third");
    assert_eq!(
        etat.tentative(),
        3,
        "the COUNT, for its part, keeps growing"
    );
}

/// 🔴 **`EtatObserve` HAS NO CONSUMER ON THE HOST** (the only three
/// live in `surveillance_pont.rs` and `lanceur/pont.rs`, both
/// `#[cfg(windows)]` — see the module header doc). Without this test, its
/// re-export AND the enum itself are dead code for `cargo check` on the
/// host side: two warnings, one of them NEW since the extraction — the very class
/// `CLAUDE.md` names for extractions ("it leaves its
/// imports behind it").
#[test]
fn observed_state_tells_dead_from_alive_and_absent_by_its_outcome() {
    let mort_propre = EtatObserve::Mort(IssueDeSortie::Propre);
    let error_death = EtatObserve::Mort(IssueDeSortie::Error);
    assert_ne!(mort_propre, error_death, "l'issue distingue deux morts");
    assert_ne!(EtatObserve::Vivant, EtatObserve::Absent);
    assert_ne!(EtatObserve::Vivant, mort_propre);
}

/// The translation of the exit code, in all three directions — including the one
/// that does NOT run on the target (see the doc of `depuis_le_code`).
#[test]
fn the_exit_code_translates_in_all_three_directions() {
    assert_eq!(
        IssueDeSortie::depuis_le_code(Some(0)),
        IssueDeSortie::Propre
    );
    assert_eq!(IssueDeSortie::depuis_le_code(Some(1)), IssueDeSortie::Error);
    assert_eq!(
        IssueDeSortie::depuis_le_code(Some(101)),
        IssueDeSortie::Error
    );
    assert_eq!(IssueDeSortie::depuis_le_code(None), IssueDeSortie::Inconnue);
}

/// 🔴 **FIX ROUND 4'S RED, "WE RE-ARM" HALF** — and it
/// replaces `une_vie_normale_mais_pas_stable_reinitialise_quand_meme_le_
/// repli`, which **asserted the defect as the desired property**: the
/// defect was sealed in its own check.
///
/// A bridge that was refused five times, then SERVES a session and
/// ends CLEANLY, must start again from the floor — "an agent connected
/// for three days that loses its network for one second must resume in half
/// a second, not in thirty". **And that session's duration enters
/// nowhere**: the test shows it by providing none.
#[test]
fn a_clean_exit_rearms_the_backoff_without_any_duration_involved() {
    let mut etat = EtatRelance::neuve();
    for _ in 0..5 {
        etat.tentative_lancee();
    }
    assert!(
        etat.espacement_ms() > ESPACEMENT_PLANCHER_MS,
        "the backoff did grow before the test"
    );
    etat.reset_the_backoff(IssueDeSortie::Propre);
    assert_eq!(
        etat.espacement_ms(),
        ESPACEMENT_PLANCHER_MS,
        "a CLEAN exit must re-arm the backoff: the past failure is resolved"
    );
}

/// 🔴 **FIX ROUND 4'S RED, "WE DO NOT RE-ARM" HALF —
/// IT IS THE ONE HOLDING THE MEASURED DEFECT.** A REFUSED bridge honours
/// `retryApresS`, stays alive up to `REPLI_MAX_MS`, **then dies in
/// error**: its long life proves nothing, and the fallback must keep
/// growing. Under round 3, the same situation re-armed the fallback ~500 ms
/// after EACH launch, and the cadence rose to 100 connections/minute
/// against a shared budget of 120.
///
/// 🔵 The TWO other outcomes are exercised in the same test, because
/// they share the conclusion: neither `Error` nor `Inconnue` re-arms.
#[test]
fn neither_a_refusal_nor_an_unknown_outcome_re_arms_the_backoff() {
    for issue in [IssueDeSortie::Error, IssueDeSortie::Inconnue] {
        let mut etat = EtatRelance::neuve();
        for _ in 0..5 {
            etat.tentative_lancee();
        }
        let before = etat.espacement_ms();
        assert!(
            before > ESPACEMENT_PLANCHER_MS,
            "the backoff did grow before the test"
        );
        // A VERY long life — longer than the longest possible refusal
        // sleep — and yet no re-arming: duration does not enter
        // the decision, that is the whole of round 4.
        etat.reset_the_backoff(issue);
        assert_eq!(
            etat.espacement_ms(),
            before,
            "{issue:?} must re-arm NOTHING, whatever the lifetime was"
        );
    }
}

/// 🔴 THE EXACT RED OF THE DEFECT FIXED BY ROUND 2, REPLAYED HERE:
/// a REFUSED process staying alive up to `REPLI_MAX_MS` before
/// dying (`honour_suggested_retry`'s sleep) must NEVER be
/// declared stable during that sleep. Probed at regular intervals,
/// as the supervisor loop would at ~10 Hz. **It is THE red
/// of the "trace loop"**, which rounds 3 and 4 had to preserve:
/// it keeps holding because `stable` never changed its
/// guard condition.
#[test]
fn stable_during_a_refusal_sleep_never_declares_stable() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    let mut ecoule = 0u64;
    while ecoule < REPLI_MAX_MS {
        assert!(
            !etat.stable(ecoule),
            "declared stable at {ecoule} ms, while the refusal sleep \
             can last up to {REPLI_MAX_MS} ms — this is the trace \
             loop of correction round 2"
        );
        ecoule += 97; // a non-round step, so as not to land exactly on a case
    }
}

/// 🔵 POSITIVE WITNESS: a REALLY stable process — alive well
/// beyond the longest possible refusal sleep — is indeed declared
/// stable, and only once.
///
/// 🔴 **WHAT IT HELD IN ROUND 4 BECAME UNREACHABLE IN ROUND 5, AND
/// IT IS NOT A COVERAGE HOLE — IT IS A BETTER GUARANTEE.** The
/// previous version targeted the state `cycle_signale == false && tentative >
/// 0` (a bridge declared stable then ended cleanly, round 3's guard
/// on `reset_the_backoff` then blocking the re-arming) by calling
/// `reset_the_backoff(Propre)` AFTER `stable`. Since `stable()`
/// ITSELF resets `tentative` to zero in its true branch (see its doc),
/// that call could no longer prove anything: `tentative` was already zero
/// BEFORE it, whatever `reset_the_backoff` did — proven by
/// mutation. The real guarantee is the INVARIANT that makes this state
/// unreachable, held directly below rather than deduced: `stable()
/// == true` resets `tentative` to zero IN THE SAME GESTURE that makes
/// `cycle_signale` fall back, so there is no longer an instant where `cycle_signale ==
/// false` and `tentative > 0` at the same time.
#[test]
fn a_really_stable_process_ends_up_declared_stable_once() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    etat.tentative_lancee(); // le repli a grandi : tentative = 2
    assert!(!etat.stable(SEUIL_STABILITE_MS - 1));
    assert!(etat.stable(SEUIL_STABILITE_MS));
    assert_eq!(
        etat.tentative(),
        0,
        "the invariant that replaces the guarantee targeted in round 4: `stable() \
         == true` resets `tentative` to zero in the SAME move that makes \
         `cycle_signale` drop — the state this test targeted before \
         (cycle dropped, tentative still positive) no longer exists"
    );
    // A second call, cycle already fallen back: nothing more to signal as long
    // as no new attempt has taken place.
    assert!(!etat.stable(SEUIL_STABILITE_MS * 2));
}

/// Without a cycle in progress (no attempt launched since the last
/// stability), `stable` must declare nothing, whatever `ecoule_ms`
/// — a bridge never restarted has nothing to "become" stable "again".
#[test]
fn without_a_cycle_in_progress_stable_never_declares_anything() {
    let mut etat = EtatRelance::neuve();
    assert!(!etat.stable(SEUIL_STABILITE_MS * 10));
}

/// 🔴 THE SIMULATION OF SEVERAL REFUSAL CYCLES — the test that counts the
/// LINES THAT WOULD BE EMITTED, not the calls: it is the form the
/// review asked for in round 2. Each cycle: one attempt launched, a
/// refusal sleep up to `REPLI_MAX_MS` probed at ~10 Hz, then death IN
/// ERROR — the outcome `bail!` produces — which `surveiller` observes once
/// before restarting.
///
/// 🔵 **WHAT THE FIRST VERSION OF THIS TEST WRONGLY BELIEVED** (turned red
/// before the fix, and that is the proof the property was not
/// assumed): "one "launched" line per cycle". It is WRONG, and it is even
/// BETTER than that — because `stable()` NEVER re-arms `cycle_signale`
/// as long as no cycle reaches REAL stability, `tentative_lancee()`
/// returns `premier_du_cycle = false` for EVERY restart after the very
/// first of the whole episode. The "launched" line therefore only comes out **ONCE
/// FOR THE WHOLE hammering episode**, not once per
/// cycle — exactly the module's header promise: "signal the
/// first time, keep quiet as long as the situation repeats".
/// **Round 4 changes nothing to this property**: `reset_the_
/// backoff` never touches `cycle_signale`, and the `Error` outcome does not even
/// re-arm `tentative`.
#[test]
fn over_several_refusal_cycles_a_single_launched_line_and_no_stable_line() {
    const PAS_MS: u64 = 100; // ~10 Hz, the loop's real cadence
    let mut etat = EtatRelance::neuve();
    let mut lignes_lancees = 0u32;
    let mut lignes_stables = 0u32;
    for _cycle in 0..5 {
        if etat.tentative_lancee() {
            lignes_lancees += 1;
        }
        let mut ecoule = 0u64;
        while ecoule < REPLI_MAX_MS {
            if etat.stable(ecoule) {
                lignes_stables += 1;
            }
            ecoule += PAS_MS;
        }
        // The process dies here, IN ERROR (end of the refusal sleep, then
        // `pont::executer`'s `bail!`): it is what `surveiller`
        // observes, once only, before restarting.
        etat.reset_the_backoff(IssueDeSortie::Error);
    }
    assert_eq!(
        lignes_lancees, 1,
        "ONE SINGLE « launched » line for the whole episode — never one per \
         cycle: this is the silence property the module already promises, \
         and that the single-threshold bug broke by re-arming `cycle_signale` \
         at every false stability"
    );
    assert_eq!(
        lignes_stables, 0,
        "NO « stable » line must come out as long as no cycle has \
         really held: this is exactly the loop that correction round 2 \
         closes, and that neither round 3 nor round 4 reopen"
    );
    assert!(
        etat.espacement_ms() > REPLI_MAX_MS / 2,
        "after five refusals, the spacing must have GROWN — this is the \
         round 3 defect, where it fell back to the floor at every cycle"
    );
}

/// 🔵 WITNESS: if a cycle ends up REALLY holding (the bridge stops
/// being refused), the "stable" line comes out ONCE, and the NEXT cycle
/// becomes noisy again on its first launch — the half of the mechanism
/// the test above, on its own, cannot prove since none of
/// its cycles ever reaches stability.
#[test]
fn after_real_stability_the_next_cycle_becomes_noisy_again() {
    let mut etat = EtatRelance::neuve();
    assert!(
        etat.tentative_lancee(),
        "first launch of the episode: noisy"
    );
    assert!(
        etat.stable(SEUIL_STABILITE_MS),
        "really stayed alive long enough"
    );
    assert!(
        etat.tentative_lancee(),
        "a NEW cycle, after real stability, becomes noisy again"
    );
}

/// 🔴 **THE JUDGE TEST OF THE WHOLE BATCH: THE `/signal` CONNECTION CADENCE
/// OF A BRIDGE DYING IN A LOOP.** It is the figure the missing-brakes
/// legacy exists to bound — `plateforme/src/securite/frein.ts` gives
/// `REQUETES_MAX_ADRESSE = 120` per 60 s window, **shared** with the
/// supervisor's control session and each window child.
///
/// ⚠️ **THIS TEST REWRITES THE WIRING** (`surveiller` + `tenter`), which lives
/// behind `#[cfg(windows)]` and stays out of reach of
/// `cargo test --workspace`: it exercises the DECISION under a realistic
/// cadence, never the real wiring. It is said rather than assumed.
///
/// 🔵 **MEASURED, NOT ASSUMED**: under round 3, this same simulation
/// returned **100** launches for `VIE_MS = 600` (and 60 for 1 s, 30 for
/// 2 s, 12 for 5 s) — 83% of the shared budget consumed by the bridge alone,
/// **without any `retryApresS` having to step in**.
#[test]
fn a_bridge_dying_in_error_after_half_a_second_does_not_hammer() {
    const PAS_MS: u64 = 100;
    const DUREE_MS: u64 = 60_000;
    const VIE_MS: u64 = 600;
    // 🔴 THE MEASURED VALUE, NOT A LOOSE BOUND. An earlier draft
    // asserted `<= 120 / 10`, i.e. 12, where the measurement returns 6: **a
    // regression that DOUBLED the cadence would have passed** (pointed out by the
    // review of fix round 5). Exact equality forces REMEASURING
    // the day `REPLI_MIN_MS` or `REPLI_MAX_MS` move, which is
    // exactly what one wants from a judge figure.
    const LANCEMENTS_MESURES: u32 = 6;
    // The platform's SHARED budget, for reading the message.
    const BUDGET_PARTAGE_PAR_MINUTE: u32 = 120;

    let mut relance = EtatRelance::neuve();
    let mut derniere_tentative_ms: i64 = -(ESPACEMENT_PLANCHER_MS as i64);
    let mut lance_a: Option<i64> = None;
    let mut lancements = 0u32;
    let mut horloge: i64 = 0;
    while horloge < DUREE_MS as i64 {
        let vivant = matches!(lance_a, Some(t) if horloge - t < VIE_MS as i64);
        let ecoule_ms = (horloge - derniere_tentative_ms) as u64;
        if vivant {
            let _ = relance.stable(ecoule_ms);
        } else {
            if lance_a.take().is_some() {
                // The death is observed ONCE: `bail!` after the refusal.
                relance.reset_the_backoff(IssueDeSortie::Error);
            }
            if relance.doit_relancer(ecoule_ms) {
                derniere_tentative_ms = horloge;
                relance.tentative_lancee();
                lance_a = Some(horloge);
                lancements += 1;
            }
        }
        horloge += PAS_MS as i64;
    }
    assert_eq!(
        lancements, LANCEMENTS_MESURES,
        "{lancements} /signal connections in one minute for the bridge ALONE, \
         against a SHARED budget of {BUDGET_PARTAGE_PAR_MINUTE} and a \
         measurement of {LANCEMENTS_MESURES}: this is the VM lockout that \
         this batch exists to close"
    );
}

/// 🔴 **FIX ROUND 5'S RED — "THE QUOTE THE MODULE HAS CARRIED
/// SINCE ROUND 3 WAS WRONG FOR ONE CASE".** A bridge refused six
/// times (spacing at the CEILING), then restarted a seventh time and SERVING
/// FOR THREE DAYS, then killed by a network outage — hence a death IN ERROR,
/// which no clean exit redeems — must see its
/// `espacement_ms()` FALL BACK to the floor rather than stay at the ceiling
/// of an already resolved failure. ⚠️ **IT IS NOT "resume in half a
/// second, not in thirty": the final review measured, at the loop
/// level, that the FIRST resumption after the outage is immediate in
/// BOTH cases** (`doit_relancer` compares the time elapsed since the LAUNCH, and
/// three days of life exceed any fallback) — **it is the RAMP of the
/// NEXT episode that starts again from the floor**, and this test exercises it at
/// the level of the PURE state, one notch before the loop.
///
/// 🔵 **WHAT MAKES DURATION LEGITIMATE HERE, WHERE IT WAS NOT IN
/// ROUND 3**: the threshold crossed is `SEUIL_STABILITE_MS` (35 s), which a
/// sleeping refused bridge CANNOT reach — its sleep is bounded at
/// `REPLI_MAX_MS` (30 s). The two tests holding this invariant are
/// `the_stability_threshold_stays_strictly_above_the_backoff_ceiling`
/// and `stable_during_a_refusal_sleep_never_declares_stable`.
#[test]
fn a_long_stable_life_then_an_error_death_restarts_at_the_floor() {
    const TROIS_JOURS_MS: u64 = 3 * 24 * 60 * 60 * 1_000;
    let mut etat = EtatRelance::neuve();
    for _ in 0..6 {
        etat.tentative_lancee();
    }
    assert_eq!(
        etat.espacement_ms(),
        REPLI_MAX_MS,
        "six refusals: the backoff is at the CEILING"
    );

    // Seventh launch — this one holds, and for long.
    etat.tentative_lancee();
    assert!(
        etat.stable(TROIS_JOURS_MS),
        "three days of life, that is stable"
    );

    // Then the network outage: `pont::executer` returns an `Err`, hence a non-zero
    // exit code, hence `IssueDeSortie::Error` — which
    // `reset_the_backoff` rightly refuses to count as proof.
    etat.reset_the_backoff(IssueDeSortie::Error);
    assert_eq!(
        etat.espacement_ms(),
        ESPACEMENT_PLANCHER_MS,
        "after three days of service, a network cut must fall back to \
         the FLOOR spacing and not stay at the ceiling of {REPLI_MAX_MS} ms \
         — the RAMP of the next refusal episode, not the first reconnection \
         (immediate in both cases): this is the quote that \
         `reset_the_backoff` carries, and that only `stable` can hold \
         for a death IN ERROR"
    );
    assert!(etat.doit_relancer(ESPACEMENT_PLANCHER_MS));
}
