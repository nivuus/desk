use super::*;

fn t0() -> Instant {
    Instant::now()
}

/// A pool of 2 places, without hysteresis, so that the eviction tests
/// do not have to age time.
fn petit() -> Vivier {
    Vivier::new(2, Duration::ZERO)
}

#[test]
fn une_fenetre_inscrite_est_endormie_tant_qu_elle_n_est_pas_visible() {
    let mut v = petit();
    let ordres = v.inscrire("a", t0());
    assert!(
        ordres.is_empty(),
        "une inscription seule n'ordonne rien : {ordres:?}"
    );
    assert_eq!(v.eveillee("a"), Some(false));
}

#[test]
fn une_fenetre_visible_est_reveillee_quand_il_reste_de_la_place() {
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    let ordres = v.signaler("a", true, true, t);
    assert_eq!(ordres, vec![("a".to_string(), Ordre::Reveiller)]);
    assert_eq!(v.eveillee("a"), Some(true));
}

#[test]
fn a_hidden_window_falls_asleep_with_the_hidden_reason() {
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    let ordres = v.signaler("a", false, false, t + Duration::from_secs(1));
    assert_eq!(
        ordres,
        vec![("a".to_string(), Ordre::Dormir(Raison::Masquee))]
    );
    assert_eq!(v.eveillee("a"), Some(false));
}

#[test]
fn au_dela_du_plafond_la_moins_recemment_vue_est_evincee() {
    let mut v = petit();
    let t = t0();
    for (i, nom) in ["a", "b", "c"].iter().enumerate() {
        let quand = t + Duration::from_millis(i as u64 * 100);
        v.inscrire(nom, quand);
        v.signaler(nom, true, true, quand);
    }
    // "a" was seen first, hence the least recently seen of the three.
    assert_eq!(v.eveillee("a"), Some(false), "a devait être évincée");
    assert_eq!(v.eveillee("b"), Some(true));
    assert_eq!(v.eveillee("c"), Some(true));
}

#[test]
fn l_eviction_porte_la_raison_evincee_et_non_masquee() {
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    v.inscrire("b", t + Duration::from_millis(100));
    v.signaler("b", true, true, t + Duration::from_millis(100));
    v.inscrire("c", t + Duration::from_millis(200));
    let ordres = v.signaler("c", true, true, t + Duration::from_millis(200));
    assert!(
        ordres.contains(&("a".to_string(), Ordre::Dormir(Raison::Evincee))),
        "l'éviction doit être motivée, pas confondue avec une mise en veille voulue : {ordres:?}"
    );
    assert!(ordres.contains(&("c".to_string(), Ordre::Reveiller)));
}

#[test]
fn un_focus_rafraichit_la_recence_et_protege_de_l_eviction() {
    let mut v = petit();
    let t = t0();
    for (i, nom) in ["a", "b"].iter().enumerate() {
        let quand = t + Duration::from_millis(i as u64 * 100);
        v.inscrire(nom, quand);
        v.signaler(nom, true, true, quand);
    }
    // "a" takes the focus back: it becomes the most recently seen again.
    v.signaler("a", true, true, t + Duration::from_millis(500));
    v.inscrire("c", t + Duration::from_millis(600));
    v.signaler("c", true, true, t + Duration::from_millis(600));
    assert_eq!(v.eveillee("a"), Some(true), "a venait d'être focalisée");
    assert_eq!(
        v.eveillee("b"),
        Some(false),
        "b est la plus anciennement vue"
    );
}

#[test]
fn l_hysteresis_empeche_d_evincer_une_fenetre_tout_juste_reveillee() {
    let mut v = Vivier::new(1, Duration::from_secs(2));
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    assert_eq!(v.eveillee("a"), Some(true));

    // "b" asks to wake 500 ms later: "a" is protected.
    v.inscrire("b", t + Duration::from_millis(500));
    let ordres = v.signaler("b", true, true, t + Duration::from_millis(500));
    assert!(
        ordres.is_empty(),
        "rien ne doit bouger sous l'hystérésis : {ordres:?}"
    );
    assert_eq!(v.eveillee("a"), Some(true));
    assert_eq!(v.eveillee("b"), Some(false));
}

#[test]
fn passee_l_hysteresis_la_rearbitration_periodique_debloque_le_reveil() {
    let mut v = Vivier::new(1, Duration::from_secs(2));
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    v.inscrire("b", t + Duration::from_millis(500));
    v.signaler("b", true, true, t + Duration::from_millis(500));

    // Without this re-arbitration, "b" would wait for a signal that will
    // never come: it is already visible and already focused.
    let ordres = v.rearbitrer(t + Duration::from_secs(3));
    assert!(ordres.contains(&("a".to_string(), Ordre::Dormir(Raison::Evincee))));
    assert!(ordres.contains(&("b".to_string(), Ordre::Reveiller)));
}

