//! Tests of `sommeil` — sibling file rather than inline module:
//! `sommeil.rs` was at 500 lines for a project cap of 500 (zero
//! margin since task 5 of D7), and this suite alone accounted for 195 of them.
//! Extract rather than compress — same scheme as `capteur/distante.rs` /
//! `capteur/distante/tests.rs`.
//!
//! **The module path stays `sommeil::tests`**: `parts::tests` and
//! `porteurs::tests` import `sommeil::tests::{premier_ordre,
//! verrouiller_pour_le_test}`, and this extraction changes neither that path nor
//! any visibility — only the physical location of the file changes.

use super::*;

/// Unique names (t5-a, t5-b…) are not enough to isolate these tests from
/// each other: the shared pool has only ONE cap of `PLAFOND_EVEIL` places
/// for the whole process, and a test that saturates it (to test a
/// place being freed) de facto deprives the other tests, run in
/// parallel by default, of any available place — observed: saturating
/// at 8 makes a neighbouring test that expects to wake up
/// immediately fail intermittently. A `Mutex` dedicated to tests serialises
/// this file without touching production code or `vivier.rs`.
static VERROU_TESTS: Mutex<()> = Mutex::new(());

/// `pub(super)`: reused by `parts::tests`, which saturates the same shared
/// pool and must serialise on it in exactly the same way (this module
/// and `parts::tests` are two distinct descendants of `sommeil`, not
/// of each other — hence the explicit visibility).
pub(super) fn verrouiller_pour_le_test() -> MutexGuard<'static, ()> {
    VERROU_TESTS
        .lock()
        .unwrap_or_else(|empoisonne| empoisonne.into_inner())
}

/// The first sleep ORDER received, ignoring the shares that may
/// precede or be interleaved with it.
///
/// **Necessary since `inscrire` ends with
/// `distribuer_les_parts`**: a session's very first share goes out at
/// registration itself, before any order — a window still asleep does
/// have a share (the `PART_DORMANTE_BPS` floor), and it is deliberate
/// (see the doc of `inscrire`). The ORDER tests, inherited from D5, are about
/// `Ordre` and not `Message`: this filter restores their original
/// intent without changing it.
///
/// `pub(super)`: reused by `parts::tests`, for the same reason as
/// `verrouiller_pour_le_test`.
pub(super) fn premier_ordre(canal: &ReceveurSession) -> Option<Ordre> {
    loop {
        match canal.essayer_recevoir() {
            Ok(Message::Sommeil(ordre)) => return Some(ordre),
            Ok(Message::Part { .. }) => continue,
            Ok(Message::Audio { .. }) => continue,
            Ok(Message::PressePapier { .. }) => continue,
            Err(_) => return None,
        }
    }
}

#[test]
fn une_session_inscrite_recoit_l_ordre_de_se_reveiller_quand_elle_devient_visible() {
    let _verrou = verrouiller_pour_le_test();
    // Unique names: the registry is a process-GLOBAL state, and Rust
    // tests run in parallel in the same process.
    let (ordres, generation) = inscrire("t5-a", 5001);
    signaler("t5-a", true, true);
    assert_eq!(premier_ordre(&ordres), Some(Ordre::Reveiller));
    retirer("t5-a", generation);
}

#[test]
fn une_session_retiree_ne_recoit_plus_rien() {
    let _verrou = verrouiller_pour_le_test();
    let (ordres, generation) = inscrire("t5-b", 5002);
    retirer("t5-b", generation);
    signaler("t5-b", true, true);
    assert_eq!(premier_ordre(&ordres), None);
}

#[test]
fn les_deux_raisons_ont_un_texte_stable_pour_le_client() {
    // No access to the shared pool here: no lock required.
    assert_eq!(raison_en_texte(Raison::Masquee), "masquee");
    assert_eq!(raison_en_texte(Raison::Evincee), "evincee");
}

