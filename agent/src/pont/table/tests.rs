use super::*;

fn maintenant() -> Instant {
    Instant::now()
}

fn attributs(chemin: &str) -> Attendue {
    Attendue::Attributs { chemin: chemin.to_string() }
}

#[test]
fn une_reponse_arrivee_apres_annulation_est_jetee() {
    // ⚠️ Le test que la spec §4.4 nomme. ProjFS annule une commande quand
    // l'application appelante abandonne ; la réponse du navigateur, elle, est
    // déjà en vol. L'appliquer écrirait dans un tampon que le système a repris.
    let mut t = Table::nouvelle();
    let c = t.inscrire(42, attributs("a.txt"), maintenant() + DELAI_ATTRIBUTS);
    assert_eq!(t.en_vol(), 1);

    assert_eq!(t.annuler(42), Some(c));
    assert_eq!(t.en_vol(), 0, "l'annulation doit retirer l'entrée");
    assert_eq!(t.resoudre(c), None, "la réponse tardive doit être JETÉE");
}

#[test]
fn une_commande_expiree_ne_reste_pas_en_table() {
    let debut = maintenant();
    let mut t = Table::nouvelle();
    let c = t.inscrire(7, attributs("a.txt"), debut + DELAI_ATTRIBUTS);

    // Juste avant l'échéance : rien n'expire.
    assert!(t.expirees(debut).is_empty());
    assert_eq!(t.en_vol(), 1);

    // À l'échéance EXACTE : elle expire. Faire dépendre l'expiration d'un
    // dépassement strict la ferait dépendre de la granularité de l'horloge.
    assert_eq!(t.expirees(debut + DELAI_ATTRIBUTS), vec![(Some(7), c)]);
    assert_eq!(t.en_vol(), 0, "en_vol() doit être décrémenté");
    // …et une seconde passe ne la rend pas deux fois.
    assert!(t.expirees(debut + DELAI_LISTER).is_empty());
}

#[test]
fn une_reponse_arrivee_apres_expiration_est_jetee() {
    let debut = maintenant();
    let mut t = Table::nouvelle();
    let c = t.inscrire(9, attributs("a.txt"), debut + DELAI_ATTRIBUTS);
    t.expirees(debut + DELAI_ATTRIBUTS);
    assert_eq!(t.resoudre(c), None);
}

#[test]
fn les_correlations_sont_monotones_et_ne_se_reutilisent_pas() {
    let mut t = Table::nouvelle();
    let e = maintenant() + DELAI_LIRE;
    let a = t.inscrire(1, attributs("a"), e);
    let b = t.inscrire(2, attributs("b"), e);
    assert_ne!(a, b);
    // Et une corrélation résolue n'est PAS recyclée : une réponse dupliquée du
    // navigateur serait sinon appliquée à la commande suivante.
    t.resoudre(a);
    let c = t.inscrire(3, attributs("c"), e);
    assert_ne!(c, a);
    assert_ne!(c, b);
}

#[test]
fn vider_rend_tout_et_laisse_la_table_vide() {
    // C'est ce qui précède `PrjStopVirtualizing` : une commande laissée en vol
    // y attendrait une réponse que plus rien ne peut délivrer, et ProjFS
    // attendrait sa complétion indéfiniment.
    let mut t = Table::nouvelle();
    let e = maintenant() + DELAI_LISTER;
    let c1 = t.inscrire(11, attributs("a"), e);
    let c2 = t.inscrire(22, attributs("b"), e);
    let c3 = t.inscrire(33, attributs("c"), e);

    let tout = t.vider();
    assert_eq!(tout.len(), 3, "vider doit rendre TOUT");
    assert_eq!(tout, vec![(Some(11), c1), (Some(22), c2), (Some(33), c3)]);
    assert_eq!(t.en_vol(), 0);
    assert!(t.vider().is_empty());
    // …et plus aucune réponse n'est appliquée après.
    assert_eq!(t.resoudre(c1), None);
}

#[test]
fn une_correlation_inconnue_rend_none_sans_paniquer() {
    let mut t = Table::nouvelle();
    assert_eq!(t.resoudre(12_345), None);
    assert_eq!(t.annuler(999), None);
    assert!(t.expirees(maintenant()).is_empty());
}