#[test]
fn a_hidden_window_falls_asleep_even_under_the_hysteresis() {
    // Hiding is an EXPLICIT gesture of the user: hysteresis
    // protects against eviction flapping, never against a will.
    let mut v = Vivier::new(2, Duration::from_secs(10));
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    let ordres = v.signaler("a", false, false, t + Duration::from_millis(10));
    assert_eq!(
        ordres,
        vec![("a".to_string(), Ordre::Dormir(Raison::Masquee))]
    );
}

#[test]
fn retirer_une_fenetre_eveillee_rend_sa_place_a_une_endormie() {
    let mut v = petit();
    let t = t0();
    for (i, nom) in ["a", "b", "c"].iter().enumerate() {
        let quand = t + Duration::from_millis(i as u64 * 100);
        v.inscrire(nom, quand);
        v.signaler(nom, true, true, quand);
    }
    assert_eq!(v.eveillee("a"), Some(false));
    let ordres = v.retirer("c", t + Duration::from_secs(1));
    assert_eq!(ordres, vec![("a".to_string(), Ordre::Reveiller)]);
    assert_eq!(v.eveillee("c"), None, "une session retirée n'a plus d'état");
}

#[test]
fn un_signal_pour_une_session_inconnue_est_ignore_sans_paniquer() {
    let mut v = petit();
    let ordres = v.signaler("fantome", true, true, t0());
    assert!(ordres.is_empty());
    assert_eq!(v.eveillee("fantome"), None);
}

#[test]
fn un_ordre_n_est_jamais_emis_deux_fois_pour_le_meme_etat() {
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    // Second identical signal: the window is already awake.
    let ordres = v.signaler("a", true, true, t + Duration::from_millis(50));
    assert!(
        ordres.is_empty(),
        "un état inchangé n'ordonne rien : {ordres:?}"
    );
}

#[test]
fn a_failing_wake_returns_the_entry_asleep_and_does_not_reelect_it_next_round() {
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    assert_eq!(v.eveillee("a"), Some(true), "a était éveillée");

    // The encoder fails to build: record the failure.
    let ordres = v.echec_de_reveil("a", t + Duration::from_millis(100));
    assert_eq!(
        v.eveillee("a"),
        Some(false),
        "a doit s'endormir après l'échec"
    );
    // No new order must be emitted: a was already the only awake one.
    assert!(
        ordres.is_empty(),
        "pas de réélection au tour même de l'échec : {ordres:?}"
    );
}

#[test]
fn past_the_respite_a_rearbitrate_does_repropose_the_failed_window() {
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    v.echec_de_reveil("a", t + Duration::from_millis(100));
    assert_eq!(v.eveillee("a"), Some(false), "a est endormie après l'échec");

    // Before the respite: no re-proposal.
    let ordres = v.rearbitrer(t + Duration::from_millis(200));
    assert!(
        ordres.is_empty(),
        "avant le répit, a n'est pas reproposée : {ordres:?}"
    );
    assert_eq!(v.eveillee("a"), Some(false));

    // After the respite: re-proposal.
    let ordres = v.rearbitrer(t + Duration::from_millis(700));
    assert_eq!(
        ordres,
        vec![("a".to_string(), Ordre::Reveiller)],
        "après le répit, a doit être réveillée : {ordres:?}"
    );
    assert_eq!(v.eveillee("a"), Some(true));
}

#[test]
fn returned_orders_place_every_sleep_before_any_wake() {
    // Case where one window is turned off and another turned on
    // simultaneously: check that Dormir precedes Reveiller.
    let mut v = petit();
    let t = t0();
    for (i, nom) in ["a", "b", "c"].iter().enumerate() {
        let quand = t + Duration::from_millis(i as u64 * 100);
        v.inscrire(nom, quand);
        v.signaler(nom, true, true, quand);
    }
    // State: a asleep, b and c awake.
    assert_eq!(v.eveillee("a"), Some(false));
    assert_eq!(v.eveillee("b"), Some(true));
    assert_eq!(v.eveillee("c"), Some(true));

    // Bring in a fourth window: d > c > b > a in recency.
    v.inscrire("d", t + Duration::from_millis(300));
    let ordres = v.signaler("d", true, true, t + Duration::from_millis(300));

    // Expected orders: b falls asleep (Dormir), d wakes up (Reveiller).
    // Or more broadly, the Dormir before the Reveiller.
    let premiers_dormir = ordres
        .iter()
        .position(|(_, o)| matches!(o, Ordre::Dormir(_)));
    let premier_reveiller = ordres
        .iter()
        .position(|(_, o)| matches!(o, Ordre::Reveiller));
    assert!(
        premiers_dormir.is_some(),
        "ce scenario doit produire au moins un Dormir : {ordres:?}"
    );
    assert!(
        premier_reveiller.is_some(),
        "ce scenario doit produire au moins un Reveiller : {ordres:?}"
    );
    let (d_idx, r_idx) = (premiers_dormir.unwrap(), premier_reveiller.unwrap());
    assert!(
        d_idx < r_idx,
        "tous les Dormir doivent précéder tout Reveiller : {ordres:?}"
    );
}

