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
    let d = c.courant();

    assert_eq!(d.video_bitrate_bps, 12_000_000);
    assert_eq!(d.encode_size, (1920, 1080));
    assert_eq!(d.adaptation, Adaptation::Indisponible);
    assert_eq!(d.qualite, Qualite::Bonne);

    // Cent observations sans estimation ne changent rien et ne
    // produisent aucune décision.
    for i in 0..100 {
        let at = base + Duration::from_millis(i * 100);
        assert_eq!(c.observer(obs(None, None, at)), None);
    }
    assert_eq!(c.courant().adaptation, Adaptation::Indisponible);
}

#[test]
fn une_session_muette_ne_retranche_plus_le_budget_audio() {
    // Le défaut préexistant que D7 corrige : `Controleur::new` posait
    // `audio_bps` inconditionnellement, si bien que sept fenêtres sur huit
    // amputaient leur budget vidéo de 128 kb/s pour une piste qu'elles
    // n'avaient pas — environ 8,5 % d'une part de 1,5 Mb/s.
    let base = t0();
    let mut avec = Controleur::new(config(), base);
    let mut sans = Controleur::new(config(), base);
    sans.changer_audio_bps(0);

    let o = obs(Some(2_000_000), None, base + DELAI_AMORCAGE * 2);
    avec.observer(o);
    sans.observer(o);

    assert!(
        sans.courant().video_bitrate_bps > avec.courant().video_bitrate_bps,
        "sans piste audio, le budget video doit etre plus grand : {} vs {}",
        sans.courant().video_bitrate_bps,
        avec.courant().video_bitrate_bps
    );
    assert_eq!(
        sans.courant().video_bitrate_bps - avec.courant().video_bitrate_bps,
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

    // +5 % : sous le seuil, aucune décision.
    assert_eq!(
        c.observer(obs(Some(9_450_000), None, base + Duration::from_secs(3))),
        None
    );
    // +20 % : au-delà du seuil, décision produite.
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

    // 5 Mb/s : sous le minimum du barreau 0 (6,2 Mb/s). Il faut 2 s — mais
    // la toute première estimation date de 1 s ci-dessus, donc les 5 s
    // suivantes (jusqu'à 6 s) tombent dans `DELAI_AMORCAGE` (I3) : le
    // contrôleur n'y vise jamais un barreau pire que le courant, la
    // descente ne peut donc commencer à s'accumuler qu'à partir de 6 s,
    // pour aboutir à 8 s. La borne du balayage est repoussée en
    // conséquence (40 -> 80, soit 15,8 s) pour laisser cette marge, sans
    // quoi ce test daterait d'avant l'amorçage et échouerait à tort.
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
fn sous_le_plancher_la_qualite_est_declaree_insuffisante() {
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
    // On est descendu au dernier barreau, pas plus bas : la cadence n'est
    // jamais sacrifiée automatiquement.
    let echelle = Echelle::depuis((1920, 1080), 60);
    assert_eq!(d.encode_size, echelle.barreaux().last().unwrap().taille);
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

    // 60 % de perte : plafonné à 25, au-delà duquel la redondance coûte
    // plus qu'elle ne sauve.
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
    // Amorce le contrôleur puis laisse s'écouler `DELAI_AMORCAGE` (5 s,
    // voir I3) avant les deux observations qui font l'objet de ce test :
    // il vérifie un mensonge possible en RÉGIME ÉTABLI, pas pendant la
    // rampe de démarrage du BWE, que l'amorçage protège maintenant
    // délibérément (autre test dédié à ce cas : voir
    // `l_amorcage_ne_declenche_pas_de_fausse_alerte`).
    c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)));
    c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(7)));

    // Effondrement à 5 Mb/s, observé une seule fois, moins de 2 s après
    // l'observation précédente : l'hystérésis de descente n'a pas eu le
    // temps de faire descendre la résolution, qui reste donc la taille
    // source. Le débit vidéo, lui, bascule immédiatement (aucune
    // hystérésis ne le protège).
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
    // I3 (revue finale de branche) : sur une source 1920×1080, le barreau
    // 0 exige 6,22 Mb/s, mais le BWE part volontairement bas
    // (`ESTIMATION_INITIALE_BPS` = 2,5 Mb/s côté transport) et sonde à la
    // hausse. Sans fenêtre d'amorçage, la toute première observation
    // annoncerait « Image réduite par le réseau » sur un lien par ailleurs
    // parfait.
    let base = t0();
    let mut c = Controleur::new(config(), base);

    // Première estimation, basse comme au vrai démarrage du BWE : 2,12 Mb/s
    // disponibles (2,5 M × 0,9 − 128 k), bien sous le minimum du barreau 0.
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

    // Une seconde estimation tout aussi basse, encore pendant la fenêtre
    // d'amorçage (moins de 5 s après la première) : toujours aucune
    // descente engagée, la qualité reste bonne.
    let d = c.observer(obs(Some(2_500_000), None, base + Duration::from_secs(3)));
    if let Some(d) = d {
        assert_eq!(d.qualite, Qualite::Bonne);
        assert_eq!(d.encode_size, (1920, 1080));
    }

    // Après la fenêtre d'amorçage (>= 5 s après la première estimation,
    // donc >= 6 s depuis `base`), une estimation toujours basse doit,
    // elle, produire la dégradation normale — l'amorçage ne doit protéger
    // que la rampe de démarrage, pas masquer un lien réellement mauvais.
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