#[test]
fn un_echec_de_reveil_rendort_la_session_et_ne_la_reelit_pas_immediatement() {
    let _verrou = verrouiller_pour_le_test();
    // "t5-c" becomes visible and focused, hence awakened by arbitrer().
    let (ordres, generation) = inscrire("t5-c", 5003);
    signaler("t5-c", true, true);
    assert_eq!(premier_ordre(&ordres), Some(Ordre::Reveiller));

    // Rebuilding the WindowsSource fails: the pool must put the session
    // back into the asleep state. No new ORDER is due within the
    // 500 ms of respite that follow, even if the session stays visible and
    // focused: proposing it again immediately would loop at every
    // arbitration on an encoder construction bound to fail again. A
    // share (the return to the `PART_DORMANTE_BPS` floor) is on the other hand
    // legitimate: the window is really asleep again.
    echec_de_reveil("t5-c");
    assert_eq!(premier_ordre(&ordres), None);

    retirer("t5-c", generation);
}

/// M1 from the final branch review of sub-block D6: `retirer` emptied
/// `focalisee`, but neither `distribuer` nor the `rompus` path of
/// `distribuer_les_parts` did. A focused session dying
/// through a broken channel — the window thread panicking before its single
/// removal point — therefore left its name in the registry.
///
/// **The state is read directly, and it is deliberate.** The consequence
/// visible through the shares is not discriminating: a dead name designates
/// no live window, so the boost applies to nobody —
/// which is also the case when `focalisee` is `None`. What BITES is
/// the re-registration of the same name (re-attachment, D4's resumption path),
/// which would inherit the focus without the client ever having re-emitted it; but
/// testing it through the shares would require a `signaler` on that name, which empties
/// `focalisee` by itself and would erase the defect before measuring it.
/// The field is private to this module, and this test is a descendant of it:
/// reading it is the most direct and least ambiguous observation.
#[test]
fn un_canal_rompu_libere_aussi_le_focus_de_la_session_morte() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation_focus) = inscrire("m1-focus", 5004);
    signaler("m1-focus", true, true);
    assert_eq!(
        etat().focalisee.as_deref(),
        Some("m1-focus"),
        "precondition: the registry does hold this session as the focused one"
    );

    // The thread of "m1-focus" dies WITHOUT going through `retirer`, exactly what
    // happens when it panics.
    drop(canal);

    // Registering a third session — asleep — changes the
    // shared budget, hence the share of "m1-focus", hence attempts a send on
    // its broken channel: that is what triggers the detection.
    let (_, generation_tiers) = inscrire("m1-tiers", 5005);
    assert_eq!(
        etat().focalisee,
        None,
        "the focus of a dead session must be returned with the rest of what the registry \
         kept of it"
    );

    retirer("m1-focus", generation_focus);
    retirer("m1-tiers", generation_tiers);
}

#[test]
fn un_retrait_qui_libere_une_place_reveille_bien_la_session_qui_l_attendait() {
    let _verrou = verrouiller_pour_le_test();
    // Saturates the PLAFOND_EVEIL (8) places with dedicated sessions, whose
    // ReceveurSession we keep alive so that their channel is never
    // broken by accident during the test.
    let mut recepteurs_pleins = Vec::new();
    for i in 0..8 {
        let nom = format!("t5-plein-{i}");
        let (ordres, generation) = inscrire(&nom, 5100 + i as u32);
        signaler(&nom, true, true);
        assert_eq!(
            premier_ordre(&ordres),
            Some(Ordre::Reveiller),
            "{nom} should wake up"
        );
        recepteurs_pleins.push((nom, ordres, generation));
    }
    let generation_plein_0 = recepteurs_pleins[0].2;

    // "t5-attend" arrives while the cap is already reached: it
    // stays asleep, for lack of a place.
    let (ordres_attend, generation_attend) = inscrire("t5-attend", 5200);
    signaler("t5-attend", true, true);
    assert_eq!(
        premier_ordre(&ordres_attend),
        None,
        "t5-attend should stay asleep"
    );

    // "t5-tardif" arrives next: more recent than "t5-attend", so it
    // would get ahead of it if a place were freed. Its receiver is thrown away
    // immediately (by the `_` of the destructuring): its channel is broken
    // before any send attempt.
    let (_, generation_tardif) = inscrire("t5-tardif", 5201);
    signaler("t5-tardif", true, true);

    // Frees ONE place by removing the first "plein". The pool then elects
    // "t5-tardif" (the more recent of the two blocked candidates),
    // and that delivery fails since its channel is broken. Without the
    // loop in `distribuer`, the Reveiller this removal generates
    // AFTERWARDS for "t5-attend" would be lost forever: the pool
    // would already have set `eveillee = true` on "t5-attend" internally, and
    // no re-arbitration would ever propose it again.
    retirer("t5-plein-0", generation_plein_0);

    assert_eq!(
        premier_ordre(&ordres_attend),
        Some(Ordre::Reveiller),
        "the wake-up freed by the death of t5-tardif must reach t5-attend"
    );

    // Nettoyage.
    retirer("t5-attend", generation_attend);
    retirer("t5-tardif", generation_tardif);
    for (nom, _, generation) in recepteurs_pleins.into_iter().skip(1) {
        retirer(&nom, generation);
    }
}

