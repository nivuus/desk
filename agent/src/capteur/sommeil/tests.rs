//! Tests de `sommeil` — fichier voisin plutôt que module en ligne :
//! `sommeil.rs` était à 500 lignes pour un plafond de projet à 500 (marge
//! nulle dès la tâche 5 de D7), et cette suite à elle seule en portait 195.
//! Extraire plutôt que comprimer — même schéma que `capteur/distante.rs` /
//! `capteur/distante/tests.rs`.
//!
//! **Le chemin de module reste `sommeil::tests`** : `parts::tests` et
//! `porteurs::tests` importent `sommeil::tests::{premier_ordre,
//! verrouiller_pour_le_test}`, et cette extraction ne change ni ce chemin ni
//! aucune visibilité — seul l'emplacement physique du fichier change.

use super::*;

/// Noms uniques (t5-a, t5-b…) ne suffisent pas à isoler ces tests entre
/// eux : le vivier partagé n'a qu'UN plafond de `PLAFOND_EVEIL` places
/// pour tout le processus, et un test qui le sature (pour éprouver une
/// place qui se libère) prive de facto les autres tests, exécutés en
/// parallèle par défaut, de toute place disponible — observé : la
/// saturation à 8 fait échouer intermittemment un test voisin qui
/// s'attend à s'éveiller aussitôt. Un `Mutex` dédié aux tests sérialise
/// ce fichier sans toucher au code de production ni à `vivier.rs`.
static VERROU_TESTS: Mutex<()> = Mutex::new(());

/// `pub(super)` : repris par `parts::tests`, qui sature le même vivier
/// partagé et doit s'y sérialiser exactement de la même façon (ce module
/// et `parts::tests` sont deux descendants distincts de `sommeil`, pas
/// l'un de l'autre — d'où la visibilité explicite).
pub(super) fn verrouiller_pour_le_test() -> MutexGuard<'static, ()> {
    VERROU_TESTS.lock().unwrap_or_else(|empoisonne| empoisonne.into_inner())
}

/// Le premier ORDRE de sommeil reçu, en ignorant les parts qui peuvent le
/// précéder ou s'y intercaler.
///
/// **Nécessaire depuis que `inscrire` se termine par
/// `distribuer_les_parts`** : la toute première part d'une session part à
/// l'inscription même, avant tout ordre — une fenêtre encore endormie a
/// bien une part (le plancher `PART_DORMANTE_BPS`), et c'est délibéré
/// (voir la doc de `inscrire`). Les tests d'ORDRE, hérités de D5, portent
/// sur `Ordre` et non sur `Message` : ce filtre restaure leur intention
/// d'origine sans la changer.
///
/// `pub(super)` : repris par `parts::tests`, pour la même raison que
/// `verrouiller_pour_le_test`.
pub(super) fn premier_ordre(canal: &Receiver<Message>) -> Option<Ordre> {
    loop {
        match canal.try_recv() {
            Ok(Message::Sommeil(ordre)) => return Some(ordre),
            Ok(Message::Part { .. }) => continue,
            Ok(Message::Audio { .. }) => continue,
            Err(_) => return None,
        }
    }
}

#[test]
fn une_session_inscrite_recoit_l_ordre_de_se_reveiller_quand_elle_devient_visible() {
    let _verrou = verrouiller_pour_le_test();
    // Noms uniques : le registre est un état GLOBAL de processus, et les
    // tests Rust tournent en parallèle dans le même processus.
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
    // Pas d'accès au vivier partagé ici : aucun verrou requis.
    assert_eq!(raison_en_texte(Raison::Masquee), "masquee");
    assert_eq!(raison_en_texte(Raison::Evincee), "evincee");
}

#[test]
fn un_echec_de_reveil_rendort_la_session_et_ne_la_reelit_pas_immediatement() {
    let _verrou = verrouiller_pour_le_test();
    // "t5-c" devient visible et focalisee, donc eveillee par arbitrer().
    let (ordres, generation) = inscrire("t5-c", 5003);
    signaler("t5-c", true, true);
    assert_eq!(premier_ordre(&ordres), Some(Ordre::Reveiller));

    // La reconstruction du WindowsSource echoue : le vivier doit repasser
    // la session a l'etat endormi. Aucun nouvel ORDRE n'est du dans les
    // 500 ms de repit qui suivent, meme si la session reste visible et
    // focalisee : la reproposer immediatement bouclerait a chaque
    // arbitrage sur une construction d'encodeur vouee a rechouer. Une
    // part (le retour au plancher `PART_DORMANTE_BPS`) est en revanche
    // legitime : la fenetre est reellement rendormie.
    echec_de_reveil("t5-c");
    assert_eq!(premier_ordre(&ordres), None);

    retirer("t5-c", generation);
}

