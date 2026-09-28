use super::*;

#[test]
fn neuve_n_a_rien_a_relancer_ni_rien_a_stabiliser() {
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
fn le_premier_espacement_egale_le_plancher() {
    assert_eq!(delai_de_repli(0), ESPACEMENT_PLANCHER_MS);
}

/// 🔴 THE TEST THAT ANCHORS ROUND 2'S FIX: `SEUIL_STABILITE_MS`
/// must stay STRICTLY ABOVE `REPLI_MAX_MS`, otherwise the
/// property this module exists to guarantee falls the day one
/// of the two drifts without the other following — same pattern as
/// `frein.test.ts::FENETRE_REQUETES_MS_reste_plus_courte_que_FENETRE_MS`.
///
/// ⚠️ **ITS TWIN WAS REMOVED IN ROUND 4, AND IT IS SAID RATHER THAN PASSED OVER IN SILENCE**:
/// `le_plancher_reste_strictement_sous_le_seuil_de_stabilite` fixed
/// `ESPACEMENT_PLANCHER_MS < SEUIL_STABILITE_MS` to guarantee the ORDER in
/// which `surveiller` called `reinitialiser_le_repli` then `stable`. That
/// order no longer exists: both methods now live in
/// DIFFERENT BRANCHES of `surveiller` (dead versus alive), and can no longer
/// run at the same turn. Keeping this test would have been keeping a
/// false justification — which this repository pays more dearly for than one test
/// fewer.
#[test]
fn le_seuil_de_stabilite_reste_strictement_au_dessus_du_plafond_de_repli() {
    const {
        assert!(
            SEUIL_STABILITE_MS > REPLI_MAX_MS,
            "SEUIL_STABILITE_MS n'est pas > REPLI_MAX_MS"
        )
    };
}

#[test]
fn doit_relancer_est_faux_juste_apres_une_tentative() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    assert!(!etat.doit_relancer(0));
    assert!(!etat.doit_relancer(ESPACEMENT_PLANCHER_MS - 1));
}

#[test]
fn doit_relancer_devient_vrai_a_l_espacement_exact() {
    let etat = EtatRelance::neuve();
    assert!(etat.doit_relancer(ESPACEMENT_PLANCHER_MS));
}

#[test]
fn seul_le_premier_lancement_du_cycle_est_signale() {
    let mut etat = EtatRelance::neuve();
    assert!(
        etat.tentative_lancee(),
        "le premier lancement doit être signalé"
    );
    assert!(
        !etat.tentative_lancee(),
        "le second, du MÊME cycle, ne doit plus l'être"
    );
    assert!(!etat.tentative_lancee(), "ni le troisième");
    assert_eq!(etat.tentative(), 3, "le COMPTE, lui, continue de croître");
}

/// 🔴 **`EtatObserve` HAS NO CONSUMER ON THE HOST** (the only three
/// live in `surveillance_pont.rs` and `lanceur/pont.rs`, both
/// `#[cfg(windows)]` — see the module header doc). Without this test, its
/// re-export AND the enum itself are dead code for `cargo check` on the
/// host side: two warnings, one of them NEW since the extraction — the very class
/// `CLAUDE.md` names for extractions ("it leaves its
/// imports behind it").
#[test]
fn etat_observe_distingue_mort_de_vivant_et_d_absent_par_son_issue() {
    let mort_propre = EtatObserve::Mort(IssueDeSortie::Propre);
    let mort_erreur = EtatObserve::Mort(IssueDeSortie::Erreur);
    assert_ne!(mort_propre, mort_erreur, "l'issue distingue deux morts");
    assert_ne!(EtatObserve::Vivant, EtatObserve::Absent);
    assert_ne!(EtatObserve::Vivant, mort_propre);
}

