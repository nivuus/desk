//! Tests d'hôte de [`super`]. Purs : aucun `.lnk`, aucun COM, aucune VM.

use super::*;

fn app(cle: &str, nom: &str, chemin: &str) -> Application {
    Application {
        cle: cle.into(),
        nom: nom.into(),
        chemin: chemin.into(),
        cible: r"c:\x\y.exe".into(),
        arguments: String::new(),
        repertoire: r"c:\x".into(),
        // ⚠️ Ce fixture N'A PAS D'ICÔNE, et c'est délibéré : le diff apparie
        // sur la CLÉ, et l'icône n'est pas une identité. Le cas d'une icône
        // qui change sans que la clé change est éprouvé à part.
        icone: None,
        source_max: proto::plateforme::SourceMax::NonMesuree,
        // ⚠️ MÊME RAISON QUE L'ICÔNE : ni l'accent ni les associations ne
        // participent à l'identité, qui est la CLÉ. Le cas d'une application
        // dont l'accent change sans que la clé change relève du même
        // raisonnement, et le diff n'a rien à en dire.
        accent: None,
        associations: Vec::new(),
    }
}

#[test]
fn deux_catalogues_identiques_rendent_un_diff_entierement_vide() {
    // 🔴 LA ROUGE : comparer par ORDRE au lieu de par CLÉ. Le parcours d'un
    // répertoire ne garantit aucun ordre, donc un simple remaniement de la
    // lecture produirait un diff plein À CHAQUE TOUR — c'est-à-dire un message
    // toutes les 30 secondes pour un disque qui n'a pas bougé.
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
    let d = diff(&[app("a", "A", "/a")], &[app("a", "A", "/a"), app("b", "B", "/b")]);
    assert_eq!(d.apparues, vec![app("b", "B", "/b")]);
    assert!(d.modifiees.is_empty());
    assert!(d.disparues.is_empty());
}

#[test]
fn un_nom_qui_change_a_cle_egale_est_une_modification() {
    // Le `.lnk` renommé : même triplet, donc même application, mais le nom
    // affiché doit suivre. Ne comparer que les clés le figerait pour toujours.
    let d = diff(&[app("a", "Ancien", "/a")], &[app("a", "Nouveau", "/a")]);
    assert_eq!(d.modifiees, vec![app("a", "Nouveau", "/a")]);
    assert!(d.apparues.is_empty());
    assert!(d.disparues.is_empty());
}

#[test]
fn un_chemin_de_lnk_qui_change_est_une_modification() {
    // Et c'est ce chemin que le lancement emploie : le laisser dériver ferait
    // lancer un raccourci qui n'est plus là.
    let d = diff(&[app("a", "A", "/bureau/a.lnk")], &[app("a", "A", "/menu/a.lnk")]);
    assert_eq!(d.modifiees, vec![app("a", "A", "/menu/a.lnk")]);
}

#[test]
fn une_application_absente_d_aujourdhui_disparait_PAR_SA_CLE() {
    // La plateforme n'a besoin que de l'identité : rendre l'objet entier ferait
    // grossir le message pour rien.
    let d = diff(&[app("a", "A", "/a"), app("b", "B", "/b")], &[app("a", "A", "/a")]);
    assert_eq!(d.disparues, vec!["b".to_string()]);
    assert!(d.apparues.is_empty());
    assert!(d.modifiees.is_empty());
}

#[test]
fn une_application_qui_revient_est_apparue() {
    // 🔴 « Une disparition n'est pas une suppression » : le diff la redonne en
    // `apparues`, et c'est la plateforme qui saura qu'elle la connaît déjà.
    // L'agent ne tient aucune mémoire des disparitions passées.
    let d1 = diff(&[app("a", "A", "/a")], &[]);
    assert_eq!(d1.disparues, vec!["a".to_string()]);
    let d2 = diff(&[], &[app("a", "A", "/a")]);
    assert_eq!(d2.apparues, vec![app("a", "A", "/a")]);
}

#[test]
fn un_catalogue_d_hier_vide_rend_tout_en_apparues_et_rien_en_disparues() {
    // C'est le premier tour, et il ne doit rien annoncer disparu.
    let d = diff(&[], &[app("a", "A", "/a"), app("b", "B", "/b")]);
    assert_eq!(d.apparues.len(), 2);
    assert!(d.disparues.is_empty());
    assert!(d.modifiees.is_empty());
}

#[test]
fn le_diff_est_deterministe_et_trie_quel_que_soit_l_ordre_d_entree() {
    // 🔴 SANS LE TRI, deux réconciliations successives émettraient des messages
    // DIFFÉRENTS pour un état IDENTIQUE, et rien ne le dirait — le journal
    // montrerait un catalogue qui bouge sans que le disque ait changé.
    let hier = vec![app("x", "X", "/x")];
    let a = vec![app("c", "C", "/c"), app("a", "A", "/a"), app("b", "B", "/b")];
    let b = vec![app("b", "B", "/b"), app("c", "C", "/c"), app("a", "A", "/a")];
    let da = diff(&hier, &a);
    let db = diff(&hier, &b);
    assert_eq!(da, db);
    assert_eq!(
        da.apparues.iter().map(|x| x.cle.as_str()).collect::<Vec<_>>(),
        ["a", "b", "c"]
    );

    let hier2 = vec![app("z", "Z", "/z"), app("y", "Y", "/y"), app("x", "X", "/x")];
    let d = diff(&hier2, &[]);
    assert_eq!(d.disparues, vec!["x".to_string(), "y".to_string(), "z".to_string()]);
}

#[test]
fn un_doublon_de_cle_dans_la_lecture_du_jour_ne_produit_qu_une_application() {
    // Deux `.lnk` au même triplet — le cas mesuré : sur la VM, 167 raccourcis
    // retenus rendent 154 clés. Le diff ne doit pas les compter deux fois, ni
    // rendre une `modifiee` pour une application qui vient d'apparaître.
    let d = diff(&[], &[app("a", "Bureau", "/bureau/a.lnk"), app("a", "Menu", "/menu/a.lnk")]);
    assert_eq!(d.apparues.len(), 1);
    assert!(d.modifiees.is_empty());
}