#[test]
fn le_repit_expire_et_rend_la_fenetre_apte() {
    // Tests `purger_les_inaptitudes`, the PRODUCT's function — not
    // `HashMap::retain`. The clock is injected (`maintenant`), which makes
    // the test deterministic without any real wait.
    let mut inaptes: HashMap<String, Instant> = HashMap::new();
    let t0 = Instant::now();
    inaptes.insert("w-1".into(), t0 + Duration::from_millis(10));
    inaptes.insert("w-2".into(), t0 + Duration::from_secs(60));

    purger_les_inaptitudes(&mut inaptes, t0 + Duration::from_millis(20));

    assert!(
        !inaptes.contains_key("w-1"),
        "the grace period of w-1 has expired"
    );
    assert!(
        inaptes.contains_key("w-2"),
        "the one of w-2 is still running"
    );
}

#[test]
fn une_inaptitude_dont_l_echeance_vaut_exactement_maintenant_est_purgee() {
    // On an EMPTY registry, `purger_les_inaptitudes` can only remove —
    // the test it replaces (`une_purge_sur_un_registre_vide_ne_panique_pas`)
    // therefore could not return the other value (final branch review,
    // M3). The case that counts is the bound: `retain(|_, echeance| *echeance >
    // maintenant)` (agent/src/capteur/sommeil.rs) purges a deadline
    // EXACTLY equal to `maintenant`, not only an expired deadline.
    let mut inaptes: HashMap<String, Instant> = HashMap::new();
    let maintenant = Instant::now();
    inaptes.insert("w-1".into(), maintenant);

    purger_les_inaptitudes(&mut inaptes, maintenant);

    assert!(
        !inaptes.contains_key("w-1"),
        "a deadline equal to `maintenant` must be purged, not kept"
    );
}

/// Remedy for the review reservation: `oublier` (hence `retirer`) must purge
/// `inaptes` and `rearmements`, otherwise a re-attachment — which re-registers the
/// SAME session (`inscrire`, D4's resumption path) — would inherit a
/// STALE unfitness or re-arm counter. It is defect M1 of
/// the final branch review of sub-block D6, replayed on these two tables.
#[test]
fn un_retrait_purge_l_inaptitude_et_le_compteur_de_rearmements() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("t8-purge", 6001);

    audio_mort("t8-purge");
    assert!(
        etat().inaptes.contains_key("t8-purge"),
        "precondition: the session must be marked unfit"
    );
    assert_eq!(
        etat().rearmements.get("t8-purge"),
        Some(&1),
        "precondition: a first re-arm must be counted"
    );

    retirer("t8-purge", generation);

    assert!(
        !etat().inaptes.contains_key("t8-purge"),
        "the unfitness of a removed session must be forgotten, otherwise a \
         reattachment would wrongly inherit it"
    );
    assert!(
        !etat().rearmements.contains_key("t8-purge"),
        "the re-arm counter of a removed session must be forgotten, \
         otherwise a reattachment would inherit a stale counter"
    );

    drop(canal);
}