#[test]
fn deux_enumerations_du_meme_chemin_coexistent() {
    // ⚠️ La session d'énumération est indexée par le GUID d'énumération du
    // rappel, PAS par le chemin (spec §7.2) : deux applications qui listent le
    // même répertoire en même temps ouvrent deux sessions distinctes. Indexer
    // par chemin ferait que la seconde écraserait la première, et l'une des
    // deux recevrait un répertoire vide — sans qu'aucune erreur ne soit levée.
    let mut t = Table::nouvelle();
    let e = maintenant() + DELAI_LISTER;
    let g1 = [1u8; 16];
    let g2 = [2u8; 16];
    let c1 = t.inscrire(
        100,
        Attendue::Lister { chemin: "dossier".into(), enumeration: g1 },
        e,
    );
    let c2 = t.inscrire(
        200,
        Attendue::Lister { chemin: "dossier".into(), enumeration: g2 },
        e,
    );

    assert_ne!(c1, c2);
    assert_eq!(t.en_vol(), 2, "les deux sessions doivent COEXISTER");
    // …et chacune se résout sur SA session, pas sur celle de l'autre.
    let (id1, quoi1) = t.resoudre(c1).expect("la première session existe");
    assert_eq!(id1, Some(100));
    assert_eq!(quoi1, Attendue::Lister { chemin: "dossier".into(), enumeration: g1 });
    let (id2, quoi2) = t.resoudre(c2).expect("la seconde session existe");
    assert_eq!(id2, Some(200));
    assert_eq!(quoi2, Attendue::Lister { chemin: "dossier".into(), enumeration: g2 });
}

#[test]
fn un_debordement_du_compteur_de_correlation_ne_reutilise_pas_une_correlation_en_vol() {
    // ⚠️ Ce test est le seul qui ne puisse pas s'écrire naïvement : faire
    // tourner un `u32` demanderait quatre milliards d'inscriptions. Il passe
    // par la couture `nouvelle_depuis`, ce qui le rend atteignable en trois
    // appels. **Sans elle il serait vacueux** — il passerait sans rien exercer.
    let mut t = Table::nouvelle_depuis(u32::MAX - 1);
    let e = maintenant() + DELAI_LIRE;
    let a = t.inscrire(1, attributs("a"), e); // u32::MAX - 1
    let b = t.inscrire(2, attributs("b"), e); // u32::MAX
    let c = t.inscrire(3, attributs("c"), e); // reboucle à 0
    let d = t.inscrire(4, attributs("d"), e); // 1

    assert_eq!(a, u32::MAX - 1);
    assert_eq!(b, u32::MAX);
    assert_eq!(c, 0, "le compteur doit reboucler, pas paniquer");
    assert_eq!(d, 1);
    assert_eq!(t.en_vol(), 4, "aucune des quatre ne doit en écraser une autre");

    // …et le cas qui mord vraiment : une corrélation ENCORE EN VOL est
    // enjambée, pas écrasée. On repart de 0 alors que 0 et 1 sont pris.
    let mut t = Table::nouvelle_depuis(0);
    let e = maintenant() + DELAI_LIRE;
    let zero = t.inscrire(10, attributs("z"), e);
    let un = t.inscrire(11, attributs("u"), e);
    assert_eq!((zero, un), (0, 1));
    t.prochaine = 0; // le compteur a rebouclé sur des entrées encore en vol
    let apres = t.inscrire(12, attributs("v"), e);
    assert!(
        apres != zero && apres != un,
        "la corrélation rebouclée {apres} écrase une commande en vol"
    );
    assert_eq!(t.en_vol(), 3);
    // La commande d'origine répond toujours pour ELLE.
    assert_eq!(t.resoudre(zero).map(|(id, _)| id), Some(Some(10)));
}