/// M1 de la revue finale de branche du sous-bloc D6 : `retirer` vidait
/// `focalisee`, mais ni `distribuer` ni le chemin `rompus` de
/// `distribuer_les_parts` ne le faisaient. Une session focalisée qui meurt
/// par canal rompu — le fil de fenêtre qui panique avant son point de
/// retrait unique — laissait donc son nom dans le registre.
///
/// **L'état est lu directement, et c'est délibéré.** La conséquence
/// visible par les parts n'est pas discriminante : un nom mort ne désigne
/// aucune fenêtre vivante, donc la majoration ne s'applique à personne —
/// ce qui est aussi le cas quand `focalisee` vaut `None`. Ce qui MORD est
/// la réinscription du même nom (rattachement, chemin de reprise de D4),
/// qui hériterait du focus sans que le client l'ait jamais réémis ; mais
/// l'éprouver par les parts exigerait un `signaler` sur ce nom, qui vide
/// `focalisee` de lui-même et effacerait le défaut avant de le mesurer.
/// Le champ est privé à ce module, et ce test en est un descendant : le
/// lire est l'observation la plus directe et la moins ambiguë.
#[test]
fn un_canal_rompu_libere_aussi_le_focus_de_la_session_morte() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation_focus) = inscrire("m1-focus", 5004);
    signaler("m1-focus", true, true);
    assert_eq!(
        etat().focalisee.as_deref(),
        Some("m1-focus"),
        "précondition : le registre tient bien cette session pour la focalisée"
    );

    // Le fil de "m1-focus" meurt SANS passer par `retirer`, exactement ce
    // qui arrive quand il panique.
    drop(canal);

    // L'inscription d'une session tierce — endormie — fait varier le
    // budget partagé, donc la part de "m1-focus", donc tente un envoi sur
    // son canal rompu : c'est ce qui déclenche la détection.
    let (_, generation_tiers) = inscrire("m1-tiers", 5005);
    assert_eq!(
        etat().focalisee,
        None,
        "le focus d'une session morte doit être rendu avec le reste de ce que le registre \
         retenait d'elle"
    );

    retirer("m1-focus", generation_focus);
    retirer("m1-tiers", generation_tiers);
}