/// Remedy for reservation I2 of the review of task 9 (sub-block D9):
/// `SourceDistante::rattacher` cannot distinguish a real restart of the
/// sensor from a mere channel reconnection on a sensor that stayed alive, and
/// resets `Session::audio_mort_signale` in both cases — a second
/// `AudioMort` for the SAME session, still unfit, must therefore NOT count
/// as one more CONSECUTIVE failure: it would be the same failure, said again, and it
/// would bring giving up for good 24 h closer for a reason foreign to the
/// capture's real state.
#[test]
fn un_signal_audio_mort_redondant_ne_recompte_pas_le_rearmement() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("t9-redondant", 6002);

    audio_mort("t9-redondant");
    assert_eq!(
        etat().rearmements.get("t9-redondant"),
        Some(&1),
        "precondition: a first re-arm must be counted"
    );

    // Second signal, without any removal having happened in between:
    // the session is still unfit (respite of `REPIT_REARMEMENT_AUDIO`, not
    // yet expired). A real channel re-attached on a restarted sensor would be
    // indistinguishable from this for `SourceDistante` — it is exactly the case
    // this test isolates on the sensor side, where the distinction IS possible.
    audio_mort("t9-redondant");
    assert_eq!(
        etat().rearmements.get("t9-redondant"),
        Some(&1),
        "a redundant signal, received while the session is still \
         unfit, must not advance the re-arm counter"
    );

    retirer("t9-redondant", generation);
    drop(canal);
}

/// D9's hand-over 6, registry side: `signaler_audio_vivant` — the PROOF —
/// closes the re-arm counter, exactly as the arbitration decision alone
/// used to (wrongly, see `sommeil/porteurs.rs`).
#[test]
fn un_signal_audio_vivant_remet_le_compteur_de_rearmements_a_zero() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("t10-preuve", 6003);

    audio_mort("t10-preuve");
    assert_eq!(
        etat().rearmements.get("t10-preuve"),
        Some(&1),
        "precondition: a first re-arm must be counted"
    );

    signaler_audio_vivant("t10-preuve");
    assert!(
        !etat().rearmements.contains_key("t10-preuve"),
        "a proof of sound must reset the re-arm counter to zero"
    );

    retirer("t10-preuve", generation);
    drop(canal);
}

/// F5 (D7, pre-existing). A session dies, re-attaches under the same name
/// with a new generation, and the `retirer` of the PREVIOUS instance
/// arrives afterwards. Without the generation, it would take the live session away.
#[test]
fn un_retirer_perime_n_emporte_pas_l_inscription_neuve() {
    let mut generations: HashMap<String, u64> = HashMap::new();
    generations.insert("w-1".into(), 7); // the new registration, after re-attachment

    assert!(
        retirer_est_perime(&generations, "w-1", 6),
        "the removal of generation 6 is late: it must remove nothing"
    );
    assert!(
        !retirer_est_perime(&generations, "w-1", 7),
        "the one of the current generation does remove"
    );
}

#[test]
fn un_retirer_sur_une_session_inconnue_n_est_pas_perime() {
    // No registration: `retirer` must follow its normal path, which is
    // already tolerant of absence. Returning `true` here would make it inert for
    // any session the registry does not know yet.
    let generations: HashMap<String, u64> = HashMap::new();
    assert!(!retirer_est_perime(&generations, "w-1", 3));
}

#[test]
fn un_retirer_d_une_generation_posterieure_n_est_pas_perime() {
    // The case of a `retirer` arriving AFTER the registration it targets: it
    // carries a generation more recent than the recorded one, so it acts.
    let mut generations: HashMap<String, u64> = HashMap::new();
    generations.insert("w-1".into(), 7);
    assert!(!retirer_est_perime(&generations, "w-1", 8));
}

