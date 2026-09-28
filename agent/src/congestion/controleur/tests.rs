use super::super::hysteresis::t0;
use super::super::reconfiguration::config;
use super::*;
use std::time::Duration;

fn obs(estimate_bps: Option<u32>, loss: Option<f32>, at: Instant) -> Observation {
    Observation {
        estimate_bps,
        rtt: None,
        loss,
        at,
    }
}

#[test]
fn sans_estimation_le_debit_reste_au_plafond_et_l_adaptation_est_indisponible() {
    let base = t0();
    let mut c = Controleur::new(config(), base);
    let d = c.current();

    assert_eq!(d.video_bitrate_bps, 12_000_000);
    assert_eq!(d.encode_size, (1920, 1080));
    assert_eq!(d.adaptation, Adaptation::Indisponible);
    assert_eq!(d.qualite, Qualite::Bonne);

    // A hundred observations without an estimate change nothing and
    // produce no decision.
    for i in 0..100 {
        let at = base + Duration::from_millis(i * 100);
        assert_eq!(c.observer(obs(None, None, at)), None);
    }
    assert_eq!(c.current().adaptation, Adaptation::Indisponible);
}

#[test]
fn une_session_muette_ne_retranche_plus_le_budget_audio() {
    // The pre-existing defect D7 fixes: `Controleur::new` set
    // `audio_bps` unconditionally, so that seven windows out of eight
    // cut their video budget by 128 kb/s for a track they
    // did not have — about 8.5 % of a 1.5 Mb/s share.
    let base = t0();
    let mut with = Controleur::new(config(), base);
    let mut sans = Controleur::new(config(), base);
    sans.changer_audio_bps(0);

    let o = obs(Some(2_000_000), None, base + DELAI_AMORCAGE * 2);
    with.observer(o);
    sans.observer(o);

    assert!(
        sans.current().video_bitrate_bps > with.current().video_bitrate_bps,
        "sans piste audio, le budget video doit etre plus grand : {} vs {}",
        sans.current().video_bitrate_bps,
        with.current().video_bitrate_bps
    );
    assert_eq!(
        sans.current().video_bitrate_bps - with.current().video_bitrate_bps,
        crate::opus::BITRATE_BPS as u32,
        "l'ecart doit valoir exactement le budget audio"
    );
}

#[test]
fn la_premiere_estimation_rend_l_adaptation_active() {
    let base = t0();
    let mut c = Controleur::new(config(), base);
    let d = c
        .observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)))
        .expect("la première estimation doit produire une décision");

    assert_eq!(d.adaptation, Adaptation::Active);
    // 9 Mb/s × 0,9 − 128 kb/s d'audio = 7,972 Mb/s.
    assert_eq!(d.video_bitrate_bps, 7_972_000);
    // 7,97 Mb/s finance encore le barreau 0 (minimum 6,2 Mb/s).
    assert_eq!(d.encode_size, (1920, 1080));
    assert_eq!(d.qualite, Qualite::Bonne);
}

#[test]
fn le_debit_ne_bouge_pas_pour_moins_de_dix_pour_cent_d_ecart() {
    let base = t0();
    let mut c = Controleur::new(config(), base);
    c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)))
        .expect("première décision");

    // +5 %: under the threshold, no decision.
    assert_eq!(
        c.observer(obs(Some(9_450_000), None, base + Duration::from_secs(3))),
        None
    );
    // +20 %: beyond the threshold, a decision is produced.
    assert!(c
        .observer(obs(Some(10_800_000), None, base + Duration::from_secs(5)))
        .is_some());
}

#[test]
fn le_debit_ne_depasse_jamais_le_plafond() {
    let base = t0();
    let mut c = Controleur::new(config(), base);
    let d = c
        .observer(obs(Some(80_000_000), None, base + Duration::from_secs(1)))
        .expect("décision");
    assert_eq!(
        d.video_bitrate_bps, 12_000_000,
        "le plafond BITRATE doit borner"
    );
}

#[test]
fn une_contrainte_durable_fait_descendre_un_barreau_et_marque_la_degradation() {
    let base = t0();
    let mut c = Controleur::new(config(), base);
    c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)));

    // 5 Mb/s: under the minimum of rung 0 (6.2 Mb/s). It takes 2 s — but
    // the very first estimate dates from 1 s above, so the following 5 s
    // (up to 6 s) fall within `DELAI_AMORCAGE` (I3): the
    // controller never aims at a rung worse than the current one there, the
    // descent can therefore only start accumulating from 6 s,
    // to complete at 8 s. The sweep bound is pushed back
    // accordingly (40 -> 80, that is 15.8 s) to leave this margin, without
    // which this test would date from before the bootstrap and would wrongly fail.
    let mut derniere = None;
    for i in 10..80 {
        let at = base + Duration::from_millis(i * 200);
        if let Some(d) = c.observer(obs(Some(5_000_000), None, at)) {
            derniere = Some(d);
        }
    }
    let d = derniere.expect("une décision devait tomber");
    assert_ne!(
        d.encode_size,
        (1920, 1080),
        "la résolution devait descendre"
    );
    assert_eq!(d.qualite, Qualite::Degradee);
}

