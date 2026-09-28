//! Host tests of [`super`]. Pure: no `.lnk`, no COM, no VM.

use super::*;

fn app(cle: &str, nom: &str, chemin: &str) -> Application {
    Application {
        cle: cle.into(),
        nom: nom.into(),
        chemin: chemin.into(),
        cible: r"c:\x\y.exe".into(),
        arguments: String::new(),
        repertoire: r"c:\x".into(),
        // ⚠️ This fixture HAS NO ICON, and it is deliberate: the diff matches
        // on the KEY, and the icon is not an identity. The case of an icon
        // that changes without the key changing is tested separately.
        icone: None,
        source_max: proto::plateforme::SourceMax::NonMesuree,
        // ⚠️ SAME REASON AS THE ICON: neither the accent nor the associations
        // take part in the identity, which is the KEY. The case of an application
        // whose accent changes without the key changing follows the same
        // reasoning, and the diff has nothing to say about it.
        accent: None,
        associations: Vec::new(),
    }
}

#[test]
fn deux_catalogues_identiques_rendent_un_diff_entierement_vide() {
    // 🔴 THE RED: comparing by ORDER instead of by KEY. Walking a
    // directory guarantees no order, so a mere reshuffle of the
    // read would produce a full diff ON EVERY ROUND — that is, a message
    // every 30 seconds for a disk that has not moved.
    let hier = vec![app("a", "A", "/a"), app("b", "B", "/b")];
    let aujourdhui = vec![app("b", "B", "/b"), app("a", "A", "/a")];
    let d = diff(&hier, &aujourdhui);
    assert!(d.est_vide(), "{d:?}");
    assert_eq!(d.apparues.len(), 0);
    assert_eq!(d.modifiees.len(), 0);
    assert_eq!(d.disparues.len(), 0);
}

#[test]
fn une_application_neuve_est_apparue_et_pas_modifiee() {
    let d = diff(
        &[app("a", "A", "/a")],
        &[app("a", "A", "/a"), app("b", "B", "/b")],
    );
    assert_eq!(d.apparues, vec![app("b", "B", "/b")]);
    assert!(d.modifiees.is_empty());
    assert!(d.disparues.is_empty());
}

#[test]
fn un_nom_qui_change_a_cle_egale_est_une_modification() {
    // The renamed `.lnk`: same triple, hence same application, but the displayed
    // name must follow. Comparing only keys would freeze it forever.
    let d = diff(&[app("a", "Ancien", "/a")], &[app("a", "Nouveau", "/a")]);
    assert_eq!(d.modifiees, vec![app("a", "Nouveau", "/a")]);
    assert!(d.apparues.is_empty());
    assert!(d.disparues.is_empty());
}

#[test]
fn un_chemin_de_lnk_qui_change_est_une_modification() {
    // And it is that path the launch uses: letting it drift would
    // launch a shortcut that is no longer there.
    let d = diff(
        &[app("a", "A", "/bureau/a.lnk")],
        &[app("a", "A", "/menu/a.lnk")],
    );
    assert_eq!(d.modifiees, vec![app("a", "A", "/menu/a.lnk")]);
}

#[test]
fn une_application_absente_d_aujourdhui_disparait_par_sa_cle() {
    // The platform only needs the identity: returning the whole object would
    // inflate the message for nothing.
    let d = diff(
        &[app("a", "A", "/a"), app("b", "B", "/b")],
        &[app("a", "A", "/a")],
    );
    assert_eq!(d.disparues, vec!["b".to_string()]);
    assert!(d.apparues.is_empty());
    assert!(d.modifiees.is_empty());
}

#[test]
fn une_application_qui_revient_est_apparue() {
    // 🔴 "A disappearance is not a deletion": the diff gives it back as
    // `apparues`, and it is the platform that will know it already knows it.
    // The agent keeps no memory of past disappearances.
    let d1 = diff(&[app("a", "A", "/a")], &[]);
    assert_eq!(d1.disparues, vec!["a".to_string()]);
    let d2 = diff(&[], &[app("a", "A", "/a")]);
    assert_eq!(d2.apparues, vec![app("a", "A", "/a")]);
}

#[test]
fn un_catalogue_d_hier_vide_rend_tout_en_apparues_et_rien_en_disparues() {
    // It is the first round, and it must announce nothing as disappeared.
    let d = diff(&[], &[app("a", "A", "/a"), app("b", "B", "/b")]);
    assert_eq!(d.apparues.len(), 2);
    assert!(d.disparues.is_empty());
    assert!(d.modifiees.is_empty());
}

#[test]
fn le_diff_est_deterministe_et_trie_quel_que_soit_l_ordre_d_entree() {
    // 🔴 WITHOUT SORTING, two successive reconciliations would emit DIFFERENT
    // messages for an IDENTICAL state, and nothing would say so — the log
    // would show a catalogue moving without the disk having changed.
    let hier = vec![app("x", "X", "/x")];
    let a = vec![
        app("c", "C", "/c"),
        app("a", "A", "/a"),
        app("b", "B", "/b"),
    ];
    let b = vec![
        app("b", "B", "/b"),
        app("c", "C", "/c"),
        app("a", "A", "/a"),
    ];
    let da = diff(&hier, &a);
    let db = diff(&hier, &b);
    assert_eq!(da, db);
    assert_eq!(
        da.apparues
            .iter()
            .map(|x| x.cle.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );

    let hier2 = vec![
        app("z", "Z", "/z"),
        app("y", "Y", "/y"),
        app("x", "X", "/x"),
    ];
    let d = diff(&hier2, &[]);
    assert_eq!(
        d.disparues,
        vec!["x".to_string(), "y".to_string(), "z".to_string()]
    );
}

#[test]
fn un_doublon_de_cle_dans_la_lecture_du_jour_ne_produit_qu_une_application() {
    // Two `.lnk` with the same triple — the measured case: on the VM, 167 kept
    // shortcuts yield 154 keys. The diff must not count them twice, nor
    // return a `modifiee` for an application that has just appeared.
    let d = diff(
        &[],
        &[
            app("a", "Bureau", "/bureau/a.lnk"),
            app("a", "Menu", "/menu/a.lnk"),
        ],
    );
    assert_eq!(d.apparues.len(), 1);
    assert!(d.modifiees.is_empty());
}