/// 🔴 **UNE ÉCRITURE ET UNE LECTURE NE PARTAGENT JAMAIS UNE CORRÉLATION.**
///
/// C'est la raison pour laquelle `inscrire_sans_commande` passe par CETTE
/// table, et non par un second compteur : deux compteurs indépendants sur le
/// même canal se collisionneraient, et la collision serait **silencieuse** —
/// une réponse appliquée à la mauvaise commande.
#[test]
fn une_ecriture_et_une_lecture_ne_partagent_jamais_une_correlation() {
    let e = maintenant() + DELAI_LIRE;
    let mut t = Table::nouvelle();
    let mut vues = std::collections::HashSet::new();
    for i in 0..64 {
        // Les deux sortes s'entrelacent, comme sur le chemin réel : un fil
        // d'écriture pousse pendant qu'une application lit.
        let lecture = t.inscrire(i, attributs(&format!("l{i}")), e);
        let ecriture = t.inscrire_sans_commande(
            Attendue::Ecrire { chemin: format!("e{i}"), dernier: false },
            e,
        );
        assert!(vues.insert(lecture), "corrélation {lecture} distribuée deux fois");
        assert!(vues.insert(ecriture), "corrélation {ecriture} distribuée deux fois");
    }
    assert_eq!(t.en_vol(), 128);
}

/// Une inscription sans commande reçoit une corrélation **et aucun
/// `command_id`** : `verbes::completer` doit pouvoir la distinguer.
#[test]
fn une_inscription_sans_commande_n_a_pas_de_command_id() {
    let e = maintenant() + DELAI_ECRIRE;
    let mut t = Table::nouvelle();
    let c = t.inscrire_sans_commande(Attendue::Creer { chemin: "neuf.txt".into() }, e);
    let (commande, quoi) = t.resoudre(c).expect("inscrite à l'instant");
    assert_eq!(commande, None, "une écriture ne complète AUCUN rappel ProjFS");
    assert_eq!(quoi, Attendue::Creer { chemin: "neuf.txt".into() });
}

/// 🔴 **`vider` REND LES ÉCRITURES AVEC UN `command_id` ABSENT.**
///
/// Rendre `Some(0)` ferait appeler `PrjCompleteCommand(0)` à l'arrêt du pont,
/// c'est-à-dire compléter une commande qui appartient à quelqu'un d'autre.
#[test]
fn vider_rend_les_ecritures_avec_un_command_id_absent() {
    let e = maintenant() + DELAI_ECRIRE;
    let mut t = Table::nouvelle();
    let lecture = t.inscrire(42, attributs("a"), e);
    let ecriture =
        t.inscrire_sans_commande(Attendue::Ecrire { chemin: "b".into(), dernier: true }, e);
    let tout = t.vider();
    assert_eq!(tout.len(), 2);
    assert!(tout.contains(&(Some(42), lecture)));
    assert!(tout.contains(&(None, ecriture)), "l'écriture doit sortir SANS command_id");
}

/// Une écriture expirée est retirée comme les autres.
///
/// L'exclure du balayage la laisserait en table **pour toujours** : rien
/// d'autre ne la retire, puisqu'aucun rappel ProjFS ne l'a inscrite et
/// qu'aucune annulation ne peut la viser.
#[test]
fn une_ecriture_expiree_est_retiree_comme_les_autres() {
    let debut = maintenant();
    let mut t = Table::nouvelle();
    let c = t.inscrire_sans_commande(
        Attendue::Ecrire { chemin: "gros.bin".into(), dernier: false },
        debut + DELAI_ECRIRE,
    );
    assert!(t.expirees(debut).is_empty());
    assert_eq!(t.expirees(debut + DELAI_ECRIRE), vec![(None, c)]);
    assert_eq!(t.en_vol(), 0);
}

/// `annuler` ne peut pas viser une écriture — elle n'a pas de `command_id`.
///
/// ⚠️ **Sans cette assertion, `annuler(0)` pourrait apparier une écriture dont
/// le `command_id` est `None`** le jour où la comparaison serait écrite à
/// l'envers. Une application qui abandonne son E/S emporterait alors une
/// écriture due, qui ne serait jamais poussée ET jamais retirée du journal.
#[test]
fn annuler_ne_vise_jamais_une_ecriture() {
    let e = maintenant() + DELAI_ECRIRE;
    let mut t = Table::nouvelle();
    let ecriture =
        t.inscrire_sans_commande(Attendue::Ecrire { chemin: "a".into(), dernier: true }, e);
    assert_eq!(t.annuler(0), None, "aucune commande ProjFS 0 n'existe");
    assert_eq!(t.en_vol(), 1, "l'écriture est toujours là");
    assert!(t.resoudre(ecriture).is_some());
}
