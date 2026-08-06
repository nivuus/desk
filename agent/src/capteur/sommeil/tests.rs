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
    let ordres = inscrire("t5-a", 5001);
    signaler("t5-a", true, true);
    assert_eq!(premier_ordre(&ordres), Some(Ordre::Reveiller));
    retirer("t5-a");
}

#[test]
fn une_session_retiree_ne_recoit_plus_rien() {
    let _verrou = verrouiller_pour_le_test();
    let ordres = inscrire("t5-b", 5002);
    retirer("t5-b");
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
    let ordres = inscrire("t5-c", 5003);
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

    retirer("t5-c");
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
    let canal = inscrire("m1-focus", 5004);
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
    inscrire("m1-tiers", 5005);
    assert_eq!(
        etat().focalisee,
        None,
        "le focus d'une session morte doit être rendu avec le reste de ce que le registre \
         retenait d'elle"
    );

    retirer("m1-focus");
    retirer("m1-tiers");
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
        let ordres = inscrire(&nom, 5100 + i as u32);
        signaler(&nom, true, true);
        assert_eq!(
            premier_ordre(&ordres),
            Some(Ordre::Reveiller),
            "{nom} devrait s'eveiller"
        );
        recepteurs_pleins.push((nom, ordres));
    }

    // "t5-attend" arrive alors que le plafond est deja atteint : elle
    // reste endormie, faute de place.
    let ordres_attend = inscrire("t5-attend", 5200);
    signaler("t5-attend", true, true);
    assert_eq!(premier_ordre(&ordres_attend), None, "t5-attend devrait rester endormie");

    // "t5-tardif" arrive ensuite : plus recente que "t5-attend", donc elle
    // la devancerait si une place se liberait. Son recepteur est jete
    // immediatement : son canal est rompu des avant toute tentative
    // d'envoi.
    drop(inscrire("t5-tardif", 5201));
    signaler("t5-tardif", true, true);

    // Libere UNE place en retirant le premier "plein". Le vivier elit
    // alors "t5-tardif" (la plus recente des deux candidates bloquees),
    // et cette livraison echoue puisque son canal est rompu. Sans la
    // boucle de `distribuer`, le Reveiller que ce retrait engendre
    // ENSUITE pour "t5-attend" serait perdu pour toujours : le vivier
    // aurait deja pose `eveillee = true` sur "t5-attend" en interne, et
    // plus aucun rearbitrage ne le reproposerait.
    retirer("t5-plein-0");

    assert_eq!(
        premier_ordre(&ordres_attend),
        Some(Ordre::Reveiller),
        "le reveil libere par la mort de t5-tardif doit atteindre t5-attend"
    );

    // Nettoyage.
    retirer("t5-attend");
    retirer("t5-tardif");
    for (nom, _) in recepteurs_pleins.into_iter().skip(1) {
        retirer(&nom);
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
fn une_purge_sur_un_registre_vide_ne_panique_pas() {
    let mut inaptes: HashMap<String, Instant> = HashMap::new();
    purger_les_inaptitudes(&mut inaptes, Instant::now());
    assert!(inaptes.is_empty());
}

/// Remède à la réserve de revue : `oublier` (donc `retirer`) doit purger
/// `inaptes` et `rearmements`, sans quoi un rattachement — qui réinscrit la
/// MÊME session (`inscrire`, chemin de reprise de D4) — hériterait d'une
/// inaptitude ou d'un compteur de réarmements PÉRIMÉS. C'est le défaut M1 de
/// la revue finale de branche du sous-bloc D6, rejoué sur ces deux tables.
#[test]
fn un_retrait_purge_l_inaptitude_et_le_compteur_de_rearmements() {
    let _verrou = verrouiller_pour_le_test();
    let canal = inscrire("t8-purge", 6001);

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

    retirer("t8-purge");

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