#[test]
fn un_retrait_qui_libere_une_place_reveille_bien_la_session_qui_l_attendait() {
    let _verrou = verrouiller_pour_le_test();
    // Sature les PLAFOND_EVEIL (8) places avec des sessions dediees, dont
    // on garde les Receiver vivants pour que leur canal ne soit jamais
    // rompu par accident pendant le test.
    let mut recepteurs_pleins = Vec::new();
    for i in 0..8 {
        let nom = format!("t5-plein-{i}");
        let (ordres, generation) = inscrire(&nom, 5100 + i as u32);
        signaler(&nom, true, true);
        assert_eq!(
            premier_ordre(&ordres),
            Some(Ordre::Reveiller),
            "{nom} devrait s'eveiller"
        );
        recepteurs_pleins.push((nom, ordres, generation));
    }
    let generation_plein_0 = recepteurs_pleins[0].2;

    // "t5-attend" arrive alors que le plafond est deja atteint : elle
    // reste endormie, faute de place.
    let (ordres_attend, generation_attend) = inscrire("t5-attend", 5200);
    signaler("t5-attend", true, true);
    assert_eq!(premier_ordre(&ordres_attend), None, "t5-attend devrait rester endormie");

    // "t5-tardif" arrive ensuite : plus recente que "t5-attend", donc elle
    // la devancerait si une place se liberait. Son recepteur est jete
    // immediatement (par le `_` du destructurage) : son canal est rompu
    // des avant toute tentative d'envoi.
    let (_, generation_tardif) = inscrire("t5-tardif", 5201);
    signaler("t5-tardif", true, true);

    // Libere UNE place en retirant le premier "plein". Le vivier elit
    // alors "t5-tardif" (la plus recente des deux candidates bloquees),
    // et cette livraison echoue puisque son canal est rompu. Sans la
    // boucle de `distribuer`, le Reveiller que ce retrait engendre
    // ENSUITE pour "t5-attend" serait perdu pour toujours : le vivier
    // aurait deja pose `eveillee = true` sur "t5-attend" en interne, et
    // plus aucun rearbitrage ne le reproposerait.
    retirer("t5-plein-0", generation_plein_0);

    assert_eq!(
        premier_ordre(&ordres_attend),
        Some(Ordre::Reveiller),
        "le reveil libere par la mort de t5-tardif doit atteindre t5-attend"
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
    // Éprouve `purger_les_inaptitudes`, la fonction du PRODUIT — pas
    // `HashMap::retain`. L'horloge est injectée (`maintenant`), ce qui rend
    // le test déterministe sans aucune attente réelle.
    let mut inaptes: HashMap<String, Instant> = HashMap::new();
    let t0 = Instant::now();
    inaptes.insert("w-1".into(), t0 + Duration::from_millis(10));
    inaptes.insert("w-2".into(), t0 + Duration::from_secs(60));

    purger_les_inaptitudes(&mut inaptes, t0 + Duration::from_millis(20));

    assert!(!inaptes.contains_key("w-1"), "le répit de w-1 a expiré");
    assert!(inaptes.contains_key("w-2"), "celui de w-2 court encore");
}

#[test]
fn une_inaptitude_dont_l_echeance_vaut_exactement_maintenant_est_purgee() {
    // Sur un registre VIDE, `purger_les_inaptitudes` ne peut que retirer —
    // le test qu'il remplace (`une_purge_sur_un_registre_vide_ne_panique_pas`)
    // ne pouvait donc pas rendre l'autre valeur (revue finale de branche,
    // M3). Le cas qui vaut est la borne : `retain(|_, echeance| *echeance >
    // maintenant)` (agent/src/capteur/sommeil.rs) purge une échéance
    // EXACTEMENT égale à `maintenant`, pas seulement une échéance dépassée.
    let mut inaptes: HashMap<String, Instant> = HashMap::new();
    let maintenant = Instant::now();
    inaptes.insert("w-1".into(), maintenant);

    purger_les_inaptitudes(&mut inaptes, maintenant);

    assert!(
        !inaptes.contains_key("w-1"),
        "une echeance egale a `maintenant` doit etre purgee, pas conservee"
    );
}

/// Remède à la réserve de revue : `oublier` (donc `retirer`) doit purger
/// `inaptes` et `rearmements`, sans quoi un rattachement — qui réinscrit la
/// MÊME session (`inscrire`, chemin de reprise de D4) — hériterait d'une
/// inaptitude ou d'un compteur de réarmements PÉRIMÉS. C'est le défaut M1 de
/// la revue finale de branche du sous-bloc D6, rejoué sur ces deux tables.
#[test]
fn un_retrait_purge_l_inaptitude_et_le_compteur_de_rearmements() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("t8-purge", 6001);

    audio_mort("t8-purge");
    assert!(
        etat().inaptes.contains_key("t8-purge"),
        "précondition : la session doit être marquée inapte"
    );
    assert_eq!(
        etat().rearmements.get("t8-purge"),
        Some(&1),
        "précondition : un premier réarmement doit être compté"
    );

    retirer("t8-purge", generation);

    assert!(
        !etat().inaptes.contains_key("t8-purge"),
        "l'inaptitude d'une session retirée doit être oubliée, sinon un \
         rattachement en hériterait à tort"
    );
    assert!(
        !etat().rearmements.contains_key("t8-purge"),
        "le compteur de réarmements d'une session retirée doit être oublié, \
         sinon un rattachement hériterait d'un compteur périmé"
    );

    drop(canal);
}

