use super::*;

/// 🔴 **Les formes sur le fil sont ÉPINGLÉES, littéralement.** Ce dépôt a
/// laissé passer une variante `battement-recu` verte sur cinquante tests parce
/// que rien n'épinglait ses octets ; `proto::fichiers` épingle déjà les sept
/// codes d'échec pour la même raison. Un champ renommé ici casse le pont sans
/// casser un seul test, s'il n'est pas épinglé.
#[test]
fn les_entetes_de_requete_ont_une_forme_epinglee_sur_le_fil() {
    assert_eq!(
        serde_json::to_string(&Chemin { chemin: "dossier/note.txt".into() }).unwrap(),
        r#"{"chemin":"dossier/note.txt"}"#
    );
    assert_eq!(
        serde_json::to_string(&Lire {
            chemin: "gros.bin".into(),
            position: 65536,
            longueur: 4096
        })
        .unwrap(),
        r#"{"chemin":"gros.bin","position":65536,"longueur":4096}"#
    );
}

#[test]
fn les_entetes_de_reponse_se_lisent_depuis_leur_forme_epinglee() {
    let meta: Meta = serde_json::from_str(
        r#"{"repertoire":false,"taille":1234,"modifie":1690000000000}"#,
    )
    .expect("Meta lisible");
    assert_eq!(meta, Meta { repertoire: false, taille: 1234, modifie: 1_690_000_000_000 });

    let entrees: Entrees = serde_json::from_str(
        r#"{"entrees":[{"nom":"a.txt","repertoire":false,"taille":7,"modifie":42}]}"#,
    )
    .expect("Entrees lisible");
    assert_eq!(entrees.entrees.len(), 1);
    assert_eq!(entrees.entrees[0].nom, "a.txt");
    assert_eq!(entrees.entrees[0].taille, 7);

    let donnees: Donnees =
        serde_json::from_str(r#"{"position":128,"longueur":64}"#).expect("Donnees lisible");
    assert_eq!(donnees, Donnees { position: 128, longueur: 64 });

    let echec: Echec = serde_json::from_str(r#"{"code":"chemin-introuvable"}"#)
        .expect("Echec lisible");
    assert_eq!(echec.code, proto::fichiers::CodeEchec::CheminIntrouvable);
}

/// Un en-tête auquel il manque un champ est **rejeté**, jamais silencieusement
/// complété — la doctrine de version de `proto::control`, appliquée aux
/// en-têtes : aucun `#[serde(default)]` nulle part.
#[test]
fn un_entete_incomplet_est_rejete_plutot_que_complete() {
    assert!(serde_json::from_str::<Meta>(r#"{"repertoire":false,"taille":1}"#).is_err());
    assert!(serde_json::from_str::<Donnees>(r#"{"position":0}"#).is_err());
}

/// 🔴 **L'époque de FILETIME n'est pas celle d'Unix, et l'écart est de 369
/// ans.** Se tromper d'époque ou d'unité rend des dates de 1601 dans
/// l'Explorateur — visible, mais seulement si quelqu'un regarde. Se tromper de
/// FACTEUR (10⁷ contre 10⁶) ne se voit quasiment pas.
#[test]
fn l_epoque_unix_devient_l_epoque_filetime() {
    // 1ᵉʳ janvier 1970, 00:00:00 UTC = 116 444 736 000 000 000 unités de 100 ns
    // depuis le 1ᵉʳ janvier 1601.
    assert_eq!(filetime_depuis_ms(0), 116_444_736_000_000_000);
    // Une milliseconde vaut 10 000 unités de 100 ns.
    assert_eq!(filetime_depuis_ms(1), 116_444_736_000_010_000);
    assert_eq!(filetime_depuis_ms(1000), 116_444_736_010_000_000);
}

/// Une date antérieure à 1970 est licite (`lastModified` peut être négatif) ;
/// une date antérieure à **1601** ne l'est pas, et rendrait un FILETIME négatif
/// que Windows interprète comme un temps relatif. Elle est ramenée à zéro.
#[test]
fn une_date_anterieure_a_1601_est_ramenee_a_zero() {
    assert_eq!(filetime_depuis_ms(-11_644_473_600_000), 0);
    assert_eq!(filetime_depuis_ms(-11_644_473_600_001), 0);
    assert_eq!(filetime_depuis_ms(i64::MIN), 0);
    // Juste au-dessus de l'époque FILETIME : toujours positif.
    assert_eq!(filetime_depuis_ms(-11_644_473_599_999), 10_000);
}

/// Un `lastModified` absurde ne doit pas faire déborder le calcul : un
/// débordement en `release` boucle en silence et rendrait une date arbitraire.
#[test]
fn une_date_absurde_ne_deborde_pas() {
    assert_eq!(filetime_depuis_ms(i64::MAX), i64::MAX);
}