/// The translation of the exit code, in all three directions — including the one
/// that does NOT run on the target (see the doc of `depuis_le_code`).
#[test]
fn le_code_de_sortie_se_traduit_dans_les_trois_sens() {
    assert_eq!(
        IssueDeSortie::depuis_le_code(Some(0)),
        IssueDeSortie::Propre
    );
    assert_eq!(
        IssueDeSortie::depuis_le_code(Some(1)),
        IssueDeSortie::Erreur
    );
    assert_eq!(
        IssueDeSortie::depuis_le_code(Some(101)),
        IssueDeSortie::Erreur
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
fn une_sortie_propre_rearme_le_repli_sans_qu_aucune_duree_n_intervienne() {
    let mut etat = EtatRelance::neuve();
    for _ in 0..5 {
        etat.tentative_lancee();
    }
    assert!(
        etat.espacement_ms() > ESPACEMENT_PLANCHER_MS,
        "le repli a bien grandi avant le test"
    );
    etat.reinitialiser_le_repli(IssueDeSortie::Propre);
    assert_eq!(
        etat.espacement_ms(),
        ESPACEMENT_PLANCHER_MS,
        "une sortie PROPRE doit réarmer le repli : la panne passée est résolue"
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
/// they share the conclusion: neither `Erreur` nor `Inconnue` re-arms.
#[test]
fn ni_un_refus_ni_une_issue_inconnue_ne_rearment_le_repli() {
    for issue in [IssueDeSortie::Erreur, IssueDeSortie::Inconnue] {
        let mut etat = EtatRelance::neuve();
        for _ in 0..5 {
            etat.tentative_lancee();
        }
        let avant = etat.espacement_ms();
        assert!(
            avant > ESPACEMENT_PLANCHER_MS,
            "le repli a bien grandi avant le test"
        );
        // A VERY long life — longer than the longest possible refusal
        // sleep — and yet no re-arming: duration does not enter
        // the decision, that is the whole of round 4.
        etat.reinitialiser_le_repli(issue);
        assert_eq!(
            etat.espacement_ms(),
            avant,
            "{issue:?} ne doit RIEN réarmer, quelle qu'ait été la durée de vie"
        );
    }
}

/// 🔴 THE EXACT RED OF THE DEFECT FIXED BY ROUND 2, REPLAYED HERE:
/// a REFUSED process staying alive up to `REPLI_MAX_MS` before
/// dying (`honorer_retry_suggere`'s sleep) must NEVER be
/// declared stable during that sleep. Probed at regular intervals,
/// as the supervisor loop would at ~10 Hz. **It is THE red
/// of the "trace loop"**, which rounds 3 and 4 had to preserve:
/// it keeps holding because `stable` never changed its
/// guard condition.
#[test]
fn stable_pendant_un_sommeil_de_refus_ne_declare_jamais_stable() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    let mut ecoule = 0u64;
    while ecoule < REPLI_MAX_MS {
        assert!(
            !etat.stable(ecoule),
            "déclaré stable à {ecoule} ms, alors que le sommeil de refus \
             peut durer jusqu'à {REPLI_MAX_MS} ms — c'est la boucle de \
             trace du round de correction 2"
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
/// on `reinitialiser_le_repli` then blocking the re-arming) by calling
/// `reinitialiser_le_repli(Propre)` AFTER `stable`. Since `stable()`
/// ITSELF resets `tentative` to zero in its true branch (see its doc),
/// that call could no longer prove anything: `tentative` was already zero
/// BEFORE it, whatever `reinitialiser_le_repli` did — proven by
/// mutation. The real guarantee is the INVARIANT that makes this state
/// unreachable, held directly below rather than deduced: `stable()
/// == true` resets `tentative` to zero IN THE SAME GESTURE that makes
/// `cycle_signale` fall back, so there is no longer an instant where `cycle_signale ==
/// false` and `tentative > 0` at the same time.
#[test]
fn un_processus_reellement_stable_finit_par_etre_declare_stable_une_fois() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    etat.tentative_lancee(); // le repli a grandi : tentative = 2
    assert!(!etat.stable(SEUIL_STABILITE_MS - 1));
    assert!(etat.stable(SEUIL_STABILITE_MS));
    assert_eq!(
        etat.tentative(),
        0,
        "l'invariant qui remplace la garantie visée au round 4 : `stable() \
         == true` remet `tentative` à zéro dans le MÊME geste qui fait \
         retomber `cycle_signale` — l'état que ce test visait avant \
         (cycle retombé, tentative encore positive) n'existe plus"
    );
    // A second call, cycle already fallen back: nothing more to signal as long
    // as no new attempt has taken place.
    assert!(!etat.stable(SEUIL_STABILITE_MS * 2));
}

/// Without a cycle in progress (no attempt launched since the last
/// stability), `stable` must declare nothing, whatever `ecoule_ms`
/// — a bridge never restarted has nothing to "become" stable "again".
#[test]
fn sans_cycle_en_cours_stable_ne_declare_jamais_rien() {
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
/// **Round 4 changes nothing to this property**: `reinitialiser_le_
/// repli` never touches `cycle_signale`, and the `Erreur` outcome does not even
/// re-arm `tentative`.
#[test]
fn sur_plusieurs_cycles_de_refus_une_seule_ligne_lancee_et_aucune_ligne_stable() {
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
        etat.reinitialiser_le_repli(IssueDeSortie::Erreur);
    }
    assert_eq!(
        lignes_lancees, 1,
        "UNE SEULE ligne « lancé » pour tout l'épisode — jamais une par \
         cycle : c'est la propriété de silence que le module promet déjà, \
         et que le bug du seuil unique cassait en réarmant `cycle_signale` \
         à chaque fausse stabilité"
    );
    assert_eq!(
        lignes_stables, 0,
        "AUCUNE ligne « stable » ne doit sortir tant qu'aucun cycle n'a \
         vraiment tenu : c'est exactement la boucle que le round de \
         correction 2 ferme, et que ni le round 3 ni le round 4 ne rouvrent"
    );
    assert!(
        etat.espacement_ms() > REPLI_MAX_MS / 2,
        "après cinq refus, l'espacement doit avoir GRANDI — c'est le \
         défaut du round 3, où il retombait au plancher à chaque cycle"
    );
}

/// 🔵 WITNESS: if a cycle ends up REALLY holding (the bridge stops
/// being refused), the "stable" line comes out ONCE, and the NEXT cycle
/// becomes noisy again on its first launch — the half of the mechanism
/// the test above, on its own, cannot prove since none of
/// its cycles ever reaches stability.
#[test]
fn apres_une_vraie_stabilite_le_cycle_suivant_redevient_bruyant() {
    let mut etat = EtatRelance::neuve();
    assert!(
        etat.tentative_lancee(),
        "premier lancement de l'épisode : bruyant"
    );
    assert!(
        etat.stable(SEUIL_STABILITE_MS),
        "vraiment resté vivant assez longtemps"
    );
    assert!(
        etat.tentative_lancee(),
        "un cycle NEUF, après une vraie stabilité, redevient bruyant"
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
fn un_pont_qui_meurt_en_erreur_apres_une_demi_seconde_ne_martele_pas() {
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
                relance.reinitialiser_le_repli(IssueDeSortie::Erreur);
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
        "{lancements} connexions /signal en une minute pour le pont SEUL, \
         contre un budget PARTAGÉ de {BUDGET_PARTAGE_PAR_MINUTE} et une \
         mesure de {LANCEMENTS_MESURES} : c'est le verrouillage de la VM que \
         ce lot existe pour fermer"
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
/// `le_seuil_de_stabilite_reste_strictement_au_dessus_du_plafond_de_repli`
/// and `stable_pendant_un_sommeil_de_refus_ne_declare_jamais_stable`.
#[test]
fn une_longue_vie_stable_puis_une_mort_en_erreur_reprend_au_plancher() {
    const TROIS_JOURS_MS: u64 = 3 * 24 * 60 * 60 * 1_000;
    let mut etat = EtatRelance::neuve();
    for _ in 0..6 {
        etat.tentative_lancee();
    }
    assert_eq!(
        etat.espacement_ms(),
        REPLI_MAX_MS,
        "six refus : le repli est au PLAFOND"
    );

    // Seventh launch — this one holds, and for long.
    etat.tentative_lancee();
    assert!(
        etat.stable(TROIS_JOURS_MS),
        "trois jours de vie, c'est stable"
    );

    // Then the network outage: `pont::executer` returns an `Err`, hence a non-zero
    // exit code, hence `IssueDeSortie::Erreur` — which
    // `reinitialiser_le_repli` rightly refuses to count as proof.
    etat.reinitialiser_le_repli(IssueDeSortie::Erreur);
    assert_eq!(
        etat.espacement_ms(),
        ESPACEMENT_PLANCHER_MS,
        "après trois jours de service, une coupure réseau doit retomber à \
         l'espacement PLANCHER et non rester au plafond de {REPLI_MAX_MS} ms \
         — la RAMPE de l'épisode de refus suivant, pas la première reprise \
         (immédiate dans les deux cas) : c'est la citation que \
         `reinitialiser_le_repli` porte, et que seule `stable` peut tenir \
         pour une mort EN ERREUR"
    );
    assert!(etat.doit_relancer(ESPACEMENT_PLANCHER_MS));
}