/// Remède à la réserve I2 de la revue de la tâche 9 (sous-bloc D9) :
/// `SourceDistante::rattacher` ne peut pas distinguer un redémarrage réel du
/// capteur d'une simple reconnexion de canal sur un capteur resté vivant, et
/// remet `Session::audio_mort_signale` à zéro dans les deux cas — un second
/// `AudioMort` pour la MÊME session, encore inapte, ne doit donc PAS compter
/// comme un échec CONSÉCUTIF de plus : ce serait le même échec, redit, et ça
/// rapprocherait l'abandon définitif de 24 h pour une raison étrangère à
/// l'état réel de la capture.
#[test]
fn un_signal_audio_mort_redondant_ne_recompte_pas_le_rearmement() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("t9-redondant", 6002);

    audio_mort("t9-redondant");
    assert_eq!(
        etat().rearmements.get("t9-redondant"),
        Some(&1),
        "précondition : un premier réarmement doit être compté"
    );

    // Second signal, sans qu'aucun retrait n'ait eu lieu entre les deux :
    // la session est toujours inapte (répit de `REPIT_REARMEMENT_AUDIO`, pas
    // encore expiré). Un vrai canal rattaché sur un capteur relancé serait
    // indiscernable de ceci pour `SourceDistante` — c'est exactement le cas
    // que ce test isole côté capteur, où la distinction EST possible.
    audio_mort("t9-redondant");
    assert_eq!(
        etat().rearmements.get("t9-redondant"),
        Some(&1),
        "un signal redondant, reçu pendant que la session est encore \
         inapte, ne doit pas avancer le compteur de réarmements"
    );

    retirer("t9-redondant", generation);
    drop(canal);
}

/// F5 (D7, préexistant). Une session meurt, se rattache sous le même nom
/// avec une génération neuve, et le `retirer` de l'instance PRÉCÉDENTE
/// arrive après. Sans la génération, il emporterait la session vivante.
#[test]
fn un_retirer_perime_n_emporte_pas_l_inscription_neuve() {
    let mut generations: HashMap<String, u64> = HashMap::new();
    generations.insert("w-1".into(), 7); // l'inscription neuve, après rattachement

    assert!(
        retirer_est_perime(&generations, "w-1", 6),
        "le retirer de la génération 6 est en retard : il ne doit rien retirer"
    );
    assert!(
        !retirer_est_perime(&generations, "w-1", 7),
        "celui de la génération courante retire bien"
    );
}

#[test]
fn un_retirer_sur_une_session_inconnue_n_est_pas_perime() {
    // Aucune inscription : `retirer` doit suivre son chemin normal, qui est
    // déjà tolérant à l'absence. Rendre `true` ici le rendrait inerte pour
    // toute session que le registre ne connaît pas encore.
    let generations: HashMap<String, u64> = HashMap::new();
    assert!(!retirer_est_perime(&generations, "w-1", 3));
}

#[test]
fn un_retirer_d_une_generation_posterieure_n_est_pas_perime() {
    // Le cas d'un `retirer` qui arrive APRÈS l'inscription qu'il vise : il
    // porte une génération plus récente que celle enregistrée, donc il agit.
    let mut generations: HashMap<String, u64> = HashMap::new();
    generations.insert("w-1".into(), 7);
    assert!(!retirer_est_perime(&generations, "w-1", 8));
}

/// Le test que les trois ci-dessus ne pouvaient PAS voir (revue de la
/// première version de cette tâche, D9) : ils éprouvent `retirer_est_perime`
/// sur des valeurs choisies à la main, jamais sur les valeurs que
/// `inscrire`/`retirer` produisent RÉELLEMENT en production. Celui-ci simule
/// le rattachement F5 avec le chemin complet — deux `inscrire` successifs
/// pour le MÊME nom, comme le fait un enfant qui se rattache au capteur
/// après une rupture de tube (`CanalTube::rattacher`) pendant que le fil de
/// fenêtre précédent vit encore.
#[test]
fn un_rattachement_recoit_une_generation_neuve_et_le_retirer_precedent_est_perime() {
    let _verrou = verrouiller_pour_le_test();
    let (premier_canal, premiere_generation) = inscrire("t10-rattache", 7001);
    // Le rattachement : MÊME nom, avant que le `retirer` de l'instance
    // précédente n'ait eu le temps d'arriver.
    let (second_canal, seconde_generation) = inscrire("t10-rattache", 7001);

    assert_ne!(
        premiere_generation, seconde_generation,
        "deux inscriptions du même nom doivent recevoir des générations distinctes"
    );

    // Le `retirer` de l'instance PRÉCÉDENTE, arrivant après le rattachement
    // (c'est exactement F5) : il ne doit RIEN retirer de l'inscription
    // vivante.
    retirer("t10-rattache", premiere_generation);
    assert!(
        etat().canaux.contains_key("t10-rattache"),
        "un retirer périmé ne doit pas emporter l'inscription neuve"
    );

    // Le retirer de l'instance VIVANTE, lui, retire bien.
    retirer("t10-rattache", seconde_generation);
    assert!(
        !etat().canaux.contains_key("t10-rattache"),
        "le retirer de la génération courante doit retirer réellement"
    );

    drop(premier_canal);
    drop(second_canal);
}