#[test]
fn a_window_in_respite_stays_excluded_from_candidates_until_the_respite_elapses() {
    // A window that fails to wake up stays in respite and is not
    // proposed again even if a place is freed, until the respite
    // elapses.
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    v.inscrire("b", t + Duration::from_millis(100));
    v.signaler("b", true, true, t + Duration::from_millis(100));

    // Wake-up failure of "a" at t + 200.
    v.echec_de_reveil("a", t + Duration::from_millis(200));
    // "b" gets hidden at t + 300 (100 ms after the failure), freeing a place.
    // But "a" is still in respite (the respite lasts 500 ms).
    let ordres = v.signaler("b", false, false, t + Duration::from_millis(300));
    assert!(
        !ordres.iter().any(|(nom, _)| nom == "a"),
        "a ne doit pas être réveillée car elle est en répit depuis seulement 100 ms : {ordres:?}"
    );
    assert_eq!(v.eveillee("a"), Some(false));
}

#[test]
fn eveillees_rend_exactement_les_sessions_reveillees() {
    let base = t0();
    let mut vivier = Vivier::new(2, Duration::from_secs(2));
    vivier.inscrire("a", base);
    vivier.inscrire("b", base);
    assert!(vivier.eveillees().is_empty(), "une fenêtre naît endormie");

    vivier.signaler("a", true, true, base);
    assert_eq!(vivier.eveillees(), vec!["a".to_string()]);

    vivier.signaler("b", true, false, base);
    let mut eveillees = vivier.eveillees();
    eveillees.sort();
    assert_eq!(eveillees, vec!["a".to_string(), "b".to_string()]);

    vivier.retirer("a", base);
    assert_eq!(vivier.eveillees(), vec!["b".to_string()]);
}

/// The two clauses of `annuler_ordre_non_livre`'s written contract that its
/// only caller cannot reach.
///
/// ⚠️ **It had NO pure test**: the two reds that hold it
/// go through `sommeil::registre::distribuer`, hence through the path that matters
/// — but two clauses were held by a sentence alone. The guard "no
/// effect if the session has disappeared, never a panic" is **defensive code
/// no caller can exercise** (the names come from `canaux`, under the
/// same lock), and the invariant "it touches ONLY `eveillee`" was pinned
/// by nothing.
#[test]
fn annuler_un_ordre_non_livre_ne_touche_qu_eveillee_et_ignore_une_session_disparue() {
    let t = t0();
    let mut v = Vivier::new(PLAFOND_EVEIL, HYSTERESIS);
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    assert_eq!(
        v.eveillee("a"),
        Some(true),
        "précondition : elle est éveillée"
    );

    // An undelivered `Dormir` brings it back to its state before the order.
    v.annuler_ordre_non_livre("a", Ordre::Dormir(Raison::Masquee));
    assert_eq!(
        v.eveillee("a"),
        Some(true),
        "un Dormir non livré la laisse éveillée"
    );
    v.annuler_ordre_non_livre("a", Ordre::Reveiller);
    assert_eq!(
        v.eveillee("a"),
        Some(false),
        "un Reveiller non livré la laisse endormie"
    );

    // The guard: a vanished session is ignored, never a panic. It is
    // the clause no caller can reach.
    v.annuler_ordre_non_livre("jamais-inscrite", Ordre::Reveiller);
    assert_eq!(v.eveillee("jamais-inscrite"), None);

    // The invariant: nothing other than `eveillee` moved — the session stays
    // known, and a re-arbitration re-elects it immediately (so neither its visibility nor
    // its recency were erased).
    let ordres = v.rearbitrer(t);
    assert!(
        ordres.contains(&("a".to_string(), Ordre::Reveiller)),
        "l'annulation ne doit rien effacer d'autre : {ordres:?}"
    );
}
