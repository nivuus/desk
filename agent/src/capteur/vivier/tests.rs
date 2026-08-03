use super::*;

fn t0() -> Instant {
    Instant::now()
}

/// Un vivier de 2 places, sans hystérésis, pour que les tests d'éviction
/// n'aient pas à faire vieillir le temps.
fn petit() -> Vivier {
    Vivier::nouveau(2, Duration::ZERO)
}

#[test]
fn une_fenetre_inscrite_est_endormie_tant_qu_elle_n_est_pas_visible() {
    let mut v = petit();
    let ordres = v.inscrire("a", t0());
    assert!(ordres.is_empty(), "une inscription seule n'ordonne rien : {ordres:?}");
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
fn une_fenetre_masquee_s_endort_avec_la_raison_masquee() {
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    let ordres = v.signaler("a", false, false, t + Duration::from_secs(1));
    assert_eq!(ordres, vec![("a".to_string(), Ordre::Dormir(Raison::Masquee))]);
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
    // « a » a été vue en premier, donc la plus anciennement vue des trois.
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
    // « a » reprend le focus : elle redevient la plus récemment vue.
    v.signaler("a", true, true, t + Duration::from_millis(500));
    v.inscrire("c", t + Duration::from_millis(600));
    v.signaler("c", true, true, t + Duration::from_millis(600));
    assert_eq!(v.eveillee("a"), Some(true), "a venait d'être focalisée");
    assert_eq!(v.eveillee("b"), Some(false), "b est la plus anciennement vue");
}

#[test]
fn l_hysteresis_empeche_d_evincer_une_fenetre_tout_juste_reveillee() {
    let mut v = Vivier::nouveau(1, Duration::from_secs(2));
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    assert_eq!(v.eveillee("a"), Some(true));

    // « b » demande à veiller 500 ms plus tard : « a » est protégée.
    v.inscrire("b", t + Duration::from_millis(500));
    let ordres = v.signaler("b", true, true, t + Duration::from_millis(500));
    assert!(ordres.is_empty(), "rien ne doit bouger sous l'hystérésis : {ordres:?}");
    assert_eq!(v.eveillee("a"), Some(true));
    assert_eq!(v.eveillee("b"), Some(false));
}

#[test]
fn passee_l_hysteresis_la_rearbitration_periodique_debloque_le_reveil() {
    let mut v = Vivier::nouveau(1, Duration::from_secs(2));
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    v.inscrire("b", t + Duration::from_millis(500));
    v.signaler("b", true, true, t + Duration::from_millis(500));

    // Sans ce ré-arbitrage, « b » attendrait un signal qui ne viendra
    // jamais : elle est déjà visible et déjà focalisée.
    let ordres = v.rearbitrer(t + Duration::from_secs(3));
    assert!(ordres.contains(&("a".to_string(), Ordre::Dormir(Raison::Evincee))));
    assert!(ordres.contains(&("b".to_string(), Ordre::Reveiller)));
}

#[test]
fn une_fenetre_masquee_s_endort_meme_sous_l_hysteresis() {
    // Se masquer est un geste EXPLICITE de l'utilisateur : l'hystérésis
    // protège contre le battement d'éviction, jamais contre une volonté.
    let mut v = Vivier::nouveau(2, Duration::from_secs(10));
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    let ordres = v.signaler("a", false, false, t + Duration::from_millis(10));
    assert_eq!(ordres, vec![("a".to_string(), Ordre::Dormir(Raison::Masquee))]);
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
    // Second signal identique : la fenêtre est déjà éveillée.
    let ordres = v.signaler("a", true, true, t + Duration::from_millis(50));
    assert!(ordres.is_empty(), "un état inchangé n'ordonne rien : {ordres:?}");
}

#[test]
fn un_reveil_qui_echoue_rend_l_entree_endormie_et_ne_la_réélit_pas_au_tour_suivant() {
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    assert_eq!(v.eveillee("a"), Some(true), "a était éveillée");

    // L'encodeur échoue à se construire : enregistrer l'échec.
    let ordres = v.echec_de_reveil("a", t + Duration::from_millis(100));
    assert_eq!(v.eveillee("a"), Some(false), "a doit s'endormir après l'échec");
    // Aucun nouvel ordre ne doit être émis : a était déjà le seul éveillé.
    assert!(ordres.is_empty(), "pas de réélection au tour même de l'échec : {ordres:?}");
}

#[test]
fn passé_le_répit_un_rearbitrer_repropose_bien_la_fenetre_en_echec() {
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    v.echec_de_reveil("a", t + Duration::from_millis(100));
    assert_eq!(v.eveillee("a"), Some(false), "a est endormie après l'échec");

    // Avant le répit : pas de reproposition.
    let ordres = v.rearbitrer(t + Duration::from_millis(200));
    assert!(
        ordres.is_empty(),
        "avant le répit, a n'est pas reproposée : {ordres:?}"
    );
    assert_eq!(v.eveillee("a"), Some(false));

    // Après le répit : reproposition.
    let ordres = v.rearbitrer(t + Duration::from_millis(700));
    assert_eq!(
        ordres,
        vec![("a".to_string(), Ordre::Reveiller)],
        "après le répit, a doit être réveillée : {ordres:?}"
    );
    assert_eq!(v.eveillee("a"), Some(true));
}

#[test]
fn les_ordres_rendus_placent_tous_les_dormir_avant_tout_reveiller() {
    // Cas où on éteint une fenêtre et on en allume une autre
    // simultanément : vérifier que Dormir précède Reveiller.
    let mut v = petit();
    let t = t0();
    for (i, nom) in ["a", "b", "c"].iter().enumerate() {
        let quand = t + Duration::from_millis(i as u64 * 100);
        v.inscrire(nom, quand);
        v.signaler(nom, true, true, quand);
    }
    // État : a endormi, b et c éveillés.
    assert_eq!(v.eveillee("a"), Some(false));
    assert_eq!(v.eveillee("b"), Some(true));
    assert_eq!(v.eveillee("c"), Some(true));

    // Faire arriver une quatrième fenêtre : d > c > b > a en récence.
    v.inscrire("d", t + Duration::from_millis(300));
    let ordres = v.signaler("d", true, true, t + Duration::from_millis(300));

    // Ordres attendus : b s'endort (Dormir), d s'éveille (Reveiller).
    // Ou plus largement, les Dormir avant les Reveiller.
    let premiers_dormir = ordres.iter().position(|(_, o)| matches!(o, Ordre::Dormir(_)));
    let premier_reveiller =
        ordres.iter().position(|(_, o)| matches!(o, Ordre::Reveiller));
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
fn une_fenetre_en_repit_reste_exclue_des_candidates_jusqu_au_repit_ecoulé() {
    // Une fenêtre qui échoue à s'éveiller reste en répit et n'est pas
    // reproposée même si une place se libère, jusqu'à ce que le répit
    // s'écoule.
    let mut v = petit();
    let t = t0();
    v.inscrire("a", t);
    v.signaler("a", true, true, t);
    v.inscrire("b", t + Duration::from_millis(100));
    v.signaler("b", true, true, t + Duration::from_millis(100));

    // Échec du reveil de « a » à t + 200.
    v.echec_de_reveil("a", t + Duration::from_millis(200));
    // « b » se voit masquer à t + 300 (100 ms après l'échec), libérant une place.
    // Mais « a » est encore en répit (le répit dure 500 ms).
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
    let mut vivier = Vivier::nouveau(2, Duration::from_secs(2));
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