/// The test the three above could NOT see (review of the
/// first version of this task, D9): they test `retirer_est_perime`
/// on hand-picked values, never on the values
/// `inscrire`/`retirer` ACTUALLY produce in production. This one simulates
/// the F5 re-attachment with the complete path — two successive `inscrire`
/// for the SAME name, as a child re-attaching to the sensor does
/// after a pipe break (`CanalTube::rattacher`) while the previous
/// window thread is still alive.
#[test]
fn un_rattachement_recoit_une_generation_neuve_et_le_retirer_precedent_est_perime() {
    let _verrou = verrouiller_pour_le_test();
    let (premier_canal, premiere_generation) = inscrire("t10-rattache", 7001);
    // The re-attachment: SAME name, before the `retirer` of the previous
    // instance has had time to arrive.
    let (second_canal, seconde_generation) = inscrire("t10-rattache", 7001);

    assert_ne!(
        premiere_generation, seconde_generation,
        "two registrations of the same name must receive distinct generations"
    );

    // The `retirer` of the PREVIOUS instance, arriving after the re-attachment
    // (this is exactly F5): it must remove NOTHING from the live
    // registration.
    retirer("t10-rattache", premiere_generation);
    assert!(
        etat().canaux.contains_key("t10-rattache"),
        "a stale removal must not take the fresh registration with it"
    );

    // The `retirer` of the LIVE instance, for its part, does remove.
    retirer("t10-rattache", seconde_generation);
    assert!(
        !etat().canaux.contains_key("t10-rattache"),
        "the removal of the current generation must really remove"
    );

    drop(premier_canal);
    drop(second_canal);
}

/// 🔴 THE COVERAGE GAP MEASURED ON 25 AUGUST 2026, AND WHAT IT TAUGHT.
///
/// The broken-channel detection of `distribuer` — `envoyer(...).is_err()`, the
/// ORDERS path — was turned red by **no** test in the repository. Measured by
/// targeted mutation of that single line (`is_err()` replaced by `false`,
/// everything else intact): **1,022 tests, zero failures**.
///
/// **Why**, and it is not a drafting oversight: the three callers
/// of `distribuer` (`inscrire`, `retirer`, the wheel round) all chain
/// onto `parts::distribuer_les_parts`, which carries the SAME detection. An order
/// changes the session's wakefulness, hence its share, hence the second
/// detection fires for sure and purges what the first let through.
/// **The first is masked by the second on every end-to-end path**,
/// and that is why `un_retrait_qui_libere_une_place_reveille_bien_la_
/// session_qui_l_attendait` — which nonetheless deliberately breaks a channel — stays
/// green under the mutation.
///
/// ⚠️ **IT IS NOT A REASON TO REMOVE THE DETECTION FROM `distribuer`.**
/// It is only redundant as long as the dead session's share CHANGES with
/// its order; `distribuer_les_parts` skips any session whose share is
/// unchanged (`dernieres_parts`), and nothing forces a future order to make
/// a share vary. It also decides the ORDER of the purge, on which the
/// loop-until-exhaustion documented on `distribuer` depends.
///
/// **This test therefore isolates it by calling `distribuer` DIRECTLY**, without the
/// `distribuer_les_parts` that masks it — the only way to test that
/// line and nothing else. It turns red under the mutation above.
#[test]
fn distribuer_purge_a_lui_seul_une_session_dont_le_canal_est_rompu() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("t16-ordres", 5600);
    // The window thread dies WITHOUT going through `retirer`: that is what happens
    // when it panics before its single removal point.
    drop(canal);
    assert!(
        etat().canaux.contains_key("t16-ordres"),
        "precondition: the session is registered, and nothing has purged it yet"
    );

    {
        // `distribuer` ALONE. The `MutexGuard` is taken here and released at the end
        // of the block: `etat()` is not reentrant, and taking it again without having
        // released it would deadlock this thread.
        let mut garde = etat();
        distribuer(
            &mut garde,
            vec![("t16-ordres".to_string(), Ordre::Reveiller)],
        );
        assert!(
            !garde.canaux.contains_key("t16-ordres"),
            "`distribuer` must itself purge the session whose send returned Err"
        );
    }

    // No effect: the session is already forgotten. Present so that this test
    // leaves nothing behind in the process's GLOBAL registry.
    retirer("t16-ordres", generation);
}
