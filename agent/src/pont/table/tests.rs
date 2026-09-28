use super::*;

pub(super) fn maintenant() -> Instant {
    Instant::now()
}

pub(super) fn attributs(chemin: &str) -> Attendue {
    Attendue::Attributs {
        chemin: chemin.to_string(),
    }
}

#[test]
fn une_reponse_arrivee_apres_annulation_est_jetee() {
    // ⚠️ The test spec §4.4 names. ProjFS cancels a command when
    // the calling application gives up; the browser's response, for its part, is
    // already in flight. Applying it would write into a buffer the system has taken back.
    let mut t = Table::nouvelle();
    let c = t.inscrire(42, attributs("a.txt"), maintenant() + DELAI_ATTRIBUTS);
    assert_eq!(t.en_vol(), 1);

    assert_eq!(t.annuler(42), vec![c]);
    assert_eq!(t.en_vol(), 0, "l'annulation doit retirer l'entrée");
    assert_eq!(
        t.resoudre(c, Instant::now()),
        None,
        "la réponse tardive doit être JETÉE"
    );
}

#[test]
fn une_commande_expiree_ne_reste_pas_en_table() {
    let debut = maintenant();
    let mut t = Table::nouvelle();
    let c = t.inscrire(7, attributs("a.txt"), debut + DELAI_ATTRIBUTS);

    // Just before the deadline: nothing expires.
    assert!(t.expirees(debut).is_empty());
    assert_eq!(t.en_vol(), 1);

    // At the EXACT deadline: it expires. Making expiry depend on a
    // strict overrun would make it depend on the clock's granularity.
    assert_eq!(t.expirees(debut + DELAI_ATTRIBUTS), vec![(Some(7), c)]);
    assert_eq!(t.en_vol(), 0, "en_vol() doit être décrémenté");
    // …and a second pass does not return it twice.
    assert!(t.expirees(debut + DELAI_LISTER).is_empty());
}

#[test]
fn une_reponse_arrivee_apres_expiration_est_jetee() {
    let debut = maintenant();
    let mut t = Table::nouvelle();
    let c = t.inscrire(9, attributs("a.txt"), debut + DELAI_ATTRIBUTS);
    t.expirees(debut + DELAI_ATTRIBUTS);
    assert_eq!(t.resoudre(c, Instant::now()), None);
}

#[test]
fn les_correlations_sont_monotones_et_ne_se_reutilisent_pas() {
    let mut t = Table::nouvelle();
    let e = maintenant() + DELAI_LIRE;
    let a = t.inscrire(1, attributs("a"), e);
    let b = t.inscrire(2, attributs("b"), e);
    assert_ne!(a, b);
    // And a resolved correlation is NOT recycled: a duplicated response from the
    // browser would otherwise be applied to the next command.
    t.resoudre(a, Instant::now());
    let c = t.inscrire(3, attributs("c"), e);
    assert_ne!(c, a);
    assert_ne!(c, b);
}

#[test]
fn vider_rend_tout_et_laisse_la_table_vide() {
    // It is what precedes `PrjStopVirtualizing`: a command left in flight
    // would wait there for a response nothing can deliver any more, and ProjFS
    // would wait for its completion indefinitely.
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
    // …and no response is applied afterwards any more.
    assert_eq!(t.resoudre(c1, Instant::now()), None);
}

#[test]
fn une_correlation_inconnue_rend_none_sans_paniquer() {
    let mut t = Table::nouvelle();
    assert_eq!(t.resoudre(12_345, Instant::now()), None);
    assert!(t.annuler(999).is_empty());
    assert!(t.expirees(maintenant()).is_empty());
}

#[test]
fn deux_enumerations_du_meme_chemin_coexistent() {
    // ⚠️ The enumeration session is indexed by the callback's enumeration GUID,
    // NOT by the path (spec §7.2): two applications listing the
    // same directory at the same time open two distinct sessions. Indexing
    // by path would make the second overwrite the first, and one of the
    // two would receive an empty directory — without any error being raised.
    let mut t = Table::nouvelle();
    let e = maintenant() + DELAI_LISTER;
    let g1 = [1u8; 16];
    let g2 = [2u8; 16];
    let c1 = t.inscrire(
        100,
        Attendue::Lister {
            chemin: "dossier".into(),
            enumeration: g1,
        },
        e,
    );
    let c2 = t.inscrire(
        200,
        Attendue::Lister {
            chemin: "dossier".into(),
            enumeration: g2,
        },
        e,
    );

    assert_ne!(c1, c2);
    assert_eq!(t.en_vol(), 2, "les deux sessions doivent COEXISTER");
    // …and each resolves on ITS session, not on the other's.
    let (id1, quoi1, _) = t
        .resoudre(c1, Instant::now())
        .expect("la première session existe");
    assert_eq!(id1, Some(100));
    assert_eq!(
        quoi1,
        Attendue::Lister {
            chemin: "dossier".into(),
            enumeration: g1
        }
    );
    let (id2, quoi2, _) = t
        .resoudre(c2, Instant::now())
        .expect("la seconde session existe");
    assert_eq!(id2, Some(200));
    assert_eq!(
        quoi2,
        Attendue::Lister {
            chemin: "dossier".into(),
            enumeration: g2
        }
    );
}

#[test]
fn un_debordement_du_compteur_de_correlation_ne_reutilise_pas_une_correlation_en_vol() {
    // ⚠️ This test is the only one that cannot be written naively: wrapping
    // a `u32` would require four billion registrations. It goes
    // through the `nouvelle_depuis` seam, which makes it reachable in three
    // calls. **Without it, it would be vacuous** — it would pass without exercising anything.
    let mut t = Table::nouvelle_depuis(u32::MAX - 1);
    let e = maintenant() + DELAI_LIRE;
    let a = t.inscrire(1, attributs("a"), e); // u32::MAX - 1
    let b = t.inscrire(2, attributs("b"), e); // u32::MAX
    let c = t.inscrire(3, attributs("c"), e); // wraps to 0
    let d = t.inscrire(4, attributs("d"), e); // 1

    assert_eq!(a, u32::MAX - 1);
    assert_eq!(b, u32::MAX);
    assert_eq!(c, 0, "le compteur doit reboucler, pas paniquer");
    assert_eq!(d, 1);
    assert_eq!(
        t.en_vol(),
        4,
        "aucune des quatre ne doit en écraser une autre"
    );

    // …and the case that really bites: a correlation STILL IN FLIGHT is
    // stepped over, not overwritten. We start again from 0 while 0 and 1 are taken.
    let mut t = Table::nouvelle_depuis(0);
    let e = maintenant() + DELAI_LIRE;
    let zero = t.inscrire(10, attributs("z"), e);
    let un = t.inscrire(11, attributs("u"), e);
    assert_eq!((zero, un), (0, 1));
    t.prochaine = 0; // the counter wrapped onto entries still in flight
    let apres = t.inscrire(12, attributs("v"), e);
    assert!(
        apres != zero && apres != un,
        "la corrélation rebouclée {apres} écrase une commande en vol"
    );
    assert_eq!(t.en_vol(), 3);
    // The original command still answers for ITSELF.
    assert_eq!(
        t.resoudre(zero, Instant::now()).map(|(id, _, _)| id),
        Some(Some(10))
    );
}