#[test]
fn below_the_floor_the_quality_is_declared_insufficient() {
    let base = t0();
    let mut c = Controleur::new(config(), base);
    c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)));

    let mut derniere = None;
    for i in 10..200 {
        let at = base + Duration::from_millis(i * 200);
        if let Some(d) = c.observer(obs(Some(300_000), None, at)) {
            derniere = Some(d);
        }
    }
    let d = derniere.expect("une décision devait tomber");
    assert_eq!(d.qualite, Qualite::Insuffisante);
    // We went down to the last rung, no lower: frame rate is
    // never sacrificed automatically.
    let echelle = Echelle::depuis((1920, 1080), 60);
    assert_eq!(d.encode_size, echelle.barreaux().last().unwrap().size);
}

#[test]
fn la_perte_est_convertie_en_pourcentage_et_plafonnee_a_vingt_cinq() {
    let base = t0();
    let mut c = Controleur::new(config(), base);

    let d = c
        .observer(obs(
            Some(9_000_000),
            Some(0.03),
            base + Duration::from_secs(1),
        ))
        .expect("décision");
    assert_eq!(d.opus_loss_perc, 3);

    // 60 % loss: capped at 25, beyond which redundancy costs
    // more than it saves.
    let d = c
        .observer(obs(
            Some(9_000_000),
            Some(0.60),
            base + Duration::from_secs(3),
        ))
        .expect("décision");
    assert_eq!(d.opus_loss_perc, 25);
}

#[test]
fn la_qualite_ne_ment_pas_pendant_la_fenetre_d_hysteresis() {
    let base = t0();
    let mut c = Controleur::new(config(), base);
    // Primes the controller then lets `DELAI_AMORCAGE` (5 s,
    // see I3) elapse before the two observations this test is about:
    // it checks a possible lie in STEADY STATE, not during the
    // BWE start-up ramp, which the bootstrap now deliberately
    // protects (another test is dedicated to that case: see
    // `l_amorcage_ne_declenche_pas_de_fausse_alerte`).
    c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)));
    c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(7)));

    // Collapse to 5 Mb/s, observed only once, less than 2 s after
    // the previous observation: the descent hysteresis has not had
    // time to lower the resolution, which therefore stays the source
    // size. The video bitrate, for its part, switches immediately (no
    // hysteresis protects it).
    let d = c
        .observer(obs(
            Some(5_000_000),
            None,
            base + Duration::from_millis(7500),
        ))
        .expect("le débit a assez bougé pour produire une décision");

    assert_eq!(
        d.encode_size,
        (1920, 1080),
        "l'hystérésis n'a pas eu le temps de faire descendre la résolution"
    );
    assert_ne!(
        d.qualite,
        Qualite::Bonne,
        "le débit ne finance plus la résolution encore appliquée : la qualité ne doit pas mentir"
    );
}

#[test]
fn l_amorcage_ne_declenche_pas_de_fausse_alerte() {
    // I3 (final branch review): on a 1920×1080 source, rung
    // 0 requires 6.22 Mb/s, but the BWE deliberately starts low
    // (`ESTIMATION_INITIALE_BPS` = 2.5 Mb/s on the transport side) and probes
    // upwards. Without a bootstrap window, the very first observation
    // would announce "Image réduite par le réseau" on an otherwise
    // perfect link.
    let base = t0();
    let mut c = Controleur::new(config(), base);

    // First estimate, low as at the real BWE start-up: 2.12 Mb/s
    // available (2.5 M × 0.9 − 128 k), well under the minimum of rung 0.
    let d = c
        .observer(obs(Some(2_500_000), None, base + Duration::from_secs(1)))
        .expect("le débit a assez bougé depuis le plafond pour produire une décision");

    assert_eq!(
        d.qualite,
        Qualite::Bonne,
        "la rampe de démarrage du BWE ne doit pas être prise pour une dégradation"
    );
    assert_eq!(
        d.encode_size,
        (1920, 1080),
        "aucune descente ne doit s'engager pendant l'amorçage"
    );

    // A second, equally low estimate, still within the bootstrap
    // window (less than 5 s after the first): still no
    // descent engaged, quality stays good.
    let d = c.observer(obs(Some(2_500_000), None, base + Duration::from_secs(3)));
    if let Some(d) = d {
        assert_eq!(d.qualite, Qualite::Bonne);
        assert_eq!(d.encode_size, (1920, 1080));
    }

    // After the bootstrap window (>= 5 s after the first estimate,
    // hence >= 6 s since `base`), a still low estimate must
    // produce the normal degradation — the bootstrap must only protect
    // the start-up ramp, not mask a really bad link.
    let mut derniere = None;
    for i in 30..80 {
        let at = base + Duration::from_millis(i * 200);
        if let Some(d) = c.observer(obs(Some(2_500_000), None, at)) {
            derniere = Some(d);
        }
    }
    let d = derniere.expect("une décision devait tomber une fois l'amorçage terminé");
    assert_eq!(
        d.qualite,
        Qualite::Degradee,
        "un débit durablement insuffisant hors amorçage doit dégrader normalement"
    );
    assert_ne!(
        d.encode_size,
        (1920, 1080),
        "la résolution devait finir par descendre"
    );
}
